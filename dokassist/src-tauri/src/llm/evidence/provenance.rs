//! Source provenance for assembled evidence (issue #403).
//!
//! Every evidence unit points back to exactly one *(record kind, record id,
//! section)* triple plus a character range inside that section's text, and
//! carries the revision of the source it was cut from. That makes a cited
//! answer span verifiable: reload the section, recompute its revision, and
//! re-slice the same character range.
//!
//! Every query in this module filters on `patient_id`. A record id alone is
//! never enough to load text, so a caller cannot accidentally reach into
//! another patient's record.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::protect::char_slice;

/// The kind of source record an evidence unit came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Patient,
    Session,
    File,
    Diagnosis,
    Medication,
    OutcomeScore,
    TreatmentPlan,
    TreatmentGoal,
    TreatmentIntervention,
}

impl RecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Patient => "patient",
            Self::Session => "session",
            Self::File => "file",
            Self::Diagnosis => "diagnosis",
            Self::Medication => "medication",
            Self::OutcomeScore => "outcome_score",
            Self::TreatmentPlan => "treatment_plan",
            Self::TreatmentGoal => "treatment_goal",
            Self::TreatmentIntervention => "treatment_intervention",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "patient" => Ok(Self::Patient),
            "session" => Ok(Self::Session),
            "file" => Ok(Self::File),
            "diagnosis" => Ok(Self::Diagnosis),
            "medication" => Ok(Self::Medication),
            "outcome_score" => Ok(Self::OutcomeScore),
            "treatment_plan" => Ok(Self::TreatmentPlan),
            "treatment_goal" => Ok(Self::TreatmentGoal),
            "treatment_intervention" => Ok(Self::TreatmentIntervention),
            other => Err(AppError::Validation(format!(
                "Unknown evidence record kind: {other}"
            ))),
        }
    }
}

/// Section names. A section is either a real column of the source record or
/// [`section::CANONICAL`], a deterministic rendering of the record's typed
/// columns (see [`SourceDoc`] construction below).
pub mod section {
    pub const CANONICAL: &str = "canonical";
    pub const NOTES: &str = "notes";
    pub const CLINICAL_SUMMARY: &str = "clinical_summary";
    pub const EXTRACTED_TEXT: &str = "extracted_text";
    pub const DESCRIPTION: &str = "description";
}

/// Evidence tier.
///
/// * `Structured` — canonical clinical truth rendered from typed columns.
///   Always offered to the model first and never summarised.
/// * `Hot` — recent verbatim text, kept available for quotation.
/// * `Cold` — older verbatim text. Searchable, and promoted into the prompt
///   verbatim when retrieval selects it, otherwise represented by a pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Structured,
    Hot,
    Cold,
}

impl Tier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Structured => "structured",
            Self::Hot => "hot",
            Self::Cold => "cold",
        }
    }

    pub fn parse(value: &str) -> Result<Self, AppError> {
        match value {
            "structured" => Ok(Self::Structured),
            "hot" => Ok(Self::Hot),
            "cold" => Ok(Self::Cold),
            other => Err(AppError::Validation(format!(
                "Unknown evidence tier: {other}"
            ))),
        }
    }
}

/// One indexable section of one source record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDoc {
    pub patient_id: String,
    pub kind: RecordKind,
    pub record_id: String,
    pub section: &'static str,
    /// Human-readable citation label, e.g. `Sitzung 2026-03-04 (Notizen)`.
    pub label: String,
    /// Clinical timestamp the content belongs to (session date, diagnosis date…).
    pub occurred_at: String,
    /// Row mutation timestamp, part of the revision.
    pub updated_at: String,
    pub text: String,
}

impl SourceDoc {
    /// Content-addressed revision of this section.
    ///
    /// Both `updated_at` and the text itself are covered, so an edit is
    /// detected even if a row's timestamp was not refreshed.
    pub fn revision(&self) -> String {
        revision(
            self.kind,
            &self.record_id,
            self.section,
            &self.updated_at,
            &self.text,
        )
    }

    /// Tier for this section given the hot-window cutoff date.
    ///
    /// Canonical sections are structured truth. Verbatim sections are hot when
    /// their clinical date is at or after `hot_cutoff`.
    pub fn tier(&self, hot_cutoff: &str) -> Tier {
        if self.section == section::CANONICAL {
            return Tier::Structured;
        }
        if self.occurred_at.as_str() >= hot_cutoff {
            Tier::Hot
        } else {
            Tier::Cold
        }
    }
}

