//! Budget-bounded, tiered evidence assembly and its manifest (issue #403).
//!
//! Assembly fills a token budget in three passes:
//!
//! 1. **Structured clinical truth** — canonical diagnoses, medications, scores
//!    and plans, capped at [`AssemblyConfig::structured_share`] of the budget.
//! 2. **Retrieved verbatim evidence** — hot and cold units in retrieval order,
//!    quoted exactly, with their character range and revision.
//! 3. **Cold pointers** — dated titles of what exists but did not fit, so the
//!    model can say "there is more, from this date" instead of inventing it.
//!
//! Nothing is paraphrased or truncated mid-unit: a unit either fits whole or is
//! recorded in the manifest as omitted, which is what keeps protected spans
//! (doses, negations, dates) intact.
//!
//! The manifest is metadata-only: unit ids, provenance, selection reasons and
//! content digests, plus the question terms the caller already has. It never
//! carries record text, so it can be persisted and shown next to an answer
//! without duplicating the record.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::llm::sanitize::sanitize_for_prompt;

use super::index::{EvidenceUnit, IndexStats};
use super::protect::{ProtectedKind, ProtectedSpan};
use super::provenance::{self, RecordKind, SpanResolution, Tier};
use super::retrieve::{self, Candidate, RetrievalDiagnostics, RetrievalParams, Selection};
use super::tokens::TokenCounter;

/// How the budget is divided between tiers.
#[derive(Debug, Clone)]
pub struct AssemblyConfig {
    /// Total tokens the evidence block may occupy.
    pub token_budget: usize,
    /// Fraction of the budget reserved for structured clinical truth.
    pub structured_share: f64,
    /// Fraction of the budget reserved for the cold pointer list.
    pub cold_pointer_share: f64,
    /// Upper bound on pointer lines, independent of the token share.
    pub max_cold_pointers: usize,
    pub retrieval: RetrievalParams,
}

impl Default for AssemblyConfig {
    fn default() -> Self {
        Self {
            token_budget: 12_288,
            structured_share: 0.35,
            cold_pointer_share: 0.08,
            max_cold_pointers: 12,
            retrieval: RetrievalParams::default(),
        }
    }
}

/// Tokens spent per tier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierTokens {
    pub structured: usize,
    pub hot: usize,
    pub cold: usize,
    pub pointers: usize,
    pub overhead: usize,
}

/// One included evidence unit, fully attributed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    /// Citation marker used in the prompt and expected in the answer (`E1`).
    pub citation: String,
    pub unit_id: String,
    pub patient_id: String,
    pub record_kind: RecordKind,
    pub record_id: String,
    pub section: String,
    /// Revision of the source section this text was cut from.
    pub revision: String,
    pub tier: Tier,
    pub label: String,
    pub occurred_at: String,
    /// Character range inside the source section.
    pub char_start: usize,
    pub char_end: usize,
    /// Truncated SHA-256 of the exact unit text, so an entry can be tied to
    /// content without copying record text into the manifest.
    pub text_sha256: String,
    pub tokens: usize,
    /// Token range inside the assembled evidence block.
    pub prompt_token_start: usize,
    pub prompt_token_end: usize,
    /// Protected spans that survived, relative to the unit text.
    pub protected_spans: Vec<ProtectedSpan>,
    pub selection: Selection,
    pub selection_reasons: Vec<String>,
}

/// A unit that was considered but left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmittedEntry {
    pub unit_id: String,
    pub record_kind: RecordKind,
    pub record_id: String,
    pub section: String,
    pub tier: Tier,
    pub occurred_at: String,
    pub tokens: usize,
    pub reason: String,
}

/// Count of retained protected spans by kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectionCount {
    pub kind: ProtectedKind,
    pub count: usize,
}

/// Everything about one assembled prompt except the clinical text itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceManifest {
    pub manifest_id: String,
    pub patient_id: String,
    /// Aggregate revision of the patient's record at assembly time.
    pub patient_revision: String,
    /// Truncated SHA-256 of the question, so a manifest can be matched to a
    /// query without storing the query.
    pub question_sha256: String,
    pub created_at: String,
    pub token_budget: usize,
    pub token_counter: String,
    pub prompt_tokens: usize,
    pub tier_tokens: TierTokens,
    pub entries: Vec<ManifestEntry>,
    pub omitted: Vec<OmittedEntry>,
    pub protected_spans_retained: Vec<ProtectionCount>,
    pub retrieval: RetrievalDiagnostics,
    pub index: IndexStats,
}

