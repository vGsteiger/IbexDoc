//! Patient-scoped evidence index (issue #403).
//!
//! The index is a derived artifact: every unit records the revision of the
//! source section it was cut from, and [`refresh_patient_index`] rebuilds only
//! the sections whose revision changed. Units of a changed section — plus their
//! embeddings and, for patient files, the older `document_chunks` derived from
//! the same text — are deleted before the new units are written, so no derived
//! row can outlive the revision it was derived from.

use std::collections::{HashMap, HashSet};

use chrono::{Duration, Utc};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::protect::{self, ProtectedSpan, ProtectionLexicon};
use super::provenance::{self, RecordKind, SourceDoc, Tier};
use super::tokens::estimate_tokens;

/// How verbatim sections are cut into units and which of them count as hot.
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Preferred unit size in characters.
    pub target_chars: usize,
    /// Hard preference for the largest unit; only exceeded when a protected
    /// span would otherwise be cut in half.
    pub max_chars: usize,
    /// Verbatim text at least this recent is hot.
    pub hot_window_days: i64,
    /// The newest sessions are always hot, even outside the window, so a
    /// dormant record still offers recent verbatim evidence.
    pub min_hot_sessions: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            target_chars: 700,
            max_chars: 1_100,
            hot_window_days: 180,
            min_hot_sessions: 3,
        }
    }
}

/// What one refresh changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStats {
    pub sources_scanned: usize,
    pub sources_reindexed: usize,
    pub sources_removed: usize,
    pub units_inserted: usize,
    pub units_removed: usize,
    /// `document_chunks` rows dropped because their source file text changed.
    pub stale_chunks_removed: usize,
    pub units_total: usize,
    /// Units without a current embedding (retrieval degrades to lexical for
    /// these until [`store_unit_embedding`] is called).
    pub units_missing_embeddings: usize,
}

/// A stored evidence unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceUnit {
    pub id: String,
    pub patient_id: String,
    pub kind: RecordKind,
    pub record_id: String,
    pub section: String,
    pub revision: String,
    pub tier: Tier,
    pub unit_index: i64,
    pub char_start: usize,
    pub char_end: usize,
    pub occurred_at: String,
    pub source_updated_at: String,
    pub label: String,
    pub text: String,
    pub token_estimate: usize,
    pub protected: Vec<ProtectedSpan>,
}

impl EvidenceUnit {
    pub(super) fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        let kind: String = row.get(2)?;
        let tier: String = row.get(6)?;
        let protected_json: String = row.get(14)?;
        Ok(Self {
            id: row.get(0)?,
            patient_id: row.get(1)?,
            kind: RecordKind::parse(&kind).map_err(to_sqlite_error)?,
            record_id: row.get(3)?,
            section: row.get(4)?,
            revision: row.get(5)?,
            tier: Tier::parse(&tier).map_err(to_sqlite_error)?,
            unit_index: row.get(7)?,
            char_start: row.get::<_, i64>(8)? as usize,
            char_end: row.get::<_, i64>(9)? as usize,
            occurred_at: row.get(10)?,
            source_updated_at: row.get(11)?,
            label: row.get(12)?,
            text: row.get(13)?,
            token_estimate: row.get::<_, i64>(15)? as usize,
            protected: serde_json::from_str(&protected_json).unwrap_or_default(),
        })
    }
}

fn to_sqlite_error(err: AppError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
}

pub(super) const UNIT_COLUMNS: &str = "id, patient_id, record_kind, record_id, section, revision, \
     tier, unit_index, char_start, char_end, occurred_at, source_updated_at, label, text, \
     protected_json, token_estimate";

