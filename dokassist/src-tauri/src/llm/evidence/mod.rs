//! Provenance-bearing evidence assembly for patient-history RAG (issue #403).
//!
//! This module is the stable interface a patient-history question-answering
//! feature (issue #164) consumes. Callers describe *what* they need — a patient,
//! a question, a token budget — and get back an evidence block plus a manifest.
//! How the evidence is found (lexical, embeddings, temporal or neighbour
//! expansion) is an internal detail that can change without touching callers.
//!
//! ```ignore
//! let request = EvidenceRequest::new(&patient_id, &question).with_token_budget(11_520);
//! let assembled = assemble_patient_evidence(&conn, &request, &HeuristicCounter)?;
//! let prompt = prompts::evidence_query_prompt(&assembled.evidence, &question);
//! // … run inference, then:
//! let audit = audit_answer(&conn, &assembled.manifest, &answer)?;
//! ```
//!
//! Design notes:
//!
//! * **Patient scope.** Every index row, embedding, FTS row and manifest row
//!   carries the patient id, and every query filters on it. A record id alone
//!   never loads text.
//! * **Exact provenance.** A unit stores the source record kind, id, section,
//!   revision and character range. `provenance::resolve_span` re-derives the
//!   text, so a cited span is verifiable against the current record.
//! * **Bounded memory.** The assembler fills a token budget and reports what it
//!   dropped, so a 16K-context model never needs the raw record in its KV cache.
//! * **No lossy summarisation of protected spans.** Units are included whole;
//!   doses, dates, negations, uncertainty, risk statements and provenance
//!   tokens are detected up front and never cut through.

pub mod assemble;
pub mod index;
pub mod protect;
pub mod provenance;
pub mod retrieve;
pub mod tokens;

#[cfg(test)]
mod benchmark;
#[cfg(test)]
pub(crate) mod test_support;

use rusqlite::Connection;

use crate::error::AppError;

// Re-exported so callers depend on `evidence::…` rather than on the internal
// module layout. Types not listed here stay reachable through their module.
pub use assemble::{
    audit_answer, latest_manifest, store_manifest, AnswerAudit, AssembledEvidence, AssemblyConfig,
    EvidenceManifest,
};
pub use index::{refresh_patient_index, IndexConfig, IndexStats};
pub use provenance::SpanResolution;
pub use tokens::{HeuristicCounter, TokenCounter};

/// Tokens reserved for the system prompt, question and answer scaffolding that
/// wraps the evidence block.
pub const PROMPT_SCAFFOLD_TOKENS: usize = 768;

/// Evidence budget for a model context, after reserving completion headroom and
/// prompt scaffolding.
pub fn budget_for_context(context_size: usize, completion_tokens: usize) -> usize {
    context_size.saturating_sub(completion_tokens + PROMPT_SCAFFOLD_TOKENS)
}

/// A request for assembled evidence.
#[derive(Debug, Clone)]
pub struct EvidenceRequest<'a> {
    pub patient_id: &'a str,
    pub question: &'a str,
    /// The embedded question. `None` degrades retrieval to lexical plus
    /// expansions rather than failing.
    pub query_vec: Option<&'a [f32]>,
    pub assembly: AssemblyConfig,
    pub index: IndexConfig,
}

impl<'a> EvidenceRequest<'a> {
    pub fn new(patient_id: &'a str, question: &'a str) -> Self {
        Self {
            patient_id,
            question,
            query_vec: None,
            assembly: AssemblyConfig::default(),
            index: IndexConfig::default(),
        }
    }

    pub fn with_token_budget(mut self, token_budget: usize) -> Self {
        self.assembly.token_budget = token_budget;
        self
    }

    pub fn with_query_vector(mut self, query_vec: &'a [f32]) -> Self {
        self.query_vec = Some(query_vec);
        self
    }
}

/// Refresh the patient's evidence index and assemble a budget-bounded evidence
/// block with its manifest.
///
/// The refresh is incremental: sections whose revision is unchanged are left
/// alone, so repeated questions about the same patient do no indexing work.
pub fn assemble_patient_evidence(
    conn: &Connection,
    request: &EvidenceRequest<'_>,
    counter: &dyn TokenCounter,
) -> Result<AssembledEvidence, AppError> {
    let stats = index::refresh_patient_index(conn, request.patient_id, &request.index)?;
    assemble::assemble(
        conn,
        request.patient_id,
        request.question,
        request.query_vec,
        &request.assembly,
        counter,
        stats,
    )
}