impl EvidenceManifest {
    /// The entry for a citation marker such as `E3`.
    pub fn entry(&self, citation: &str) -> Option<&ManifestEntry> {
        self.entries
            .iter()
            .find(|entry| entry.citation.eq_ignore_ascii_case(citation))
    }
}

/// An assembled evidence block plus its manifest.
#[derive(Debug, Clone)]
pub struct AssembledEvidence {
    /// The evidence block, ready to be embedded in a prompt. Unit text is
    /// already sanitised for prompt inclusion.
    pub evidence: String,
    pub manifest: EvidenceManifest,
}

/// Assemble evidence for one question from an already refreshed index.
///
/// `index_stats` is recorded in the manifest so a reader can see what the
/// refresh did; [`super::assemble_patient_evidence`] is the usual entry point
/// and passes it through.
pub fn assemble(
    conn: &Connection,
    patient_id: &str,
    question: &str,
    query_vec: Option<&[f32]>,
    config: &AssemblyConfig,
    counter: &dyn TokenCounter,
    index_stats: IndexStats,
) -> Result<AssembledEvidence, AppError> {
    let patient_revision = provenance::patient_revision(conn, patient_id)?;
    let (candidates, retrieval_diagnostics) =
        retrieve::retrieve(conn, patient_id, question, query_vec, &config.retrieval)?;

    let mut builder = BlockBuilder::new(counter, config.token_budget);
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut omitted: Vec<OmittedEntry> = Vec::new();
    let mut tier_tokens = TierTokens::default();

    builder.push_section(&format!(
        "===== EVIDENZ (Patientenakte {patient_revision}) =====\n\
         Jede Zeile ist ein wörtlicher Auszug mit Quellenangabe [E# | Datum | Quelle | Revision].\n"
    ));
    tier_tokens.overhead = builder.tokens();

    // ── Tier 1: structured clinical truth ────────────────────────────────────
    let structured_budget = share_of(config.token_budget, config.structured_share);
    let structured = retrieve::structured_units(conn, patient_id)?;
    let selections = candidate_selections(&candidates);
    let mut structured_header = Some("\n--- STRUKTURIERTE KLINISCHE FAKTEN ---\n");
    let mut structured_spent = 0usize;

    for unit in &structured {
        if structured_spent >= structured_budget {
            omitted.push(omit(unit, "structured_budget_exhausted"));
            continue;
        }
        let citation = format!("E{}", entries.len() + 1);
        let block = render_unit(&citation, unit);
        let before = builder.tokens();
        match builder.try_push(
            structured_header.unwrap_or(""),
            &block,
            structured_budget - structured_spent,
        ) {
            Some(span) => {
                structured_header = None;
                structured_spent += builder.tokens().saturating_sub(before);
                tier_tokens.structured += builder.tokens().saturating_sub(before);
                entries.push(entry(
                    &citation,
                    unit,
                    selection_for(&selections, unit, true),
                    span,
                ));
            }
            None => omitted.push(omit(unit, "structured_budget_exhausted")),
        }
    }

    // ── Tier 2: retrieved verbatim evidence ──────────────────────────────────
    let pointer_reserve = share_of(config.token_budget, config.cold_pointer_share);
    let verbatim_budget = config.token_budget.saturating_sub(pointer_reserve);
    let mut hot_header = Some("\n--- WÖRTLICHE EVIDENZ (AKTUELL) ---\n");
    let mut cold_header = Some("\n--- WÖRTLICHE EVIDENZ (ARCHIV, GEZIELT ABGERUFEN) ---\n");
    let mut included_ids: Vec<String> = entries.iter().map(|e| e.unit_id.clone()).collect();

    for candidate in &candidates {
        let unit = &candidate.unit;
        if included_ids.contains(&unit.id) {
            continue;
        }
        if unit.tier == Tier::Structured {
            // Already handled above; a structured unit that did not fit stays
            // omitted rather than reappearing here.
            continue;
        }

        let citation = format!("E{}", entries.len() + 1);
        let block = render_unit(&citation, unit);
        let header = match unit.tier {
            Tier::Hot => hot_header,
            _ => cold_header,
        };
        let before = builder.tokens();
        match builder.try_push(
            header.unwrap_or(""),
            &block,
            verbatim_budget.saturating_sub(builder.tokens()),
        ) {
            Some(span) => {
                match unit.tier {
                    Tier::Hot => {
                        hot_header = None;
                        tier_tokens.hot += builder.tokens().saturating_sub(before);
                    }
                    _ => {
                        cold_header = None;
                        tier_tokens.cold += builder.tokens().saturating_sub(before);
                    }
                }
                included_ids.push(unit.id.clone());
                entries.push(entry(&citation, unit, candidate.selection.clone(), span));
            }
            None => omitted.push(omit(unit, "budget_exhausted")),
        }
    }

    // ── Tier 3: cold pointers ────────────────────────────────────────────────
    let cold = retrieve::cold_index(conn, patient_id, config.max_cold_pointers * 3)?;
    let mut pointer_header = Some("\n--- NICHT ENTHALTEN, ABER VORHANDEN ---\n");
    let mut pointers = 0usize;
    for unit in &cold {
        if pointers >= config.max_cold_pointers {
            break;
        }
        if included_ids.contains(&unit.id) {
            continue;
        }
        let line = format!(
            "- {} | {} | Zeichen {}–{} | {} (nicht im Prompt enthalten)\n",
            unit.occurred_at,
            sanitize_for_prompt(&unit.label),
            unit.char_start,
            unit.char_end,
            unit.revision
        );
        let before = builder.tokens();
        if builder
            .try_push(
                pointer_header.unwrap_or(""),
                &line,
                config.token_budget.saturating_sub(builder.tokens()),
            )
            .is_none()
        {
            break;
        }
        pointer_header = None;
        tier_tokens.pointers += builder.tokens().saturating_sub(before);
        pointers += 1;
        // A unit dropped for budget earlier is now represented by a pointer;
        // record that once, with the more informative reason.
        omitted.retain(|entry| entry.unit_id != unit.id);
        omitted.push(omit(unit, "cold_pointer_only"));
    }

    let evidence = builder.finish();
    let prompt_tokens = counter.count(&evidence);

    let manifest = EvidenceManifest {
        manifest_id: uuid::Uuid::now_v7().to_string(),
        patient_id: patient_id.to_string(),
        patient_revision,
        question_sha256: provenance::text_digest(question),
        created_at: chrono::Utc::now().to_rfc3339(),
        token_budget: config.token_budget,
        token_counter: counter.label().to_string(),
        prompt_tokens,
        tier_tokens,
        protected_spans_retained: protection_counts(&entries),
        entries,
        omitted,
        retrieval: retrieval_diagnostics,
        index: index_stats,
    };

    Ok(AssembledEvidence { evidence, manifest })
}