/// Bring the evidence index for one patient up to date with the record.
///
/// Cheap when nothing changed: one query per source table plus one revision
/// comparison per section.
pub fn refresh_patient_index(
    conn: &Connection,
    patient_id: &str,
    config: &IndexConfig,
) -> Result<IndexStats, AppError> {
    let docs = provenance::collect_sources(conn, patient_id)?;
    let cutoff = hot_cutoff(&docs, config);
    let lexicon = ProtectionLexicon::for_patient(conn, patient_id)?;

    let mut stats = IndexStats {
        sources_scanned: docs.len(),
        ..IndexStats::default()
    };

    let indexed = load_indexed_revisions(conn, patient_id)?;
    let mut live: HashSet<(String, String, String)> = HashSet::new();

    for doc in &docs {
        let key = (
            doc.kind.as_str().to_string(),
            doc.record_id.clone(),
            doc.section.to_string(),
        );
        live.insert(key.clone());

        let revision = doc.revision();
        let tier = doc.tier(&cutoff);

        if indexed.get(&key) == Some(&revision) {
            // Content is unchanged; only the hot/cold classification can drift
            // as the hot window moves.
            conn.execute(
                "UPDATE evidence_units SET tier = ?1 \
                 WHERE patient_id = ?2 AND record_kind = ?3 AND record_id = ?4 AND section = ?5 \
                   AND tier <> ?1",
                rusqlite::params![
                    tier.as_str(),
                    patient_id,
                    doc.kind.as_str(),
                    doc.record_id,
                    doc.section
                ],
            )?;
            continue;
        }

        stats.units_removed += delete_section_units(conn, patient_id, doc)?;
        stats.stale_chunks_removed += invalidate_derived_chunks(conn, patient_id, doc)?;
        let inserted = insert_units(conn, doc, &revision, tier, &lexicon, config)?;
        stats.units_inserted += inserted;
        stats.sources_reindexed += 1;

        conn.execute(
            "INSERT INTO evidence_sources \
                (patient_id, record_kind, record_id, section, revision, unit_count, indexed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
             ON CONFLICT(record_kind, record_id, section) DO UPDATE SET \
                patient_id = excluded.patient_id, revision = excluded.revision, \
                unit_count = excluded.unit_count, indexed_at = excluded.indexed_at",
            rusqlite::params![
                patient_id,
                doc.kind.as_str(),
                doc.record_id,
                doc.section,
                revision,
                inserted as i64,
                Utc::now().to_rfc3339()
            ],
        )?;
    }

    // Sections that disappeared (record deleted, or text cleared).
    for (kind, record_id, section) in indexed.keys() {
        if live.contains(&(kind.clone(), record_id.clone(), section.clone())) {
            continue;
        }
        stats.units_removed += delete_units_by_slot(conn, patient_id, kind, record_id, section)?;
        conn.execute(
            "DELETE FROM evidence_sources \
             WHERE patient_id = ?1 AND record_kind = ?2 AND record_id = ?3 AND section = ?4",
            rusqlite::params![patient_id, kind, record_id, section],
        )?;
        stats.sources_removed += 1;
    }

    stats.units_total = conn.query_row(
        "SELECT COUNT(*) FROM evidence_units WHERE patient_id = ?1",
        [patient_id],
        |row| row.get::<_, i64>(0),
    )? as usize;
    stats.units_missing_embeddings = units_missing_embeddings(conn, patient_id)?.len();

    Ok(stats)
}

/// Delete every derived evidence row for a patient. Used when a caller wants a
/// full rebuild rather than an incremental refresh.
pub fn clear_patient_index(conn: &Connection, patient_id: &str) -> Result<usize, AppError> {
    let removed = conn.execute(
        "DELETE FROM evidence_fts WHERE unit_id IN \
            (SELECT id FROM evidence_units WHERE patient_id = ?1)",
        [patient_id],
    )?;
    conn.execute(
        "DELETE FROM evidence_units WHERE patient_id = ?1",
        [patient_id],
    )?;
    conn.execute(
        "DELETE FROM evidence_sources WHERE patient_id = ?1",
        [patient_id],
    )?;
    Ok(removed)
}

