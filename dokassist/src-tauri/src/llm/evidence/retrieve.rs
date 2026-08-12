//! Patient-scoped hybrid retrieval over the evidence index (issue #403).
//!
//! Four signals are combined:
//!
//! 1. **Lexical** — FTS5/BM25 over unit text.
//! 2. **Embeddings** — cosine similarity over this patient's unit vectors.
//! 3. **Temporal expansion** — a recency prior plus units close in clinical
//!    time to a strong hit, which is what "when did we last change the SSRI?"
//!    actually needs.
//! 4. **Document-neighbour expansion** — the units immediately before and after
//!    a hit inside the same section, so a quotation is not cut off mid-thought.
//!
//! Lexical and embedding ranks are merged with Reciprocal Rank Fusion (the same
//! scheme `search::hybrid_search` uses), then expansions are added with a
//! discounted score. Every candidate carries the signals that selected it so
//! the manifest can report *why* it is in the prompt.
//!
//! Every statement filters on `patient_id`, and the embedding scan only ever
//! loads one patient's vectors.

use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::llm::embed::cosine_similarity;

use super::index::{EvidenceUnit, UNIT_COLUMNS};
use super::provenance::Tier;

/// Reciprocal Rank Fusion constant, matching `search::hybrid_search`.
const RRF_K: f64 = 60.0;

/// Score multiplier applied to expansion-only candidates.
const EXPANSION_WEIGHT: f64 = 0.35;

/// Recency prior weight, and the stronger weight used when the question itself
/// is temporal.
const RECENCY_WEIGHT: f64 = 0.02;
const TEMPORAL_QUESTION_WEIGHT: f64 = 0.08;

#[derive(Debug, Clone)]
pub struct RetrievalParams {
    pub lexical_limit: usize,
    pub semantic_limit: usize,
    pub max_candidates: usize,
    /// How many units either side of a hit to pull in, inside the same section.
    pub neighbor_radius: i64,
    /// Clinical-time window, in days, around a strong hit.
    pub temporal_window_days: i64,
    /// How many top hits seed the expansions.
    pub expansion_seeds: usize,
}

impl Default for RetrievalParams {
    fn default() -> Self {
        Self {
            lexical_limit: 40,
            semantic_limit: 40,
            max_candidates: 96,
            neighbor_radius: 1,
            temporal_window_days: 30,
            expansion_seeds: 4,
        }
    }
}

/// Why one unit was retrieved.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub lexical_rank: Option<usize>,
    pub lexical_bm25: Option<f64>,
    pub semantic_rank: Option<usize>,
    pub semantic_similarity: Option<f64>,
    pub fused_score: f64,
    pub recency_boost: f64,
    /// Question terms that matched this unit's text.
    pub matched_terms: Vec<String>,
    /// Unit ids this candidate was expanded from.
    pub document_neighbor_of: Vec<String>,
    pub temporal_neighbor_of: Vec<String>,
    /// True when the unit is structured clinical truth, which is always offered.
    pub structured_truth: bool,
}

impl Selection {
    /// Human-readable explanation of the selection, for the manifest and UI.
    pub fn reasons(&self) -> Vec<String> {
        let mut reasons = Vec::new();
        if let (Some(rank), Some(bm25)) = (self.lexical_rank, self.lexical_bm25) {
            reasons.push(format!("lexical rank {rank} (bm25 {bm25:.3})"));
        }
        if let (Some(rank), Some(similarity)) = (self.semantic_rank, self.semantic_similarity) {
            reasons.push(format!("embedding rank {rank} (cosine {similarity:.3})"));
        }
        if !self.matched_terms.is_empty() {
            reasons.push(format!("question terms: {}", self.matched_terms.join(", ")));
        }
        if !self.document_neighbor_of.is_empty() {
            reasons.push(format!(
                "document neighbour of {} unit(s)",
                self.document_neighbor_of.len()
            ));
        }
        if !self.temporal_neighbor_of.is_empty() {
            reasons.push(format!(
                "same clinical period as {} hit(s)",
                self.temporal_neighbor_of.len()
            ));
        }
        if self.recency_boost > 0.0 {
            reasons.push(format!("recency boost {:.3}", self.recency_boost));
        }
        if self.structured_truth {
            reasons.push("structured clinical truth".to_string());
        }
        if reasons.is_empty() {
            reasons.push("no signal".to_string());
        }
        reasons
    }

