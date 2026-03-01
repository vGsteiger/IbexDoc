use tauri::State;
use crate::error::AppError;
use crate::state::AppState;
use crate::models::patient::{Patient, CreatePatient, UpdatePatient};

#[tauri::command]
pub async fn create_patient(
    state: State<'_, AppState>,
    input: CreatePatient,
) -> Result<Patient, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    crate::models::patient::create_patient(&conn, input)
}

#[tauri::command]
pub async fn get_patient(
    state: State<'_, AppState>,
    id: String,
) -> Result<Patient, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    crate::models::patient::get_patient(&conn, &id)
}

#[tauri::command]
pub async fn list_patients(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Patient>, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);
    crate::models::patient::list_patients(&conn, limit, offset)
}

#[tauri::command]
pub async fn update_patient(
    state: State<'_, AppState>,
    id: String,
    input: UpdatePatient,
) -> Result<Patient, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    crate::models::patient::update_patient(&conn, &id, input)
}

#[tauri::command]
pub async fn delete_patient(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;
    crate::models::patient::delete_patient(&conn, &id)
}