/// Persist a manifest for later inspection. Metadata only — no record text.
pub fn store_manifest(conn: &Connection, manifest: &EvidenceManifest) -> Result<(), AppError> {
    let json = serde_json::to_string(manifest)
        .map_err(|e| AppError::Llm(format!("Failed to encode evidence manifest: {e}")))?;
    conn.execute(
        "INSERT INTO evidence_manifests \
            (id, patient_id, patient_revision, question_sha256, token_budget, prompt_tokens, \
             manifest_json, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            manifest.manifest_id,
            manifest.patient_id,
            manifest.patient_revision,
            manifest.question_sha256,
            manifest.token_budget as i64,
            manifest.prompt_tokens as i64,
            json,
            manifest.created_at
        ],
    )?;
    Ok(())
}

/// The most recent stored manifest for a patient.
pub fn latest_manifest(
    conn: &Connection,
    patient_id: &str,
) -> Result<Option<EvidenceManifest>, AppError> {
    let json: Option<String> = conn
        .query_row(
            "SELECT manifest_json FROM evidence_manifests WHERE patient_id = ?1 \
             ORDER BY created_at DESC, id DESC LIMIT 1",
            [patient_id],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(AppError::Database(other)),
        })?;

    match json {
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| AppError::Llm(format!("Failed to decode evidence manifest: {e}"))),
        None => Ok(None),
    }
}

// ── Answer auditing ──────────────────────────────────────────────────────────

/// One citation found in an answer, checked against the manifest and the record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CitationCheck {
    pub citation: String,
    /// False when the answer cited a marker the manifest never issued.
    pub in_manifest: bool,
    pub unit_id: Option<String>,
    pub label: Option<String>,
    pub occurred_at: Option<String>,
    /// The span still resolves to the same text at the same current revision.
    pub traceable: bool,
    pub resolution: Option<SpanResolution>,
}