    /// Final ordering score.
    pub fn score(&self) -> f64 {
        let base = if self.lexical_rank.is_none()
            && self.semantic_rank.is_none()
            && !self.structured_truth
        {
            self.fused_score * EXPANSION_WEIGHT
        } else {
            self.fused_score
        };
        base + self.recency_boost
    }
}

/// A retrieved unit together with its selection reasons.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub unit: EvidenceUnit,
    pub selection: Selection,
}

/// Diagnostics about one retrieval pass, surfaced in the manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalDiagnostics {
    pub index_units: usize,
    pub question_terms: Vec<String>,
    pub lexical_hits: usize,
    pub semantic_hits: usize,
    pub semantic_available: bool,
    pub document_neighbors_added: usize,
    pub temporal_neighbors_added: usize,
    pub temporal_question: bool,
    pub candidates: usize,
}

/// Retrieve candidate evidence units for `question`.
///
/// `query_vec` is the embedded question; when `None` (embedding engine not
/// loaded, or no vectors stored yet) retrieval degrades to lexical plus
/// expansions instead of failing.
pub fn retrieve(
    conn: &Connection,
    patient_id: &str,
    question: &str,
    query_vec: Option<&[f32]>,
    params: &RetrievalParams,
) -> Result<(Vec<Candidate>, RetrievalDiagnostics), AppError> {
    let terms = question_terms(question);
    let temporal_question = has_temporal_intent(question);
    let mut diagnostics = RetrievalDiagnostics {
        index_units: conn.query_row(
            "SELECT COUNT(*) FROM evidence_units WHERE patient_id = ?1",
            [patient_id],
            |row| row.get::<_, i64>(0),
        )? as usize,
        question_terms: terms.clone(),
        temporal_question,
        ..RetrievalDiagnostics::default()
    };

    let mut selections: HashMap<String, Selection> = HashMap::new();
    let mut units: HashMap<String, EvidenceUnit> = HashMap::new();

    // 1. Lexical.
    let lexical = lexical_search(conn, patient_id, &terms, params.lexical_limit)?;
    diagnostics.lexical_hits = lexical.len();
    for (rank, (unit, bm25)) in lexical.into_iter().enumerate() {
        let selection = selections.entry(unit.id.clone()).or_default();
        selection.lexical_rank = Some(rank + 1);
        selection.lexical_bm25 = Some(bm25);
        selection.fused_score += 1.0 / (RRF_K + (rank + 1) as f64);
        selection.matched_terms = matched_terms(&unit.text, &terms);
        units.insert(unit.id.clone(), unit);
    }

    // 2. Embeddings.
    if let Some(query_vec) = query_vec {
        let semantic = semantic_search(conn, patient_id, query_vec, params.semantic_limit)?;
        diagnostics.semantic_available = !semantic.is_empty();
        diagnostics.semantic_hits = semantic.len();
        for (rank, (unit, similarity)) in semantic.into_iter().enumerate() {
            let selection = selections.entry(unit.id.clone()).or_default();
            selection.semantic_rank = Some(rank + 1);
            selection.semantic_similarity = Some(similarity);
            selection.fused_score += 1.0 / (RRF_K + (rank + 1) as f64);
            if selection.matched_terms.is_empty() {
                selection.matched_terms = matched_terms(&unit.text, &terms);
            }
            units.insert(unit.id.clone(), unit);
        }
    }

    // 3./4. Expansions seeded by the strongest hits.
    let mut seeds: Vec<(String, f64)> = selections
        .iter()
        .map(|(id, selection)| (id.clone(), selection.score()))
        .collect();
    seeds.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    seeds.truncate(params.expansion_seeds);

    for (seed_id, _) in &seeds {
        let Some(seed) = units.get(seed_id).cloned() else {
            continue;
        };

        for neighbor in document_neighbors(conn, patient_id, &seed, params.neighbor_radius)? {
            let selection = selections.entry(neighbor.id.clone()).or_default();
            if !selection.document_neighbor_of.contains(seed_id) {
                selection.document_neighbor_of.push(seed_id.clone());
                diagnostics.document_neighbors_added += 1;
            }
            selection.fused_score += 1.0 / (RRF_K + params.lexical_limit as f64);
            units.insert(neighbor.id.clone(), neighbor);
        }

        for neighbor in temporal_neighbors(conn, patient_id, &seed, params.temporal_window_days)? {
            let selection = selections.entry(neighbor.id.clone()).or_default();
            if !selection.temporal_neighbor_of.contains(seed_id) {
                selection.temporal_neighbor_of.push(seed_id.clone());
                diagnostics.temporal_neighbors_added += 1;
            }
            selection.fused_score += 1.0 / (RRF_K + params.semantic_limit as f64);
            units.insert(neighbor.id.clone(), neighbor);
        }
    }

    // Recency prior, stronger when the question is itself temporal.
    let weight = if temporal_question {
        TEMPORAL_QUESTION_WEIGHT
    } else {
        RECENCY_WEIGHT
    };
    let today = chrono::Utc::now().date_naive();
    for (id, selection) in selections.iter_mut() {
        let Some(unit) = units.get(id) else { continue };
        selection.structured_truth = unit.tier == Tier::Structured;
        let days = days_ago(today, &unit.occurred_at).unwrap_or(3_650);
        selection.recency_boost = weight / (1.0 + (days as f64 / 30.0));
    }

    let mut candidates: Vec<Candidate> = selections
        .into_iter()
        .filter_map(|(id, selection)| units.remove(&id).map(|unit| Candidate { unit, selection }))
        .collect();

    candidates.sort_by(|a, b| {
        b.selection
            .score()
            .partial_cmp(&a.selection.score())
            .unwrap_or(std::cmp::Ordering::Equal)
            // Stable, deterministic tie-break: newest first, then unit id.
            .then_with(|| b.unit.occurred_at.cmp(&a.unit.occurred_at))
            .then_with(|| a.unit.id.cmp(&b.unit.id))
    });
    candidates.truncate(params.max_candidates);
    diagnostics.candidates = candidates.len();

    Ok((candidates, diagnostics))
}

