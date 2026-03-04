CREATE TABLE IF NOT EXISTS chat_sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    scope       TEXT NOT NULL DEFAULT 'global',
    patient_id  TEXT REFERENCES patients(id) ON DELETE CASCADE,
    title       TEXT NOT NULL DEFAULT 'New Chat',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS chat_messages (
    id               TEXT PRIMARY KEY NOT NULL,
    session_id       TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role             TEXT NOT NULL,
    content          TEXT NOT NULL,
    tool_name        TEXT,
    tool_args_json   TEXT,
    tool_result_for  TEXT,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_patient ON chat_sessions(patient_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_sessions_scope   ON chat_sessions(scope, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at ASC);

CREATE TRIGGER IF NOT EXISTS chat_sessions_updated_at
AFTER UPDATE ON chat_sessions BEGIN
    UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = NEW.id;
END;