/// Content-addressed revision string for one source section.
pub fn revision(
    kind: RecordKind,
    record_id: &str,
    section: &str,
    updated_at: &str,
    text: &str,
) -> String {
    let payload = format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        kind.as_str(),
        record_id,
        section,
        updated_at,
        text.chars().count(),
        text
    );
    let digest = ring::digest::digest(&ring::digest::SHA256, payload.as_bytes());
    format!("r1:{}", hex::encode(&digest.as_ref()[..12]))
}

/// Short content digest, used to tie a manifest entry to exact unit text
/// without copying clinical text into the manifest.
pub fn text_digest(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    hex::encode(&digest.as_ref()[..8])
}

/// Aggregate revision over every indexable section of a patient's record.
///
/// Used as the `patient_revision` of an inference session so a KV cache built
/// from an older record is never reused, and as the manifest's record stamp.
pub fn patient_revision(conn: &Connection, patient_id: &str) -> Result<String, AppError> {
    let docs = collect_sources(conn, patient_id)?;
    let mut revisions: Vec<String> = docs
        .iter()
        .map(|doc| {
            format!(
                "{}:{}:{}:{}",
                doc.kind.as_str(),
                doc.record_id,
                doc.section,
                doc.revision()
            )
        })
        .collect();
    revisions.sort();
    let digest = ring::digest::digest(&ring::digest::SHA256, revisions.join("\n").as_bytes());
    Ok(format!("p1:{}", hex::encode(&digest.as_ref()[..12])))
}

/// The result of re-checking a stored evidence span against its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpanResolution {
    /// The source section still exists for this patient.
    pub source_present: bool,
    /// The source revision matches the revision recorded for the span.
    pub revision_current: bool,
    /// The character range still yields byte-identical text.
    pub text_matches: bool,
    /// Current revision of the source section, if present.
    pub current_revision: Option<String>,
    /// Text currently found at the recorded character range, if present.
    pub current_text: Option<String>,
}

impl SpanResolution {
    /// A span is traceable when the source is present, unchanged, and still
    /// produces the exact text that was indexed.
    pub fn is_traceable(&self) -> bool {
        self.source_present && self.revision_current && self.text_matches
    }

    fn missing() -> Self {
        Self {
            source_present: false,
            revision_current: false,
            text_matches: false,
            current_revision: None,
            current_text: None,
        }
    }
}

/// Re-resolve a character range against the live source record.
#[allow(clippy::too_many_arguments)]
pub fn resolve_span(
    conn: &Connection,
    patient_id: &str,
    kind: RecordKind,
    record_id: &str,
    section: &str,
    revision_recorded: &str,
    char_start: usize,
    char_end: usize,
    text_recorded: &str,
) -> Result<SpanResolution, AppError> {
    resolve_span_digest(
        conn,
        patient_id,
        kind,
        record_id,
        section,
        revision_recorded,
        char_start,
        char_end,
        &text_digest(text_recorded),
    )
}

/// Like [`resolve_span`], but compares content by digest so callers that never
/// stored the clinical text (such as an evidence manifest) can still verify it.
#[allow(clippy::too_many_arguments)]
pub fn resolve_span_digest(
    conn: &Connection,
    patient_id: &str,
    kind: RecordKind,
    record_id: &str,
    section: &str,
    revision_recorded: &str,
    char_start: usize,
    char_end: usize,
    digest_recorded: &str,
) -> Result<SpanResolution, AppError> {
    let Some(doc) = load_source(conn, patient_id, kind, record_id, section)? else {
        return Ok(SpanResolution::missing());
    };
    let current_revision = doc.revision();
    let current_text = char_slice(&doc.text, char_start, char_end);
    Ok(SpanResolution {
        source_present: true,
        revision_current: current_revision == revision_recorded,
        text_matches: text_digest(&current_text) == digest_recorded,
        current_revision: Some(current_revision),
        current_text: Some(current_text),
    })
}

// ── Source loading ───────────────────────────────────────────────────────────
//
// One SELECT per table, always scoped by patient. The single-record loader
// reuses the same statement with an extra id predicate so the list and lookup
// paths can never disagree about scoping or column order.

