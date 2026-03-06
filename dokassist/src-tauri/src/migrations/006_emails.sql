-- Email drafts table
CREATE TABLE IF NOT EXISTS emails (
    id TEXT PRIMARY KEY NOT NULL,
    patient_id TEXT NOT NULL,
    recipient_email TEXT NOT NULL,
    subject TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft',
    sent_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (patient_id) REFERENCES patients(id) ON DELETE CASCADE
);

-- Index for performance
CREATE INDEX IF NOT EXISTS idx_emails_patient ON emails(patient_id, created_at DESC);

-- Trigger to update updated_at on emails
CREATE TRIGGER IF NOT EXISTS emails_updated_at
AFTER UPDATE ON emails
BEGIN
    UPDATE emails SET updated_at = datetime('now') WHERE id = NEW.id;
END;
