-- Migration 015: provenance-bearing evidence assembly for patient-history RAG (#403)
--
-- Every row here is a derived artifact of a source record section. Each one
-- carries the patient id and the source revision it was built from, so a
-- changed source invalidates its derived rows and retrieval can never leave the
-- patient it was scoped to.

-- One retrievable slice of one source section, with exact character offsets.
CREATE TABLE IF NOT EXISTS evidence_units (
    id                TEXT PRIMARY KEY NOT NULL,
    patient_id        TEXT NOT NULL,
    record_kind       TEXT NOT NULL,
    record_id         TEXT NOT NULL,
    section           TEXT NOT NULL,
    revision          TEXT NOT NULL,
    tier              TEXT NOT NULL,
    unit_index        INTEGER NOT NULL,
    char_start        INTEGER NOT NULL,
    char_end          INTEGER NOT NULL,
    occurred_at       TEXT NOT NULL,
    source_updated_at TEXT NOT NULL,
    label             TEXT NOT NULL,
    text              TEXT NOT NULL,
    token_estimate    INTEGER NOT NULL,
    protected_json    TEXT NOT NULL DEFAULT '[]',
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_evidence_units_slot
    ON evidence_units(record_kind, record_id, section, unit_index);
CREATE INDEX IF NOT EXISTS idx_evidence_units_patient
    ON evidence_units(patient_id, occurred_at DESC);
CREATE INDEX IF NOT EXISTS idx_evidence_units_source
    ON evidence_units(patient_id, record_kind, record_id, section, unit_index);

-- Indexing bookkeeping: the revision each section was last indexed at. Kept
-- separately from evidence_units so a section that produced no units (empty
-- after an edit) still records a revision and is not re-derived every query.
CREATE TABLE IF NOT EXISTS evidence_sources (
    patient_id  TEXT NOT NULL,
    record_kind TEXT NOT NULL,
    record_id   TEXT NOT NULL,
    section     TEXT NOT NULL,
    revision    TEXT NOT NULL,
    unit_count  INTEGER NOT NULL,
    indexed_at  TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (record_kind, record_id, section),
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evidence_sources_patient
    ON evidence_sources(patient_id);

-- Patient-scoped embeddings over evidence units. patient_id is duplicated here
-- so the vector scan itself is restricted to one patient.
CREATE TABLE IF NOT EXISTS evidence_unit_embeddings (
    unit_id    TEXT PRIMARY KEY NOT NULL,
    patient_id TEXT NOT NULL,
    revision   TEXT NOT NULL,
    vector     BLOB NOT NULL,
    model      TEXT NOT NULL DEFAULT 'nomic-embed-text-v1.5',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (unit_id) REFERENCES evidence_units(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evidence_embeddings_patient
    ON evidence_unit_embeddings(patient_id);

-- Lexical index over evidence units. Rows are written and deleted explicitly
-- from Rust (no triggers), matching how search_index is maintained.
CREATE VIRTUAL TABLE IF NOT EXISTS evidence_fts USING fts5(
    unit_id UNINDEXED,
    patient_id UNINDEXED,
    text,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Manifest of one assembled evidence prompt. Deliberately metadata-only: it
-- stores unit ids, provenance and selection reasons, never record text.
CREATE TABLE IF NOT EXISTS evidence_manifests (
    id               TEXT PRIMARY KEY NOT NULL,
    patient_id       TEXT NOT NULL,
    patient_revision TEXT NOT NULL,
    question_sha256  TEXT NOT NULL,
    token_budget     INTEGER NOT NULL,
    prompt_tokens    INTEGER NOT NULL,
    manifest_json    TEXT NOT NULL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_evidence_manifests_patient
    ON evidence_manifests(patient_id, created_at DESC);