const PATIENT_SELECT: &str = "SELECT id, first_name, last_name, date_of_birth, gender, insurance, \
     gp_name, notes, updated_at FROM patients WHERE id = ?1";

const SESSION_SELECT: &str =
    "SELECT id, session_date, session_type, notes, clinical_summary, updated_at \
     FROM sessions WHERE patient_id = ?1";

const FILE_SELECT: &str = "SELECT id, filename, document_type, extracted_text, created_at \
     FROM files WHERE patient_id = ?1";

const DIAGNOSIS_SELECT: &str =
    "SELECT id, icd10_code, description, status, diagnosed_date, resolved_date, notes, updated_at \
     FROM diagnoses WHERE patient_id = ?1";

const MEDICATION_SELECT: &str =
    "SELECT id, substance, dosage, frequency, start_date, end_date, notes, updated_at \
     FROM medications WHERE patient_id = ?1";

const OUTCOME_SCORE_SELECT: &str =
    "SELECT o.id, o.scale_type, o.score, o.interpretation, o.administered_at, o.notes, o.updated_at \
     FROM outcome_scores o JOIN sessions s ON o.session_id = s.id WHERE s.patient_id = ?1";

const TREATMENT_PLAN_SELECT: &str =
    "SELECT id, title, description, status, start_date, end_date, updated_at \
     FROM treatment_plans WHERE patient_id = ?1";

const TREATMENT_GOAL_SELECT: &str =
    "SELECT g.id, g.description, g.status, g.target_date, g.updated_at, p.start_date \
     FROM treatment_goals g JOIN treatment_plans p ON g.treatment_plan_id = p.id \
     WHERE p.patient_id = ?1";

const TREATMENT_INTERVENTION_SELECT: &str =
    "SELECT i.id, i.type, i.description, i.frequency, i.updated_at, p.start_date \
     FROM treatment_interventions i JOIN treatment_plans p ON i.treatment_plan_id = p.id \
     WHERE p.patient_id = ?1";

fn by_id(base: &str, id_column: &str) -> String {
    format!("{base} AND {id_column} = ?2")
}

/// Every indexable section of every record belonging to `patient_id`.
pub fn collect_sources(conn: &Connection, patient_id: &str) -> Result<Vec<SourceDoc>, AppError> {
    let mut docs = Vec::new();
    for kind in [
        RecordKind::Patient,
        RecordKind::Session,
        RecordKind::File,
        RecordKind::Diagnosis,
        RecordKind::Medication,
        RecordKind::OutcomeScore,
        RecordKind::TreatmentPlan,
        RecordKind::TreatmentGoal,
        RecordKind::TreatmentIntervention,
    ] {
        docs.extend(collect_kind(conn, patient_id, kind, None)?);
    }
    docs.retain(|doc| !doc.text.trim().is_empty());
    Ok(docs)
}

/// One section of one record, or `None` when it does not exist for this patient.
pub fn load_source(
    conn: &Connection,
    patient_id: &str,
    kind: RecordKind,
    record_id: &str,
    section: &str,
) -> Result<Option<SourceDoc>, AppError> {
    let docs = collect_kind(conn, patient_id, kind, Some(record_id))?;
    Ok(docs
        .into_iter()
        .find(|doc| doc.record_id == record_id && doc.section == section))
}