/// Units of this patient that still need an embedding, as `(unit_id, text)`.
///
/// The embedding engine lives in the command layer (it is loaded lazily and may
/// download a model), so callers embed these texts and feed the vectors back
/// through [`index::store_unit_embedding`].
pub fn pending_embeddings(
    conn: &Connection,
    patient_id: &str,
) -> Result<Vec<(String, String)>, AppError> {
    index::units_missing_embeddings(conn, patient_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::evidence::test_support::{seed_patient, TestVault};

    #[test]
    fn budget_leaves_room_for_completion_and_scaffolding() {
        assert_eq!(budget_for_context(16_384, 4_096), 16_384 - 4_096 - 768);
        assert_eq!(budget_for_context(1_024, 4_096), 0);
    }

    #[test]
    fn stable_interface_returns_evidence_and_manifest() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let request = EvidenceRequest::new(&patient.patient_id, "Wie ist der Schlaf aktuell?")
            .with_token_budget(4_000);
        let assembled = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();

        assert!(!assembled.evidence.is_empty());
        assert!(assembled.manifest.prompt_tokens <= 4_000);
        assert_eq!(assembled.manifest.patient_id, patient.patient_id);
        assert!(assembled.manifest.index.units_total > 0);
        assert!(assembled.manifest.patient_revision.starts_with("p1:"));

        // Repeating the request does no indexing work.
        let again = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();
        assert_eq!(again.manifest.index.sources_reindexed, 0);
        assert_eq!(again.evidence, assembled.evidence);
    }

    #[test]
    fn assembly_never_leaks_across_patients() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");
        conn.execute(
            "UPDATE sessions SET notes = 'Bruno: Lithium 900 mg, Panikattacken dokumentiert' \
             WHERE patient_id = ?1",
            [&b.patient_id],
        )
        .unwrap();
        conn.execute(
            "UPDATE patients SET notes = 'Brunos Sonderhinweis' WHERE id = ?1",
            [&b.patient_id],
        )
        .unwrap();

        let request = EvidenceRequest::new(&a.patient_id, "Lithium Panikattacken Sonderhinweis");
        let assembled = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();

        assert!(!assembled.evidence.contains("Bruno"));
        assert!(!assembled.evidence.contains("Lithium"));
        assert!(!assembled.evidence.contains("Sonderhinweis"));
        for entry in &assembled.manifest.entries {
            assert_eq!(entry.patient_id, a.patient_id);
        }
    }

    #[test]
    fn stale_revisions_never_reach_the_prompt() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let request = EvidenceRequest::new(&patient.patient_id, "Welche Dosis ist aktuell?");
        let first = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();
        assert!(first.evidence.contains("100 mg"));

        conn.execute(
            "UPDATE medications SET dosage = '150 mg' WHERE patient_id = ?1",
            [&patient.patient_id],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET notes = 'Sertralin auf 150 mg erhöht, gut verträglich.' \
             WHERE id = ?1",
            [&patient.session_id],
        )
        .unwrap();

        let second = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();
        assert!(second.evidence.contains("150 mg"));
        assert!(
            !second.evidence.contains("100 mg"),
            "superseded dose must not survive in the assembled evidence"
        );
        assert_ne!(
            first.manifest.patient_revision,
            second.manifest.patient_revision
        );

        // Every entry of the fresh manifest resolves against the live record.
        for entry in &second.manifest.entries {
            let resolution = provenance::resolve_span_digest(
                &conn,
                &entry.patient_id,
                entry.record_kind,
                &entry.record_id,
                &entry.section,
                &entry.revision,
                entry.char_start,
                entry.char_end,
                &entry.text_sha256,
            )
            .unwrap();
            assert!(
                resolution.is_traceable(),
                "entry {} is not traceable",
                entry.citation
            );
        }
    }

    #[test]
    fn pending_embeddings_are_reported_and_cleared() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        let pending = pending_embeddings(&conn, &patient.patient_id).unwrap();
        assert!(!pending.is_empty());
        for (unit_id, _) in &pending {
            index::store_unit_embedding(&conn, unit_id, &[0.1, 0.2, 0.3]).unwrap();
        }
        assert!(pending_embeddings(&conn, &patient.patient_id)
            .unwrap()
            .is_empty());
    }
}