/// Whether an answer's citations are all backed by current sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnswerAudit {
    pub manifest_id: String,
    pub citations: Vec<CitationCheck>,
    /// Markers cited by the answer that the manifest never issued.
    pub unsupported_citations: Vec<String>,
    /// Cited markers whose source moved on or vanished since assembly.
    pub stale_citations: Vec<String>,
    pub cited_entries: usize,
    pub uncited_entries: usize,
}

impl AnswerAudit {
    /// True when every citation in the answer traces to an exact current
    /// source revision.
    pub fn all_citations_traceable(&self) -> bool {
        self.unsupported_citations.is_empty()
            && self.stale_citations.is_empty()
            && self.citations.iter().all(|check| check.traceable)
    }
}

/// Check every `[E#]` citation in `answer` against the manifest and the live
/// record.
pub fn audit_answer(
    conn: &Connection,
    manifest: &EvidenceManifest,
    answer: &str,
) -> Result<AnswerAudit, AppError> {
    let cited = extract_citations(answer);
    let mut checks = Vec::with_capacity(cited.len());
    let mut unsupported = Vec::new();
    let mut stale = Vec::new();

    for citation in &cited {
        match manifest.entry(citation) {
            Some(entry) => {
                // Content is compared by digest so the manifest stays text-free.
                let resolution = provenance::resolve_span_digest(
                    conn,
                    &entry.patient_id,
                    entry.record_kind,
                    &entry.record_id,
                    &entry.section,
                    &entry.revision,
                    entry.char_start,
                    entry.char_end,
                    &entry.text_sha256,
                )?;
                let traceable = resolution.is_traceable();
                if !traceable {
                    stale.push(citation.clone());
                }
                checks.push(CitationCheck {
                    citation: citation.clone(),
                    in_manifest: true,
                    unit_id: Some(entry.unit_id.clone()),
                    label: Some(entry.label.clone()),
                    occurred_at: Some(entry.occurred_at.clone()),
                    traceable,
                    resolution: Some(resolution),
                });
            }
            None => {
                unsupported.push(citation.clone());
                checks.push(CitationCheck {
                    citation: citation.clone(),
                    in_manifest: false,
                    unit_id: None,
                    label: None,
                    occurred_at: None,
                    traceable: false,
                    resolution: None,
                });
            }
        }
    }

    let cited_entries = manifest
        .entries
        .iter()
        .filter(|entry| {
            cited
                .iter()
                .any(|c| c.eq_ignore_ascii_case(&entry.citation))
        })
        .count();

    Ok(AnswerAudit {
        manifest_id: manifest.manifest_id.clone(),
        citations: checks,
        unsupported_citations: unsupported,
        stale_citations: stale,
        cited_entries,
        uncited_entries: manifest.entries.len() - cited_entries,
    })
}

/// Citation markers (`E1`, `[E12]`) in the order they first appear.
pub fn extract_citations(answer: &str) -> Vec<String> {
    let chars: Vec<char> = answer.chars().collect();
    let mut citations: Vec<String> = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let is_marker_start = (chars[index] == 'E' || chars[index] == 'e')
            && (index == 0 || !chars[index - 1].is_alphanumeric());
        if !is_marker_start {
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < chars.len() && chars[end].is_ascii_digit() {
            end += 1;
        }
        if end == index + 1 || (end < chars.len() && chars[end].is_alphanumeric()) {
            index += 1;
            continue;
        }
        let citation: String = chars[index..end].iter().collect();
        let citation = format!("E{}", &citation[1..]);
        if !citations.contains(&citation) {
            citations.push(citation);
        }
        index = end;
    }
    citations
}

// ── Rendering helpers ────────────────────────────────────────────────────────

/// Incrementally builds the evidence block and measures it exactly.
///
/// Each candidate block is appended, the whole block re-measured, and the
/// append rolled back when it would break a budget. That makes the returned
/// token spans exact rather than additive estimates, at the cost of one
/// tokenizer pass per candidate — single-digit milliseconds at a 16K budget.
struct BlockBuilder<'a> {
    text: String,
    tokens: usize,
    counter: &'a dyn TokenCounter,
    hard_budget: usize,
}

impl<'a> BlockBuilder<'a> {
    fn new(counter: &'a dyn TokenCounter, hard_budget: usize) -> Self {
        Self {
            text: String::new(),
            tokens: 0,
            counter,
            hard_budget,
        }
    }