/// Units of this patient that have no embedding for their current revision.
pub fn units_missing_embeddings(
    conn: &Connection,
    patient_id: &str,
) -> Result<Vec<(String, String)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT u.id, u.text FROM evidence_units u \
         LEFT JOIN evidence_unit_embeddings e ON e.unit_id = u.id AND e.revision = u.revision \
         WHERE u.patient_id = ?1 AND e.unit_id IS NULL \
         ORDER BY u.occurred_at DESC",
    )?;
    let rows = stmt
        .query_map([patient_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Store the embedding for one unit, stamped with the unit's current revision
/// so a later edit invalidates it.
pub fn store_unit_embedding(
    conn: &Connection,
    unit_id: &str,
    vector: &[f32],
) -> Result<(), AppError> {
    let blob = crate::llm::embed::vec_to_blob(vector);
    let updated = conn.execute(
        "INSERT INTO evidence_unit_embeddings (unit_id, patient_id, revision, vector) \
         SELECT id, patient_id, revision, ?2 FROM evidence_units WHERE id = ?1 \
         ON CONFLICT(unit_id) DO UPDATE SET \
            vector = excluded.vector, revision = excluded.revision, \
            patient_id = excluded.patient_id, created_at = datetime('now')",
        rusqlite::params![unit_id, blob],
    )?;
    if updated == 0 {
        return Err(AppError::NotFound(format!(
            "Evidence unit {unit_id} no longer exists"
        )));
    }
    Ok(())
}

/// Load the embeddings of one patient's units. Never returns another patient's
/// vectors, and never returns a vector stamped with a superseded revision.
pub fn load_unit_embeddings(
    conn: &Connection,
    patient_id: &str,
) -> Result<Vec<(String, Vec<f32>)>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT e.unit_id, e.vector FROM evidence_unit_embeddings e \
         JOIN evidence_units u ON u.id = e.unit_id \
         WHERE e.patient_id = ?1 AND u.patient_id = ?1 AND e.revision = u.revision",
    )?;
    let rows = stmt
        .query_map([patient_id], |row| {
            let blob: Vec<u8> = row.get(1)?;
            Ok((
                row.get::<_, String>(0)?,
                crate::llm::embed::blob_to_vec(&blob),
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_indexed_revisions(
    conn: &Connection,
    patient_id: &str,
) -> Result<HashMap<(String, String, String), String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT record_kind, record_id, section, revision FROM evidence_sources \
         WHERE patient_id = ?1",
    )?;
    let rows = stmt
        .query_map([patient_id], |row| {
            Ok((
                (row.get(0)?, row.get(1)?, row.get(2)?),
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

fn delete_section_units(
    conn: &Connection,
    patient_id: &str,
    doc: &SourceDoc,
) -> Result<usize, AppError> {
    delete_units_by_slot(
        conn,
        patient_id,
        doc.kind.as_str(),
        &doc.record_id,
        doc.section,
    )
}

fn delete_units_by_slot(
    conn: &Connection,
    patient_id: &str,
    kind: &str,
    record_id: &str,
    section: &str,
) -> Result<usize, AppError> {
    // FTS rows carry no foreign key, so they are removed explicitly first.
    conn.execute(
        "DELETE FROM evidence_fts WHERE unit_id IN \
            (SELECT id FROM evidence_units \
             WHERE patient_id = ?1 AND record_kind = ?2 AND record_id = ?3 AND section = ?4)",
        rusqlite::params![patient_id, kind, record_id, section],
    )?;
    // Embeddings cascade from evidence_units.
    let removed = conn.execute(
        "DELETE FROM evidence_units \
         WHERE patient_id = ?1 AND record_kind = ?2 AND record_id = ?3 AND section = ?4",
        rusqlite::params![patient_id, kind, record_id, section],
    )?;
    Ok(removed)
}

/// Drop `document_chunks` (and their cascaded embeddings) derived from a file
/// whose extracted text has changed.
fn invalidate_derived_chunks(
    conn: &Connection,
    patient_id: &str,
    doc: &SourceDoc,
) -> Result<usize, AppError> {
    if doc.kind != RecordKind::File {
        return Ok(0);
    }
    let removed = conn.execute(
        "DELETE FROM document_chunks WHERE file_id IN \
            (SELECT id FROM files WHERE id = ?1 AND patient_id = ?2)",
        rusqlite::params![doc.record_id, patient_id],
    )?;
    Ok(removed)
}

#[allow(clippy::too_many_arguments)]
fn insert_units(
    conn: &Connection,
    doc: &SourceDoc,
    revision: &str,
    tier: Tier,
    lexicon: &ProtectionLexicon,
    config: &IndexConfig,
) -> Result<usize, AppError> {
    let protected = protect::detect(&doc.text, lexicon);
    let ranges = split_units(&doc.text, &protected, config);

    for (index, (start, end)) in ranges.iter().enumerate() {
        let text = protect::char_slice(&doc.text, *start, *end);
        let spans = protect::spans_within(&protected, *start, *end);
        let unit_id = uuid::Uuid::now_v7().to_string();
        conn.execute(
            "INSERT INTO evidence_units \
                (id, patient_id, record_kind, record_id, section, revision, tier, unit_index, \
                 char_start, char_end, occurred_at, source_updated_at, label, text, \
                 token_estimate, protected_json) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            rusqlite::params![
                unit_id,
                doc.patient_id,
                doc.kind.as_str(),
                doc.record_id,
                doc.section,
                revision,
                tier.as_str(),
                index as i64,
                *start as i64,
                *end as i64,
                doc.occurred_at,
                doc.updated_at,
                doc.label,
                text,
                estimate_tokens(&text) as i64,
                serde_json::to_string(&spans)
                    .map_err(|e| AppError::Llm(format!("Failed to encode protected spans: {e}")))?,
            ],
        )?;
        conn.execute(
            "INSERT INTO evidence_fts (unit_id, patient_id, text) VALUES (?1, ?2, ?3)",
            rusqlite::params![unit_id, doc.patient_id, text],
        )?;
    }

    Ok(ranges.len())
}

/// The date from which verbatim text counts as hot.
fn hot_cutoff(docs: &[SourceDoc], config: &IndexConfig) -> String {
    let window_cutoff = (Utc::now() - Duration::days(config.hot_window_days.max(0)))
        .format("%Y-%m-%d")
        .to_string();

    if config.min_hot_sessions == 0 {
        return window_cutoff;
    }

    let mut session_dates: Vec<&str> = docs
        .iter()
        .filter(|doc| doc.kind == RecordKind::Session)
        .map(|doc| doc.occurred_at.as_str())
        .collect();
    session_dates.sort_unstable();
    session_dates.dedup();

    // With fewer distinct session dates than the minimum, every one of them is
    // kept hot.
    let index = session_dates.len().saturating_sub(config.min_hot_sessions);
    match session_dates.get(index) {
        // Lower the cutoff if needed so the newest sessions stay hot.
        Some(date) if *date < window_cutoff.as_str() => (*date).to_string(),
        _ => window_cutoff,
    }
}

/// Cut `text` into unit ranges of character offsets.
///
/// Cuts land on paragraph or sentence boundaries where possible, on word
/// boundaries otherwise, and never inside a protected span — a unit is allowed
/// to exceed `max_chars` rather than split a dose or a negation.
pub fn split_units(
    text: &str,
    protected: &[ProtectedSpan],
    config: &IndexConfig,
) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let target = config.target_chars.max(1);
    let max = config.max_chars.max(target);

    let (strong, weak) = boundary_candidates(&chars);
    let mut units: Vec<(usize, usize)> = Vec::new();
    let mut start = 0;

    while start < total {
        // Skip leading whitespace so a unit's first character is content.
        while start < total && chars[start].is_whitespace() {
            start += 1;
        }
        if start >= total {
            break;
        }

        let end = if total - start <= max {
            total
        } else {
            let max_end = start + max;
            let min_end = (start + target).min(max_end);
            pick_cut(&strong, protected, start, min_end, max_end)
                .or_else(|| pick_cut(&weak, protected, start, min_end, max_end))
                .or_else(|| pick_cut(&weak, protected, start, start + 1, max_end))
                // Everything up to max_end sits inside one protected span:
                // extend to the end of that span instead of cutting through it.
                .or_else(|| next_safe_offset(protected, max_end, total))
                .unwrap_or(total)
        };

        let mut trimmed_end = end;
        while trimmed_end > start && chars[trimmed_end - 1].is_whitespace() {
            trimmed_end -= 1;
        }
        if trimmed_end > start {
            units.push((start, trimmed_end));
        }
        start = end.max(start + 1);
    }

    units
}

/// The largest candidate inside `[min_end, max_end]` that is a safe cut.
fn pick_cut(
    candidates: &[usize],
    protected: &[ProtectedSpan],
    start: usize,
    min_end: usize,
    max_end: usize,
) -> Option<usize> {
    candidates.iter().copied().rfind(|&at| {
        at > start && at >= min_end && at <= max_end && protect::is_safe_cut(protected, at)
    })
}

fn next_safe_offset(protected: &[ProtectedSpan], from: usize, total: usize) -> Option<usize> {
    (from..=total).find(|&at| protect::is_safe_cut(protected, at))
}

/// Boundary offsets, split into strong (paragraph / sentence ends) and weak
/// (word starts) candidates. Offsets are exclusive ends of a unit.
fn boundary_candidates(chars: &[char]) -> (Vec<usize>, Vec<usize>) {
    let mut strong = Vec::new();
    let mut weak = Vec::new();

    for index in 1..=chars.len() {
        if index == chars.len() {
            strong.push(index);
            break;
        }
        let previous = chars[index - 1];
        let current = chars[index];
        if !previous.is_whitespace() && current.is_whitespace() {
            if matches!(previous, '.' | '!' | '?' | ':' | ';') {
                strong.push(index);
            } else {
                weak.push(index);
            }
        }
        if previous == '\n' && current != '\n' {
            strong.push(index);
        }
    }

    strong.sort_unstable();
    strong.dedup();
    weak.sort_unstable();
    weak.dedup();
    (strong, weak)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::evidence::test_support::{seed_patient, TestVault};
    use crate::llm::evidence::{protect::ProtectedKind, provenance::section};

    fn config() -> IndexConfig {
        IndexConfig::default()
    }

    #[test]
    fn units_cover_the_source_and_offsets_are_exact() {
        let sentences: Vec<String> = (0..40)
            .map(|i| format!("Befund {i}: Patient berichtet über Schlafstörungen seit Wochen."))
            .collect();
        let text = sentences.join(" ");
        let protected = protect::detect(&text, &ProtectionLexicon::builtin());
        let units = split_units(&text, &protected, &config());

        assert!(units.len() > 1, "long text must produce several units");
        for (start, end) in &units {
            let slice = protect::char_slice(&text, *start, *end);
            assert!(!slice.is_empty());
            assert!(!slice.starts_with(char::is_whitespace));
            assert!(!slice.ends_with(char::is_whitespace));
        }
        // Units are ordered and non-overlapping.
        for pair in units.windows(2) {
            assert!(pair[0].1 <= pair[1].0);
        }
        assert_eq!(units[0].0, 0);
    }

    #[test]
    fn units_never_cut_through_a_protected_span() {
        // Force a dose to straddle the preferred cut point.
        let filler = "Verlauf stabil ".repeat(40);
        let text = format!("{filler}Sertralin 150 mg täglich weiter. {filler}");
        let protected = protect::detect(&text, &ProtectionLexicon::builtin());
        let units = split_units(&text, &protected, &config());

        for (start, end) in &units {
            assert!(protect::is_safe_cut(&protected, *start));
            assert!(protect::is_safe_cut(&protected, *end));
        }
        let dose_intact = units
            .iter()
            .any(|(start, end)| protect::char_slice(&text, *start, *end).contains("150 mg"));
        assert!(dose_intact, "the dose must survive inside one unit");
    }

    #[test]
    fn a_single_protected_span_may_exceed_max_chars() {
        let long_dose = format!("{}-0-1", "1".repeat(80));
        let text = format!("Schema {long_dose} beibehalten");
        let protected = vec![ProtectedSpan {
            kind: ProtectedKind::Dose,
            start: 7,
            end: 7 + long_dose.chars().count(),
        }];
        let tight = IndexConfig {
            target_chars: 10,
            max_chars: 20,
            ..config()
        };
        let units = split_units(&text, &protected, &tight);
        assert!(units
            .iter()
            .any(|(start, end)| protect::char_slice(&text, *start, *end).contains(&long_dose)));
    }

    #[test]
    fn refresh_indexes_every_section_once() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let first = refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();
        assert!(first.units_inserted > 0);
        assert_eq!(first.sources_reindexed, first.sources_scanned);

        // A second refresh is a no-op.
        let second = refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();
        assert_eq!(second.sources_reindexed, 0);
        assert_eq!(second.units_inserted, 0);
        assert_eq!(second.units_total, first.units_total);
    }

    #[test]
    fn editing_a_section_replaces_only_its_units() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();

        let unrelated: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE record_kind = 'medication'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "UPDATE sessions SET notes = 'Neue Notizen: Stimmung deutlich gebessert.' WHERE id = ?1",
            [&patient.session_id],
        )
        .unwrap();
        let stats = refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();

        // Both sections of the edited session share its `updated_at`, so the
        // summary may be reindexed alongside the notes — but nothing else is.
        assert!((1..=2).contains(&stats.sources_reindexed));
        assert!(stats.units_removed > 0);
        let text: String = conn
            .query_row(
                "SELECT text FROM evidence_units WHERE record_id = ?1 AND section = ?2",
                rusqlite::params![patient.session_id, section::NOTES],
                |row| row.get(0),
            )
            .unwrap();
        assert!(text.contains("deutlich gebessert"));
        let still_there: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE record_kind = 'medication'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(unrelated, still_there);
    }

    #[test]
    fn deleting_a_record_removes_its_units_and_fts_rows() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();

        conn.execute("DELETE FROM sessions WHERE id = ?1", [&patient.session_id])
            .unwrap();
        let stats = refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();
        assert!(stats.sources_removed > 0);

        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE record_id = ?1",
                [&patient.session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM evidence_fts", [], |row| row.get(0))
            .unwrap();
        let unit_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM evidence_units", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fts_rows, unit_rows);
    }

    #[test]
    fn changed_file_text_invalidates_derived_chunks_and_embeddings() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();

        // Simulate the chunk/embedding artifacts the RAG pipeline derives from
        // the same extracted text.
        conn.execute(
            "INSERT INTO document_chunks (id, file_id, literature_id, chunk_index, content, word_count) \
             VALUES ('chunk-1', ?1, NULL, 0, 'Alter Berichtstext', 3)",
            [&patient.file_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chunk_embeddings (chunk_id, vector) VALUES ('chunk-1', ?1)",
            [crate::llm::embed::vec_to_blob(&[0.1, 0.2])],
        )
        .unwrap();

        let unit_id: String = conn
            .query_row(
                "SELECT id FROM evidence_units WHERE record_id = ?1",
                [&patient.file_id],
                |row| row.get(0),
            )
            .unwrap();
        store_unit_embedding(&conn, &unit_id, &[0.5, 0.5]).unwrap();
        assert_eq!(
            load_unit_embeddings(&conn, &patient.patient_id)
                .unwrap()
                .len(),
            1
        );

        conn.execute(
            "UPDATE files SET extracted_text = 'Neu extrahierter Berichtstext mit anderem Inhalt' \
             WHERE id = ?1",
            [&patient.file_id],
        )
        .unwrap();
        let stats = refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();

        assert_eq!(stats.stale_chunks_removed, 1);
        let chunks: i64 = conn
            .query_row("SELECT COUNT(*) FROM document_chunks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(chunks, 0);
        let chunk_embeddings: i64 = conn
            .query_row("SELECT COUNT(*) FROM chunk_embeddings", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(chunk_embeddings, 0, "chunk embeddings cascade with chunks");
        assert!(
            load_unit_embeddings(&conn, &patient.patient_id)
                .unwrap()
                .is_empty(),
            "unit embeddings must not survive a source edit"
        );
    }

    #[test]
    fn index_rows_never_mix_patients() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");
        refresh_patient_index(&conn, &a.patient_id, &config()).unwrap();
        refresh_patient_index(&conn, &b.patient_id, &config()).unwrap();

        let mismatched: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units u \
                 JOIN evidence_fts f ON f.unit_id = u.id \
                 WHERE f.patient_id <> u.patient_id",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(mismatched, 0);

        let cross: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE patient_id = ?1 AND record_id = ?2",
                rusqlite::params![a.patient_id, b.session_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cross, 0);
    }

    #[test]
    fn hot_cutoff_keeps_newest_sessions_hot() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        // Push every session far into the past, keeping the dates distinct.
        for (index, session_id) in [
            &patient.first_session_id,
            &patient.middle_session_id,
            &patient.session_id,
        ]
        .into_iter()
        .enumerate()
        {
            conn.execute(
                "UPDATE sessions SET session_date = ?2 WHERE id = ?1",
                rusqlite::params![session_id, format!("2015-01-0{}", index + 1)],
            )
            .unwrap();
        }

        refresh_patient_index(&conn, &patient.patient_id, &config()).unwrap();
        let hot: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units \
                 WHERE patient_id = ?1 AND record_kind = 'session' AND tier = 'hot'",
                [&patient.patient_id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(hot > 0, "the newest sessions stay hot even when old");
    }

    #[test]
    fn clear_patient_index_removes_only_that_patient() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");
        refresh_patient_index(&conn, &a.patient_id, &config()).unwrap();
        refresh_patient_index(&conn, &b.patient_id, &config()).unwrap();

        clear_patient_index(&conn, &a.patient_id).unwrap();
        let remaining_a: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE patient_id = ?1",
                [&a.patient_id],
                |row| row.get(0),
            )
            .unwrap();
        let remaining_b: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evidence_units WHERE patient_id = ?1",
                [&b.patient_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_a, 0);
        assert!(remaining_b > 0);
    }
}
