-- Migration 002: Enforce append-only audit log (CRIT-5)
--
-- These triggers enforce application-level append-only behavior by raising a hard
-- error on UPDATE or DELETE while the triggers are installed. They are not a
-- cryptographic integrity mechanism: a party with the database key and filesystem
-- access can replace the database or remove the triggers. See SECURITY.md, Section 8.
--
-- Note: INSERT and SELECT are still permitted.

CREATE TRIGGER IF NOT EXISTS audit_log_no_update
BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only: UPDATE not permitted');
END;

CREATE TRIGGER IF NOT EXISTS audit_log_no_delete
BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only: DELETE not permitted');
END;