fn collect_kind(
    conn: &Connection,
    patient_id: &str,
    kind: RecordKind,
    record_id: Option<&str>,
) -> Result<Vec<SourceDoc>, AppError> {
    let mut docs = Vec::new();
    match kind {
        RecordKind::Patient => {
            // The patient row *is* the record; a record-id filter is redundant.
            if let Some(id) = record_id {
                if id != patient_id {
                    return Ok(docs);
                }
            }
            let mut stmt = conn.prepare(PATIENT_SELECT)?;
            let rows = stmt.query_map([patient_id], |row| {
                Ok(PatientRow {
                    id: row.get(0)?,
                    first_name: row.get(1)?,
                    last_name: row.get(2)?,
                    date_of_birth: row.get(3)?,
                    gender: row.get(4)?,
                    insurance: row.get(5)?,
                    gp_name: row.get(6)?,
                    notes: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?;
            for row in rows {
                docs.extend(row?.into_docs(patient_id));
            }
        }
        RecordKind::Session => {
            let sql = match record_id {
                Some(_) => by_id(SESSION_SELECT, "id"),
                None => SESSION_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(SessionRow {
                    id: row.get(0)?,
                    session_date: row.get(1)?,
                    session_type: row.get(2)?,
                    notes: row.get(3)?,
                    clinical_summary: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            };
            let rows = query_scoped(&mut stmt, patient_id, record_id, map)?;
            for row in rows {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::File => {
            let sql = match record_id {
                Some(_) => by_id(FILE_SELECT, "id"),
                None => FILE_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(FileRow {
                    id: row.get(0)?,
                    filename: row.get(1)?,
                    document_type: row.get(2)?,
                    extracted_text: row.get(3)?,
                    created_at: row.get(4)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::Diagnosis => {
            let sql = match record_id {
                Some(_) => by_id(DIAGNOSIS_SELECT, "id"),
                None => DIAGNOSIS_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(DiagnosisRow {
                    id: row.get(0)?,
                    icd10_code: row.get(1)?,
                    description: row.get(2)?,
                    status: row.get(3)?,
                    diagnosed_date: row.get(4)?,
                    resolved_date: row.get(5)?,
                    notes: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::Medication => {
            let sql = match record_id {
                Some(_) => by_id(MEDICATION_SELECT, "id"),
                None => MEDICATION_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(MedicationRow {
                    id: row.get(0)?,
                    substance: row.get(1)?,
                    dosage: row.get(2)?,
                    frequency: row.get(3)?,
                    start_date: row.get(4)?,
                    end_date: row.get(5)?,
                    notes: row.get(6)?,
                    updated_at: row.get(7)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::OutcomeScore => {
            let sql = match record_id {
                Some(_) => by_id(OUTCOME_SCORE_SELECT, "o.id"),
                None => OUTCOME_SCORE_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(OutcomeScoreRow {
                    id: row.get(0)?,
                    scale_type: row.get(1)?,
                    score: row.get(2)?,
                    interpretation: row.get(3)?,
                    administered_at: row.get(4)?,
                    notes: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::TreatmentPlan => {
            let sql = match record_id {
                Some(_) => by_id(TREATMENT_PLAN_SELECT, "id"),
                None => TREATMENT_PLAN_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(TreatmentPlanRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    status: row.get(3)?,
                    start_date: row.get(4)?,
                    end_date: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::TreatmentGoal => {
            let sql = match record_id {
                Some(_) => by_id(TREATMENT_GOAL_SELECT, "g.id"),
                None => TREATMENT_GOAL_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(TreatmentGoalRow {
                    id: row.get(0)?,
                    description: row.get(1)?,
                    status: row.get(2)?,
                    target_date: row.get(3)?,
                    updated_at: row.get(4)?,
                    plan_start_date: row.get(5)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
        RecordKind::TreatmentIntervention => {
            let sql = match record_id {
                Some(_) => by_id(TREATMENT_INTERVENTION_SELECT, "i.id"),
                None => TREATMENT_INTERVENTION_SELECT.to_string(),
            };
            let mut stmt = conn.prepare(&sql)?;
            let map = |row: &rusqlite::Row<'_>| {
                Ok(TreatmentInterventionRow {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    description: row.get(2)?,
                    frequency: row.get(3)?,
                    updated_at: row.get(4)?,
                    plan_start_date: row.get(5)?,
                })
            };
            for row in query_scoped(&mut stmt, patient_id, record_id, map)? {
                docs.extend(row.into_docs(patient_id));
            }
        }
    }
    Ok(docs)
}

fn query_scoped<T, F>(
    stmt: &mut rusqlite::Statement<'_>,
    patient_id: &str,
    record_id: Option<&str>,
    map: F,
) -> Result<Vec<T>, AppError>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let rows = match record_id {
        Some(id) => stmt
            .query_map(rusqlite::params![patient_id, id], map)?
            .collect::<Result<Vec<_>, _>>()?,
        None => stmt
            .query_map(rusqlite::params![patient_id], map)?
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(rows)
}

/// The record a section belongs to: its identity plus the timestamp that feeds
/// the section revision.
struct RecordRef<'a> {
    patient_id: &'a str,
    kind: RecordKind,
    record_id: &'a str,
    updated_at: &'a str,
}

fn doc(
    record: &RecordRef<'_>,
    section: &'static str,
    label: String,
    occurred_at: String,
    text: String,
) -> SourceDoc {
    SourceDoc {
        patient_id: record.patient_id.to_string(),
        kind: record.kind,
        record_id: record.record_id.to_string(),
        section,
        label,
        occurred_at,
        updated_at: record.updated_at.to_string(),
        text,
    }
}

fn optional(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

struct PatientRow {
    id: String,
    first_name: String,
    last_name: String,
    date_of_birth: String,
    gender: Option<String>,
    insurance: Option<String>,
    gp_name: Option<String>,
    notes: Option<String>,
    updated_at: String,
}

impl PatientRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::Patient,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let mut canonical = format!(
            "Patient: {} {} — geboren {}",
            self.first_name, self.last_name, self.date_of_birth
        );
        if let Some(gender) = optional(self.gender) {
            canonical.push_str(&format!("; Geschlecht: {gender}"));
        }
        if let Some(insurance) = optional(self.insurance) {
            canonical.push_str(&format!("; Versicherung: {insurance}"));
        }
        if let Some(gp) = optional(self.gp_name) {
            canonical.push_str(&format!("; Hausarzt: {gp}"));
        }

        let mut docs = vec![doc(
            &record,
            section::CANONICAL,
            "Patientenstammdaten".to_string(),
            self.date_of_birth.clone(),
            canonical,
        )];
        if let Some(notes) = optional(self.notes) {
            docs.push(doc(
                &record,
                section::NOTES,
                "Patientennotizen".to_string(),
                self.updated_at.clone(),
                notes,
            ));
        }
        docs
    }
}

struct SessionRow {
    id: String,
    session_date: String,
    session_type: String,
    notes: Option<String>,
    clinical_summary: Option<String>,
    updated_at: String,
}

impl SessionRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::Session,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let mut docs = Vec::new();
        if let Some(notes) = optional(self.notes) {
            docs.push(doc(
                &record,
                section::NOTES,
                format!("Sitzung {} ({})", self.session_date, self.session_type),
                self.session_date.clone(),
                notes,
            ));
        }
        if let Some(summary) = optional(self.clinical_summary) {
            docs.push(doc(
                &record,
                section::CLINICAL_SUMMARY,
                format!("Sitzung {} (klinische Zusammenfassung)", self.session_date),
                self.session_date.clone(),
                summary,
            ));
        }
        docs
    }
}

struct FileRow {
    id: String,
    filename: String,
    document_type: Option<String>,
    extracted_text: Option<String>,
    created_at: String,
}

impl FileRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        // Files have no `updated_at`; the content hash in the revision covers
        // re-extraction of the same file.
        let record = RecordRef {
            patient_id,
            kind: RecordKind::File,
            record_id: &self.id,
            updated_at: &self.created_at,
        };
        let Some(text) = optional(self.extracted_text) else {
            return Vec::new();
        };
        let label = match optional(self.document_type) {
            Some(kind) => format!("Dokument {} ({})", self.filename, kind),
            None => format!("Dokument {}", self.filename),
        };
        vec![doc(
            &record,
            section::EXTRACTED_TEXT,
            label,
            self.created_at.clone(),
            text,
        )]
    }
}

struct DiagnosisRow {
    id: String,
    icd10_code: String,
    description: String,
    status: String,
    diagnosed_date: String,
    resolved_date: Option<String>,
    notes: Option<String>,
    updated_at: String,
}

impl DiagnosisRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::Diagnosis,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let mut canonical = format!(
            "Diagnose {} — {} (Status: {}, diagnostiziert: {}",
            self.icd10_code, self.description, self.status, self.diagnosed_date
        );
        match optional(self.resolved_date) {
            Some(resolved) => canonical.push_str(&format!(", aufgelöst: {resolved})")),
            None => canonical.push(')'),
        }
        let label = format!("Diagnose {}", self.icd10_code);
        let mut docs = vec![doc(
            &record,
            section::CANONICAL,
            label.clone(),
            self.diagnosed_date.clone(),
            canonical,
        )];
        if let Some(notes) = optional(self.notes) {
            docs.push(doc(
                &record,
                section::NOTES,
                format!("{label} (Notizen)"),
                self.diagnosed_date.clone(),
                notes,
            ));
        }
        docs
    }
}

struct MedicationRow {
    id: String,
    substance: String,
    dosage: String,
    frequency: String,
    start_date: String,
    end_date: Option<String>,
    notes: Option<String>,
    updated_at: String,
}

impl MedicationRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::Medication,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let period = match optional(self.end_date) {
            Some(end) => format!("{} bis {}", self.start_date, end),
            None => format!("seit {} (laufend)", self.start_date),
        };
        let canonical = format!(
            "Medikation {} {} {} — {}",
            self.substance, self.dosage, self.frequency, period
        );
        let label = format!("Medikation {}", self.substance);
        let mut docs = vec![doc(
            &record,
            section::CANONICAL,
            label.clone(),
            self.start_date.clone(),
            canonical,
        )];
        if let Some(notes) = optional(self.notes) {
            docs.push(doc(
                &record,
                section::NOTES,
                format!("{label} (Notizen)"),
                self.start_date.clone(),
                notes,
            ));
        }
        docs
    }
}

struct OutcomeScoreRow {
    id: String,
    scale_type: String,
    score: i64,
    interpretation: Option<String>,
    administered_at: String,
    notes: Option<String>,
    updated_at: String,
}

impl OutcomeScoreRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::OutcomeScore,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let interpretation = optional(self.interpretation)
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        let canonical = format!(
            "Score {} = {} am {}{}",
            self.scale_type, self.score, self.administered_at, interpretation
        );
        let label = format!("{} {}", self.scale_type, self.administered_at);
        let mut docs = vec![doc(
            &record,
            section::CANONICAL,
            label.clone(),
            self.administered_at.clone(),
            canonical,
        )];
        if let Some(notes) = optional(self.notes) {
            docs.push(doc(
                &record,
                section::NOTES,
                format!("{label} (Notizen)"),
                self.administered_at.clone(),
                notes,
            ));
        }
        docs
    }
}

struct TreatmentPlanRow {
    id: String,
    title: String,
    description: Option<String>,
    status: String,
    start_date: String,
    end_date: Option<String>,
    updated_at: String,
}

impl TreatmentPlanRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::TreatmentPlan,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let period = match optional(self.end_date) {
            Some(end) => format!("{} bis {}", self.start_date, end),
            None => format!("seit {}", self.start_date),
        };
        let canonical = format!(
            "Behandlungsplan {} (Status: {}, {})",
            self.title, self.status, period
        );
        let label = format!("Behandlungsplan {}", self.title);
        let mut docs = vec![doc(
            &record,
            section::CANONICAL,
            label.clone(),
            self.start_date.clone(),
            canonical,
        )];
        if let Some(description) = optional(self.description) {
            docs.push(doc(
                &record,
                section::DESCRIPTION,
                format!("{label} (Beschreibung)"),
                self.start_date.clone(),
                description,
            ));
        }
        docs
    }
}

struct TreatmentGoalRow {
    id: String,
    description: String,
    status: String,
    target_date: Option<String>,
    updated_at: String,
    plan_start_date: String,
}

impl TreatmentGoalRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::TreatmentGoal,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let target = optional(self.target_date)
            .map(|date| format!(", Zieldatum: {date}"))
            .unwrap_or_default();
        let canonical = format!(
            "Therapieziel {} (Status: {}{})",
            self.description, self.status, target
        );
        vec![doc(
            &record,
            section::CANONICAL,
            "Therapieziel".to_string(),
            self.plan_start_date.clone(),
            canonical,
        )]
    }
}

struct TreatmentInterventionRow {
    id: String,
    kind: String,
    description: String,
    frequency: Option<String>,
    updated_at: String,
    plan_start_date: String,
}

impl TreatmentInterventionRow {
    fn into_docs(self, patient_id: &str) -> Vec<SourceDoc> {
        let record = RecordRef {
            patient_id,
            kind: RecordKind::TreatmentIntervention,
            record_id: &self.id,
            updated_at: &self.updated_at,
        };
        let frequency = optional(self.frequency)
            .map(|value| format!(", Frequenz: {value}"))
            .unwrap_or_default();
        let canonical = format!(
            "Intervention {} — {} (Typ: {}{})",
            self.description, self.plan_start_date, self.kind, frequency
        );
        vec![doc(
            &record,
            section::CANONICAL,
            "Intervention".to_string(),
            self.plan_start_date.clone(),
            canonical,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::evidence::test_support::{seed_patient, TestVault};

    #[test]
    fn collect_sources_is_patient_scoped() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");

        let docs = collect_sources(&conn, &a.patient_id).unwrap();
        assert!(docs.iter().all(|doc| doc.patient_id == a.patient_id));
        assert!(docs
            .iter()
            .any(|doc| doc.record_id == a.session_id && doc.section == section::NOTES));
        assert!(!docs.iter().any(|doc| doc.record_id == b.session_id));
    }

    #[test]
    fn load_source_refuses_cross_patient_record_ids() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let b = seed_patient(&conn, "756.2222.2222.22", "Bruno", "Berger");

        let own = load_source(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &a.session_id,
            section::NOTES,
        )
        .unwrap();
        assert!(own.is_some());

        let foreign = load_source(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &b.session_id,
            section::NOTES,
        )
        .unwrap();
        assert!(foreign.is_none(), "another patient's session must not load");
    }

    #[test]
    fn revision_changes_with_content_and_timestamp() {
        let base = revision(RecordKind::Session, "s1", section::NOTES, "t1", "text");
        assert_eq!(
            base,
            revision(RecordKind::Session, "s1", section::NOTES, "t1", "text")
        );
        assert_ne!(
            base,
            revision(RecordKind::Session, "s1", section::NOTES, "t2", "text")
        );
        assert_ne!(
            base,
            revision(RecordKind::Session, "s1", section::NOTES, "t1", "text ")
        );
        assert_ne!(
            base,
            revision(RecordKind::Session, "s2", section::NOTES, "t1", "text")
        );
        assert!(base.starts_with("r1:"));
    }

    #[test]
    fn patient_revision_tracks_record_edits() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let before = patient_revision(&conn, &a.patient_id).unwrap();
        conn.execute(
            "UPDATE sessions SET notes = 'Notizen überarbeitet' WHERE id = ?1",
            [&a.session_id],
        )
        .unwrap();
        let after = patient_revision(&conn, &a.patient_id).unwrap();
        assert_ne!(before, after);
    }

    #[test]
    fn resolve_span_detects_edits_and_deletions() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");

        let source = load_source(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &a.session_id,
            section::NOTES,
        )
        .unwrap()
        .unwrap();
        let revision_recorded = source.revision();
        let recorded_text = char_slice(&source.text, 0, 10);

        let resolution = resolve_span(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &a.session_id,
            section::NOTES,
            &revision_recorded,
            0,
            10,
            &recorded_text,
        )
        .unwrap();
        assert!(resolution.is_traceable());

        conn.execute(
            "UPDATE sessions SET notes = 'Vollständig neuer Notizentext' WHERE id = ?1",
            [&a.session_id],
        )
        .unwrap();
        let stale = resolve_span(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &a.session_id,
            section::NOTES,
            &revision_recorded,
            0,
            10,
            &recorded_text,
        )
        .unwrap();
        assert!(stale.source_present);
        assert!(!stale.revision_current);
        assert!(!stale.is_traceable());

        conn.execute("DELETE FROM sessions WHERE id = ?1", [&a.session_id])
            .unwrap();
        let gone = resolve_span(
            &conn,
            &a.patient_id,
            RecordKind::Session,
            &a.session_id,
            section::NOTES,
            &revision_recorded,
            0,
            10,
            &recorded_text,
        )
        .unwrap();
        assert!(!gone.source_present);
    }

    #[test]
    fn canonical_sections_are_structured_and_old_text_is_cold() {
        let vault = TestVault::new();
        let conn = vault.conn();
        let a = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
        let docs = collect_sources(&conn, &a.patient_id).unwrap();

        let medication = docs
            .iter()
            .find(|doc| doc.kind == RecordKind::Medication && doc.section == section::CANONICAL)
            .expect("medication canonical section");
        assert_eq!(medication.tier("2000-01-01"), Tier::Structured);
        assert!(medication.text.contains("Sertralin"));

        let notes = docs
            .iter()
            .find(|doc| doc.kind == RecordKind::Session && doc.section == section::NOTES)
            .expect("session notes");
        assert_eq!(notes.tier("2000-01-01"), Tier::Hot);
        assert_eq!(notes.tier("2999-01-01"), Tier::Cold);
    }
}