    fn tokens(&self) -> usize {
        self.tokens
    }

    fn push_section(&mut self, text: &str) {
        self.text.push_str(text);
        self.tokens = self.counter.count(&self.text);
    }

    /// Append `header` (once) plus `block`, keeping the total inside both the
    /// tier budget and the hard budget. Returns the block's token span.
    fn try_push(
        &mut self,
        header: &str,
        block: &str,
        tier_allowance: usize,
    ) -> Option<(usize, usize)> {
        let restore_len = self.text.len();
        let restore_tokens = self.tokens;

        self.text.push_str(header);
        let start = self.counter.count(&self.text);
        self.text.push_str(block);
        let end = self.counter.count(&self.text);

        let spent = end.saturating_sub(restore_tokens);
        if end > self.hard_budget || spent > tier_allowance {
            self.text.truncate(restore_len);
            self.tokens = restore_tokens;
            return None;
        }
        self.tokens = end;
        Some((start, end))
    }

    fn finish(self) -> String {
        self.text
    }
}

/// One evidence line: citation, date, source label, revision, exact text.
fn render_unit(citation: &str, unit: &EvidenceUnit) -> String {
    format!(
        "[{} | {} | {} | Zeichen {}–{} | {}]\n{}\n",
        citation,
        unit.occurred_at,
        sanitize_for_prompt(&unit.label),
        unit.char_start,
        unit.char_end,
        unit.revision,
        sanitize_for_prompt(&unit.text)
    )
}

fn share_of(budget: usize, share: f64) -> usize {
    ((budget as f64) * share.clamp(0.0, 1.0)).floor() as usize
}

fn candidate_selections(candidates: &[Candidate]) -> Vec<(String, Selection)> {
    candidates
        .iter()
        .map(|candidate| (candidate.unit.id.clone(), candidate.selection.clone()))
        .collect()
}

fn selection_for(
    selections: &[(String, Selection)],
    unit: &EvidenceUnit,
    structured: bool,
) -> Selection {
    selections
        .iter()
        .find(|(id, _)| *id == unit.id)
        .map(|(_, selection)| selection.clone())
        .unwrap_or(Selection {
            structured_truth: structured,
            ..Selection::default()
        })
}

fn entry(
    citation: &str,
    unit: &EvidenceUnit,
    selection: Selection,
    span: (usize, usize),
) -> ManifestEntry {
    ManifestEntry {
        citation: citation.to_string(),
        unit_id: unit.id.clone(),
        patient_id: unit.patient_id.clone(),
        record_kind: unit.kind,
        record_id: unit.record_id.clone(),
        section: unit.section.clone(),
        revision: unit.revision.clone(),
        tier: unit.tier,
        label: unit.label.clone(),
        occurred_at: unit.occurred_at.clone(),
        char_start: unit.char_start,
        char_end: unit.char_end,
        text_sha256: provenance::text_digest(&unit.text),
        tokens: span.1.saturating_sub(span.0),
        prompt_token_start: span.0,
        prompt_token_end: span.1,
        protected_spans: unit.protected.clone(),
        selection_reasons: selection.reasons(),
        selection,
    }
}

fn omit(unit: &EvidenceUnit, reason: &str) -> OmittedEntry {
    OmittedEntry {
        unit_id: unit.id.clone(),
        record_kind: unit.kind,
        record_id: unit.record_id.clone(),
        section: unit.section.clone(),
        tier: unit.tier,
        occurred_at: unit.occurred_at.clone(),
        tokens: unit.token_estimate,
        reason: reason.to_string(),
    }
}

