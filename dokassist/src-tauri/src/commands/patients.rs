use tauri::State;
use crate::error::AppError;
use crate::state::AppState;
use crate::models::patient::{Patient, CreatePatient, UpdatePatient};

#[tauri::command]
pub async fn create_patient(
    state: State<'_, AppState>,
    input: CreatePatient,
) -> Result<Patient, AppError> {
    // PKG-2: implement
    Err(AppError::Llm("Not implemented".to_string()))
}

#[tauri::command]
pub async fn get_patient(
    state: State<'_, AppState>,
    id: String,
) -> Result<Patient, AppError> {
    // PKG-2: implement
    Err(AppError::NotFound(id))
}

#[tauri::command]
pub async fn list_patients(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Patient>, AppError> {
    // PKG-2: implement
    Ok(vec![])
}

#[tauri::command]
pub async fn update_patient(
    state: State<'_, AppState>,
    id: String,
    input: UpdatePatient,
) -> Result<Patient, AppError> {
    // PKG-2: implement
    Err(AppError::NotFound(id))
}

#[tauri::command]
pub async fn delete_patient(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), AppError> {
    // PKG-2: implement
    Ok(())
}
