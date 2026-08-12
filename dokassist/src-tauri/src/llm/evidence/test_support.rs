//! Shared fixtures for evidence tests: an encrypted throwaway vault and a
//! seeded patient with a realistic, dated record.

use chrono::{Duration, Utc};
use rusqlite::Connection;

use crate::database::{self, DbConnection, DbPool};

/// An encrypted SQLCipher database in a temporary directory.
pub struct TestVault {
    _dir: tempfile::TempDir,
    pool: DbPool,
}

impl TestVault {
    pub fn new() -> Self {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let pool = database::init_db(&dir.path().join("test.db"), &[7u8; 32]).expect("init db");
        Self { _dir: dir, pool }
    }

    pub fn conn(&self) -> DbConnection<'_> {
        self.pool.conn().expect("connection")
    }
}

/// Ids of a seeded patient's records.
pub struct SeededPatient {
    pub patient_id: String,
    /// Most recent session (10 days ago).
    pub session_id: String,
    /// Middle session (40 days ago), where the dose was increased.
    pub middle_session_id: String,
    /// First session (70 days ago).
    pub first_session_id: String,
    pub file_id: String,
    pub diagnosis_id: String,
    pub medication_id: String,
}

fn days_ago(days: i64) -> String {
    (Utc::now() - Duration::days(days))
        .format("%Y-%m-%d")
        .to_string()
}

/// Insert a patient with sessions, a diagnosis, a medication, outcome scores, a
/// treatment plan and a document, all dated relative to today so the hot/cold
/// split behaves as it would in production.
pub fn seed_patient(
    conn: &Connection,
    ahv_number: &str,
    first_name: &str,
    last_name: &str,
) -> SeededPatient {
    let suffix = ahv_number.replace('.', "");
    let patient_id = format!("patient-{suffix}");
    let session_id = format!("session-recent-{suffix}");
    let middle_session_id = format!("session-middle-{suffix}");
    let first_session_id = format!("session-first-{suffix}");
    let file_id = format!("file-{suffix}");
    let diagnosis_id = format!("diagnosis-{suffix}");
    let medication_id = format!("medication-{suffix}");
    let plan_id = format!("plan-{suffix}");

    conn.execute(
        "INSERT INTO patients (id, ahv_number, first_name, last_name, date_of_birth, gender, \
             insurance, gp_name, notes) \
         VALUES (?1, ?2, ?3, ?4, '1988-06-12', 'female', 'Helsana', 'Dr. Meier', \
                 'Bevorzugt Termine am Morgen.')",
        rusqlite::params![patient_id, ahv_number, first_name, last_name],
    )
    .expect("insert patient");

    let sessions = [
        (
            &first_session_id,
            days_ago(70),
            "Erstgespräch",
            "Erstgespräch nach Zuweisung durch den Hausarzt. Antriebslosigkeit, Grübeln und \
             Schlafstörungen seit drei Monaten. Keine Suizidalität.",
            None,
        ),
        (
            &middle_session_id,
            days_ago(40),
            "Verlaufsgespräch",
            "Schlafstörungen weiterhin belastend. Sertralin von 50 mg auf 100 mg erhöht.",
            None,
        ),
        (
            &session_id,
            days_ago(10),
            "Verlaufsgespräch",
            "Schlaf deutlich gebessert unter Sertralin 100 mg. Stimmung stabil, keine \
             Suizidalität. Patientin möchte die Therapiefrequenz reduzieren.",
            Some("Verlauf: Schlafstörungen rückläufig, Sertralin unverändert fortgeführt."),
        ),
    ];

    for (id, date, session_type, notes, summary) in sessions {
        conn.execute(
            "INSERT INTO sessions (id, patient_id, session_date, session_type, duration_minutes, \
                 notes, clinical_summary) \
             VALUES (?1, ?2, ?3, ?4, 50, ?5, ?6)",
            rusqlite::params![id, patient_id, date, session_type, notes, summary],
        )
        .expect("insert session");
    }

    conn.execute(
        "INSERT INTO diagnoses (id, patient_id, icd10_code, description, status, diagnosed_date, \
             notes) \
         VALUES (?1, ?2, 'F32.1', 'Mittelgradige depressive Episode', 'active', ?3, \
                 'Diagnose im Erstgespräch gestellt.')",
        rusqlite::params![diagnosis_id, patient_id, days_ago(70)],
    )
    .expect("insert diagnosis");

    conn.execute(
        "INSERT INTO medications (id, patient_id, substance, dosage, frequency, start_date, notes) \
         VALUES (?1, ?2, 'Sertralin', '100 mg', '1-0-0', ?3, 'Aufdosierung gut vertragen.')",
        rusqlite::params![medication_id, patient_id, days_ago(40)],
    )
    .expect("insert medication");

    for (index, (session, score)) in [
        (&first_session_id, 18),
        (&middle_session_id, 12),
        (&session_id, 8),
    ]
    .into_iter()
    .enumerate()
    {
        conn.execute(
            "INSERT INTO outcome_scores (id, session_id, scale_type, score, interpretation, \
                 administered_at) \
             VALUES (?1, ?2, 'PHQ-9', ?3, 'Moderat', ?4)",
            rusqlite::params![
                format!("score-{index}-{suffix}"),
                session,
                score,
                days_ago(70 - (index as i64) * 30)
            ],
        )
        .expect("insert outcome score");
    }

    conn.execute(
        "INSERT INTO treatment_plans (id, patient_id, title, description, start_date, status) \
         VALUES (?1, ?2, 'Kognitive Verhaltenstherapie', \
                 'Wöchentliche Sitzungen mit Fokus auf Schlafhygiene.', ?3, 'active')",
        rusqlite::params![plan_id, patient_id, days_ago(65)],
    )
    .expect("insert treatment plan");

    conn.execute(
        "INSERT INTO treatment_goals (id, treatment_plan_id, description, status, sort_order) \
         VALUES (?1, ?2, 'Einschlafzeit unter 30 Minuten', 'in_progress', 0)",
        rusqlite::params![format!("goal-{suffix}"), plan_id],
    )
    .expect("insert treatment goal");

    conn.execute(
        "INSERT INTO files (id, patient_id, filename, vault_path, mime_type, size_bytes, \
             document_type, extracted_text, created_at) \
         VALUES (?1, ?2, 'bericht.pdf', ?3, 'application/pdf', 2048, 'Bericht', \
                 'Austrittsbericht: Behandlung mit Sertralin begonnen, Verlauf stabil.', ?4)",
        rusqlite::params![
            file_id,
            patient_id,
            format!("vault/{suffix}.enc"),
            days_ago(35)
        ],
    )
    .expect("insert file");

    SeededPatient {
        patient_id,
        session_id,
        middle_session_id,
        first_session_id,
        file_id,
        diagnosis_id,
        medication_id,
    }
}