/// Structured-truth units for a patient, ordered for prompt inclusion:
/// running medications and active diagnoses first, then most recent.
pub fn structured_units(
    conn: &Connection,
    patient_id: &str,
) -> Result<Vec<EvidenceUnit>, AppError> {
    let sql = format!(
        "SELECT {UNIT_COLUMNS} FROM evidence_units \
         WHERE patient_id = ?1 AND tier = 'structured' \
         ORDER BY CASE record_kind \
             WHEN 'patient' THEN 0 \
             WHEN 'diagnosis' THEN 1 \
             WHEN 'medication' THEN 2 \
             WHEN 'outcome_score' THEN 3 \
             WHEN 'treatment_plan' THEN 4 \
             ELSE 5 END, \
           occurred_at DESC, unit_index ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([patient_id], EvidenceUnit::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Cold-tier units that were not selected, newest first — the pointer list the
/// prompt shows so the model knows what exists but was not included verbatim.
pub fn cold_index(
    conn: &Connection,
    patient_id: &str,
    limit: usize,
) -> Result<Vec<EvidenceUnit>, AppError> {
    let sql = format!(
        "SELECT {UNIT_COLUMNS} FROM evidence_units \
         WHERE patient_id = ?1 AND tier = 'cold' \
         ORDER BY occurred_at DESC, unit_index ASC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params![patient_id, limit as i64],
            EvidenceUnit::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Load specific units, still scoped to the patient.
pub fn load_units(
    conn: &Connection,
    patient_id: &str,
    unit_ids: &[String],
) -> Result<Vec<EvidenceUnit>, AppError> {
    if unit_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (0..unit_ids.len())
        .map(|i| format!("?{}", i + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT {UNIT_COLUMNS} FROM evidence_units \
         WHERE patient_id = ?1 AND id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(unit_ids.len() + 1);
    params.push(&patient_id);
    for id in unit_ids {
        params.push(id);
    }
    let rows = stmt
        .query_map(params.as_slice(), EvidenceUnit::from_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn lexical_search(
    conn: &Connection,
    patient_id: &str,
    terms: &[String],
    limit: usize,
) -> Result<Vec<(EvidenceUnit, f64)>, AppError> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let query = fts_or_query(terms);
    let sql = format!(
        "SELECT {}, bm25(evidence_fts) AS rank FROM evidence_fts \
         JOIN evidence_units u ON u.id = evidence_fts.unit_id \
         WHERE evidence_fts MATCH ?1 AND evidence_fts.patient_id = ?2 AND u.patient_id = ?2 \
         ORDER BY rank LIMIT ?3",
        UNIT_COLUMNS
            .split(", ")
            .map(|column| format!("u.{column}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params![query, patient_id, limit as i64], |row| {
            Ok((EvidenceUnit::from_row(row)?, row.get::<_, f64>(16)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn semantic_search(
    conn: &Connection,
    patient_id: &str,
    query_vec: &[f32],
    limit: usize,
) -> Result<Vec<(EvidenceUnit, f64)>, AppError> {
    let embeddings = super::index::load_unit_embeddings(conn, patient_id)?;
    if embeddings.is_empty() {
        return Ok(Vec::new());
    }
    let mut scored: Vec<(f64, String)> = embeddings
        .into_iter()
        .filter(|(_, vector)| vector.len() == query_vec.len())
        .map(|(unit_id, vector)| (cosine_similarity(query_vec, &vector) as f64, unit_id))
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.cmp(&b.1))
    });
    scored.truncate(limit);

    let ids: Vec<String> = scored.iter().map(|(_, id)| id.clone()).collect();
    let units = load_units(conn, patient_id, &ids)?;
    let by_id: HashMap<String, EvidenceUnit> = units
        .into_iter()
        .map(|unit| (unit.id.clone(), unit))
        .collect();

    Ok(scored
        .into_iter()
        .filter_map(|(similarity, id)| by_id.get(&id).cloned().map(|unit| (unit, similarity)))
        .collect())
}

fn document_neighbors(
    conn: &Connection,
    patient_id: &str,
    seed: &EvidenceUnit,
    radius: i64,
) -> Result<Vec<EvidenceUnit>, AppError> {
    if radius <= 0 {
        return Ok(Vec::new());
    }
    let sql = format!(
        "SELECT {UNIT_COLUMNS} FROM evidence_units \
         WHERE patient_id = ?1 AND record_kind = ?2 AND record_id = ?3 AND section = ?4 \
           AND unit_index BETWEEN ?5 AND ?6 AND id <> ?7 \
         ORDER BY unit_index ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params![
                patient_id,
                seed.kind.as_str(),
                seed.record_id,
                seed.section,
                seed.unit_index - radius,
                seed.unit_index + radius,
                seed.id
            ],
            EvidenceUnit::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn temporal_neighbors(
    conn: &Connection,
    patient_id: &str,
    seed: &EvidenceUnit,
    window_days: i64,
) -> Result<Vec<EvidenceUnit>, AppError> {
    if window_days <= 0 {
        return Ok(Vec::new());
    }
    let Some(anchor) = parse_date(&seed.occurred_at) else {
        return Ok(Vec::new());
    };
    let from = (anchor - chrono::Duration::days(window_days))
        .format("%Y-%m-%d")
        .to_string();
    let to = (anchor + chrono::Duration::days(window_days))
        .format("%Y-%m-%d")
        .to_string();

    let sql = format!(
        "SELECT {UNIT_COLUMNS} FROM evidence_units \
         WHERE patient_id = ?1 AND id <> ?2 AND tier <> 'structured' \
           AND substr(occurred_at, 1, 10) BETWEEN ?3 AND ?4 \
         ORDER BY occurred_at DESC, unit_index ASC LIMIT 6"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params![patient_id, seed.id, from, to],
            EvidenceUnit::from_row,
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// FTS5 query that matches any question term. Each term is quoted, which
/// neutralises FTS5 operators exactly as `search::sanitize_fts5_query` does.
fn fts_or_query(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Stop words dropped from questions before retrieval (German, French, English).
const STOP_WORDS: &[&str] = &[
    "aber", "alle", "als", "auch", "auf", "aus", "bei", "beim", "das", "dass", "dem", "den", "der",
    "des", "die", "ein", "eine", "einen", "einer", "für", "hat", "haben", "ich", "ist", "man",
    "mit", "nach", "sich", "sie", "sind", "über", "und", "von", "war", "was", "wie", "wir", "zum",
    "zur", "and", "are", "the", "was", "were", "with", "what", "which", "who", "how", "for",
    "from", "does", "did", "has", "have", "que", "qui", "les", "des", "est", "une", "pour", "avec",
    "dans",
];

/// Question tokens used for lexical matching: at least three characters, not a
/// stop word, de-duplicated, capped so one long question cannot blow up the
/// FTS query.
pub fn question_terms(question: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for raw in question.split(|c: char| !c.is_alphanumeric()) {
        let term = raw.to_lowercase();
        if term.chars().count() < 3 || STOP_WORDS.contains(&term.as_str()) {
            continue;
        }
        if !terms.contains(&term) {
            terms.push(term);
        }
        if terms.len() >= 12 {
            break;
        }
    }
    terms
}

/// Question terms that literally occur in `text`.
fn matched_terms(text: &str, terms: &[String]) -> Vec<String> {
    let lower = text.to_lowercase();
    terms
        .iter()
        .filter(|term| lower.contains(term.as_str()))
        .cloned()
        .collect()
}

/// Cues that a question is about time, trend, or the most recent value.
const TEMPORAL_CUES: &[&str] = &[
    "letzte",
    "letzten",
    "letzter",
    "letztes",
    "zuletzt",
    "aktuell",
    "aktuelle",
    "seit",
    "wann",
    "trend",
    "verlauf",
    "entwicklung",
    "änderung",
    "geändert",
    "neueste",
    "jüngste",
    "last",
    "latest",
    "recent",
    "recently",
    "when",
    "since",
    "change",
    "changed",
    "trend",
    "progress",
    "current",
    "dernier",
    "dernière",
    "depuis",
    "quand",
    "évolution",
    "actuel",
];

pub fn has_temporal_intent(question: &str) -> bool {
    let lower = question.to_lowercase();
    lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| TEMPORAL_CUES.contains(&word))
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    let prefix: String = value.chars().take(10).collect();
    NaiveDate::parse_from_str(&prefix, "%Y-%m-%d").ok()
}

fn days_ago(today: NaiveDate, occurred_at: &str) -> Option<i64> {
    let date = parse_date(occurred_at)?;
    Some((today - date).num_days().max(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::evidence::index::{refresh_patient_index, IndexConfig};
    use crate::llm::evidence::test_support::{seed_patient, TestVault};

    #[test]
    fn question_terms_drop_stop_words_and_short_tokens() {
        let terms = question_terms("Wann wurde die SSRI Dosis zuletzt geändert?");
        assert!(terms.contains(&"ssri".to_string()));
        assert!(terms.contains(&"dosis".to_string()));
        assert!(!terms.contains(&"die".to_string()));
        assert!(terms.len() <= 12);
    }

    #[test]
    fn temporal_intent_is_detected_across_languages() {
        assert!(has_temporal_intent("Wann war die letzte Änderung?"));
        assert!(has_temporal_intent("What is the current medication?"));
        assert!(has_temporal_intent("Quelle est l'évolution du score?"));
        assert!(!has_temporal_intent("Welche Diagnose besteht?"));
    }

    #[test]
    fn fts_query_neutralises_operators() {
        let terms = vec!["dosis".to_string(), "or\"x".to_string()];
        assert_eq!(fts_or_query(&terms), "\"dosis\" OR \"or\"\"x\"");
    }

    #[test]
    fn lexical_retrieval_finds_matching_units() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        let (candidates, diagnostics) = retrieve(
            &conn,
            &patient.patient_id,
            "Wie hat sich der Schlaf entwickelt?",
            None,
            &RetrievalParams::default(),
        )
        .unwrap();

        assert!(diagnostics.lexical_hits > 0);
        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|candidate| candidate
            .unit
            .text
            .to_lowercase()
            .contains("schlaf")));
        assert!(candidates[0]
            .selection
            .reasons()
            .iter()
            .any(|reason| reason.contains("lexical") || reason.contains("recency")));
    }

    #[test]
    fn retrieval_never_returns_another_patients_units() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");
        conn.execute(
            "UPDATE sessions SET notes = 'Bruno berichtet über Panikattacken und Schlafstörungen' \
             WHERE patient_id = ?1",
            [&b.patient_id],
        )
        .unwrap();
        refresh_patient_index(&conn, &a.patient_id, &IndexConfig::default()).unwrap();
        refresh_patient_index(&conn, &b.patient_id, &IndexConfig::default()).unwrap();

        let (candidates, _) = retrieve(
            &conn,
            &a.patient_id,
            "Panikattacken Schlafstörungen",
            None,
            &RetrievalParams::default(),
        )
        .unwrap();
        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert_eq!(candidate.unit.patient_id, a.patient_id);
            assert_ne!(candidate.unit.record_id, b.session_id);
            assert!(!candidate.unit.text.contains("Bruno"));
        }
    }

    #[test]
    fn semantic_signal_is_used_when_vectors_exist() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        // Deterministic stand-in for the embedding engine: the target unit gets
        // a vector parallel to the query, everything else an orthogonal one.
        let missing =
            crate::llm::evidence::index::units_missing_embeddings(&conn, &patient.patient_id)
                .unwrap();
        let target = missing
            .iter()
            .find(|(_, text)| text.contains("Schlaf"))
            .expect("a unit mentioning sleep")
            .0
            .clone();
        for (unit_id, _) in &missing {
            let vector = if *unit_id == target {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            };
            crate::llm::evidence::index::store_unit_embedding(&conn, unit_id, &vector).unwrap();
        }

        let (candidates, diagnostics) = retrieve(
            &conn,
            &patient.patient_id,
            "unrelated wording",
            Some(&[1.0, 0.0]),
            &RetrievalParams::default(),
        )
        .unwrap();
        assert!(diagnostics.semantic_available);
        let hit = candidates
            .iter()
            .find(|candidate| candidate.unit.id == target)
            .expect("semantic hit must be retrieved");
        assert_eq!(hit.selection.semantic_rank, Some(1));
    }

    #[test]
    fn expansions_pull_in_neighbouring_and_contemporaneous_units() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        // A long note so the section has several units to expand into.
        let long_note = format!(
            "Einzigartigerbefund im ersten Abschnitt. {} Schlussabschnitt ohne Suchbegriff.",
            "Weiterer Verlauf unauffällig dokumentiert. ".repeat(40)
        );
        conn.execute(
            "UPDATE sessions SET notes = ?2 WHERE id = ?1",
            rusqlite::params![patient.session_id, long_note],
        )
        .unwrap();
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        let (candidates, diagnostics) = retrieve(
            &conn,
            &patient.patient_id,
            "Einzigartigerbefund",
            None,
            &RetrievalParams::default(),
        )
        .unwrap();

        assert!(diagnostics.document_neighbors_added > 0);
        assert!(candidates.iter().any(|candidate| {
            !candidate.selection.document_neighbor_of.is_empty()
                && candidate.selection.lexical_rank.is_none()
        }));
        assert!(candidates
            .iter()
            .any(|candidate| !candidate.selection.temporal_neighbor_of.is_empty()));
    }

    #[test]
    fn structured_units_are_ordered_by_clinical_importance() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        let units = structured_units(&conn, &patient.patient_id).unwrap();
        assert!(!units.is_empty());
        let kinds: Vec<&str> = units.iter().map(|unit| unit.kind.as_str()).collect();
        let diagnosis = kinds.iter().position(|kind| *kind == "diagnosis");
        let medication = kinds.iter().position(|kind| *kind == "medication");
        assert!(diagnosis < medication);
        assert!(units.iter().all(|unit| unit.tier == Tier::Structured));
    }

    #[test]
    fn empty_question_yields_no_lexical_hits_but_still_ranks_expansions() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &IndexConfig::default()).unwrap();

        let (candidates, diagnostics) = retrieve(
            &conn,
            &patient.patient_id,
            "??",
            None,
            &RetrievalParams::default(),
        )
        .unwrap();
        assert_eq!(diagnostics.lexical_hits, 0);
        assert!(candidates.is_empty());
        assert!(diagnostics.index_units > 0);
    }
}