fn protection_counts(entries: &[ManifestEntry]) -> Vec<ProtectionCount> {
    let mut counts: Vec<ProtectionCount> = Vec::new();
    for entry in entries {
        for span in &entry.protected_spans {
            match counts.iter_mut().find(|count| count.kind == span.kind) {
                Some(count) => count.count += 1,
                None => counts.push(ProtectionCount {
                    kind: span.kind,
                    count: 1,
                }),
            }
        }
    }
    counts.sort_by_key(|count| count.kind);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::evidence::index::{refresh_patient_index, IndexConfig};
    use crate::llm::evidence::test_support::{seed_patient, TestVault};
    use crate::llm::evidence::tokens::HeuristicCounter;

    fn assembled(
        conn: &Connection,
        patient_id: &str,
        question: &str,
        config: &AssemblyConfig,
    ) -> AssembledEvidence {
        let stats = refresh_patient_index(conn, patient_id, &IndexConfig::default()).unwrap();
        assemble(
            conn,
            patient_id,
            question,
            None,
            config,
            &HeuristicCounter,
            stats,
        )
        .unwrap()
    }

    #[test]
    fn assembled_prompt_cites_structured_truth_and_verbatim_evidence() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let result = assembled(
            &conn,
            &patient.patient_id,
            "Wie hat sich der Schlaf unter Sertralin entwickelt?",
            &AssemblyConfig::default(),
        );

        assert!(result.evidence.contains("STRUKTURIERTE KLINISCHE FAKTEN"));
        assert!(result.evidence.contains("WÖRTLICHE EVIDENZ"));
        assert!(result.evidence.contains("[E1"));
        assert!(result.manifest.entries.len() > 1);
        assert!(result
            .manifest
            .entries
            .iter()
            .any(|entry| entry.tier == Tier::Structured));
        assert!(result
            .manifest
            .entries
            .iter()
            .any(|entry| entry.tier != Tier::Structured));
        // Every entry is attributed.
        for entry in &result.manifest.entries {
            assert_eq!(entry.patient_id, patient.patient_id);
            assert!(entry.revision.starts_with("r1:"));
            assert!(!entry.selection_reasons.is_empty());
            assert!(entry.prompt_token_end > entry.prompt_token_start);
        }
    }

    #[test]
    fn assembly_stays_inside_the_token_budget_and_records_what_it_dropped() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        conn.execute(
            "UPDATE sessions SET notes = ?2 WHERE patient_id = ?1",
            rusqlite::params![
                patient.patient_id,
                "Schlafstörungen weiterhin dokumentiert. ".repeat(200)
            ],
        )
        .unwrap();

        let config = AssemblyConfig {
            token_budget: 400,
            ..AssemblyConfig::default()
        };
        let result = assembled(&conn, &patient.patient_id, "Schlafstörungen", &config);

        assert!(result.manifest.prompt_tokens <= 400, "budget must hold");
        assert_eq!(
            result.manifest.prompt_tokens,
            HeuristicCounter.count(&result.evidence)
        );
        assert!(
            result
                .manifest
                .omitted
                .iter()
                .any(|omitted| omitted.reason == "budget_exhausted"
                    || omitted.reason == "structured_budget_exhausted"),
            "omissions must be reported"
        );
    }

    #[test]
    fn included_text_is_verbatim_and_protected_spans_are_reported() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        conn.execute(
            "UPDATE sessions SET notes = 'Sertralin auf 100 mg erhöht am 04.03.2026. \
             Keine Suizidalität.' WHERE id = ?1",
            [&patient.session_id],
        )
        .unwrap();

        let result = assembled(
            &conn,
            &patient.patient_id,
            "Wurde die Sertralin Dosis erhöht?",
            &AssemblyConfig::default(),
        );

        assert!(result
            .evidence
            .contains("Sertralin auf 100 mg erhöht am 04.03.2026"));
        assert!(result.evidence.contains("Keine Suizidalität"));

        let kinds: Vec<ProtectedKind> = result
            .manifest
            .protected_spans_retained
            .iter()
            .map(|count| count.kind)
            .collect();
        for expected in [
            ProtectedKind::Medication,
            ProtectedKind::Dose,
            ProtectedKind::Date,
            ProtectedKind::Negation,
            ProtectedKind::Risk,
        ] {
            assert!(kinds.contains(&expected), "missing protection {expected:?}");
        }
    }

    #[test]
    fn manifest_contains_no_record_text() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        conn.execute(
            "UPDATE sessions SET notes = 'Geheimnisvolleraussage im Protokoll' WHERE id = ?1",
            [&patient.session_id],
        )
        .unwrap();

        let result = assembled(
            &conn,
            &patient.patient_id,
            "Was wurde im Protokoll festgehalten?",
            &AssemblyConfig::default(),
        );
        assert!(result.evidence.contains("Geheimnisvolleraussage"));

        let json = serde_json::to_string(&result.manifest).unwrap();
        assert!(
            !json.contains("Geheimnisvolleraussage"),
            "the manifest must reference record text by digest, never carry it"
        );
    }

    #[test]
    fn manifest_round_trips_through_storage() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let result = assembled(
            &conn,
            &patient.patient_id,
            "Schlaf",
            &AssemblyConfig::default(),
        );

        store_manifest(&conn, &result.manifest).unwrap();
        let loaded = latest_manifest(&conn, &patient.patient_id)
            .unwrap()
            .unwrap();

        assert_eq!(loaded.manifest_id, result.manifest.manifest_id);
        assert_eq!(loaded.patient_revision, result.manifest.patient_revision);
        assert_eq!(loaded.prompt_tokens, result.manifest.prompt_tokens);
        assert_eq!(loaded.tier_tokens, result.manifest.tier_tokens);
        assert_eq!(loaded.omitted, result.manifest.omitted);
        assert_eq!(loaded.retrieval, result.manifest.retrieval);
        let loaded_entries: Vec<_> = loaded
            .entries
            .iter()
            .map(|entry| {
                (
                    &entry.citation,
                    &entry.unit_id,
                    &entry.revision,
                    &entry.text_sha256,
                )
            })
            .collect();
        let original_entries: Vec<_> = result
            .manifest
            .entries
            .iter()
            .map(|entry| {
                (
                    &entry.citation,
                    &entry.unit_id,
                    &entry.revision,
                    &entry.text_sha256,
                )
            })
            .collect();
        assert_eq!(loaded_entries, original_entries);

        assert!(latest_manifest(&conn, "unknown-patient").unwrap().is_none());
    }

    #[test]
    fn citations_are_extracted_from_answers() {
        assert_eq!(
            extract_citations("Laut [E3] und E12 stabil, siehe auch [E3]."),
            vec!["E3".to_string(), "E12".to_string()]
        );
        assert!(extract_citations("Keine Belege").is_empty());
        assert!(extract_citations("EKG unauffällig").is_empty());
    }

    #[test]
    fn answer_audit_traces_cited_spans_to_current_revisions() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let result = assembled(
            &conn,
            &patient.patient_id,
            "Wie ist der Schlaf?",
            &AssemblyConfig::default(),
        );

        let verbatim = result
            .manifest
            .entries
            .iter()
            .find(|entry| entry.record_kind == RecordKind::Session)
            .expect("a session entry")
            .clone();
        let answer = format!("Der Schlaf ist gebessert [{}].", verbatim.citation);

        let audit = audit_answer(&conn, &result.manifest, &answer).unwrap();
        assert!(audit.all_citations_traceable());
        assert_eq!(audit.cited_entries, 1);
        assert!(audit.uncited_entries > 0);

        // Editing the source makes the cited span stale rather than silently ok.
        conn.execute(
            "UPDATE sessions SET notes = 'Komplett neu formulierte Notizen' WHERE id = ?1",
            [&patient.session_id],
        )
        .unwrap();
        let after_edit = audit_answer(&conn, &result.manifest, &answer).unwrap();
        assert!(!after_edit.all_citations_traceable());
        assert_eq!(after_edit.stale_citations, vec![verbatim.citation]);
    }

    #[test]
    fn answer_audit_flags_invented_citations() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let result = assembled(
            &conn,
            &patient.patient_id,
            "Schlaf",
            &AssemblyConfig::default(),
        );

        let audit = audit_answer(&conn, &result.manifest, "Belegt durch [E9999].").unwrap();
        assert_eq!(audit.unsupported_citations, vec!["E9999".to_string()]);
        assert!(!audit.all_citations_traceable());
    }

    #[test]
    fn cold_evidence_is_pointed_at_when_not_included() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        // An old session with content unrelated to the question stays cold.
        conn.execute(
            "INSERT INTO sessions (id, patient_id, session_date, session_type, notes) \
             VALUES ('old-session', ?1, '2019-04-01', 'Erstgespräch', \
                     'Anamnese: Kindheit in Bern, Ausbildung abgeschlossen.')",
            [&patient.patient_id],
        )
        .unwrap();

        let result = assembled(
            &conn,
            &patient.patient_id,
            "Wie ist der Schlaf aktuell?",
            &AssemblyConfig::default(),
        );

        assert!(result.evidence.contains("NICHT ENTHALTEN, ABER VORHANDEN"));
        assert!(result
            .manifest
            .omitted
            .iter()
            .any(|omitted| omitted.reason == "cold_pointer_only"));
        // The pointer names the date but not the content.
        assert!(result.evidence.contains("2019-04-01"));
    }
}
