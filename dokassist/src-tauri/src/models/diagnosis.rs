use crate::error::AppError;
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnosis {
    pub id: String,
    pub patient_id: String,
    pub icd10_code: String,
    pub description: String,
    pub status: String,
    pub diagnosed_date: String,
    pub resolved_date: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDiagnosis {
    pub patient_id: String,
    pub icd10_code: String,
    pub description: String,
    pub status: Option<String>,
    pub diagnosed_date: String,
    pub resolved_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDiagnosis {
    pub icd10_code: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub diagnosed_date: Option<String>,
    pub resolved_date: Option<String>,
    pub notes: Option<String>,
}

fn row_to_diagnosis(row: &Row) -> Result<Diagnosis, rusqlite::Error> {
    Ok(Diagnosis {
        id: row.get(0)?,
        patient_id: row.get(1)?,
        icd10_code: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        diagnosed_date: row.get(5)?,
        resolved_date: row.get(6)?,
        notes: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn create_diagnosis(conn: &Connection, input: CreateDiagnosis) -> Result<Diagnosis, AppError> {
    let id = Uuid::now_v7().to_string();
    let status = input.status.unwrap_or_else(|| "active".to_string());

    conn.execute(
        "INSERT INTO diagnoses (id, patient_id, icd10_code, description, status, diagnosed_date, resolved_date, notes)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            id,
            input.patient_id,
            input.icd10_code,
            input.description,
            status,
            input.diagnosed_date,
            input.resolved_date,
            input.notes,
        ],
    )?;

    get_diagnosis(conn, &id)
}

pub fn get_diagnosis(conn: &Connection, id: &str) -> Result<Diagnosis, AppError> {
    let diagnosis = conn.query_row(
        "SELECT id, patient_id, icd10_code, description, status, diagnosed_date, resolved_date, notes,
                created_at, updated_at
         FROM diagnoses WHERE id = ?",
        params![id],
        row_to_diagnosis,
    )?;

    Ok(diagnosis)
}

pub fn update_diagnosis(
    conn: &Connection,
    id: &str,
    input: UpdateDiagnosis,
) -> Result<Diagnosis, AppError> {
    get_diagnosis(conn, id)?;

    let mut updates = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(icd10_code) = input.icd10_code {
        updates.push("icd10_code = ?");
        values.push(Box::new(icd10_code));
    }
    if let Some(description) = input.description {
        updates.push("description = ?");
        values.push(Box::new(description));
    }
    if let Some(status) = input.status {
        updates.push("status = ?");
        values.push(Box::new(status));
    }
    if let Some(diagnosed_date) = input.diagnosed_date {
        updates.push("diagnosed_date = ?");
        values.push(Box::new(diagnosed_date));
    }
    if let Some(resolved_date) = input.resolved_date {
        updates.push("resolved_date = ?");
        values.push(Box::new(resolved_date));
    }
    if let Some(notes) = input.notes {
        updates.push("notes = ?");
        values.push(Box::new(notes));
    }

    if updates.is_empty() {
        return get_diagnosis(conn, id);
    }

    let query = format!("UPDATE diagnoses SET {} WHERE id = ?", updates.join(", "));
    values.push(Box::new(id.to_string()));

    let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
    conn.execute(&query, params.as_slice())?;

    get_diagnosis(conn, id)
}

pub fn delete_diagnosis(conn: &Connection, id: &str) -> Result<(), AppError> {
    let rows_affected = conn.execute("DELETE FROM diagnoses WHERE id = ?", params![id])?;

    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("Diagnosis not found: {}", id)));
    }

    Ok(())
}

pub fn list_diagnoses_for_patient(
    conn: &Connection,
    patient_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<Diagnosis>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT id, patient_id, icd10_code, description, status, diagnosed_date, resolved_date, notes,
                created_at, updated_at
         FROM diagnoses
         WHERE patient_id = ?
         ORDER BY diagnosed_date DESC
         LIMIT ? OFFSET ?",
    )?;

    let diagnoses = stmt
        .query_map(params![patient_id, limit, offset], row_to_diagnosis)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(diagnoses)
}
