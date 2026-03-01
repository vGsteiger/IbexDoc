use tauri::State;
use crate::error::AppError;
use crate::state::AppState;
use crate::models::file_record::FileRecord;

#[tauri::command]
pub async fn upload_file(
    state: State<'_, AppState>,
    patient_id: String,
    filename: String,
    data: Vec<u8>,
    mime_type: String,
) -> Result<FileRecord, AppError> {
    // PKG-3: implement
    Err(AppError::Llm("Not implemented".to_string()))
}

#[tauri::command]
pub async fn download_file(
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<FileRecord, AppError> {
    // PKG-3: implement
    Err(AppError::NotFound(vault_path))
}

#[tauri::command]
pub async fn list_files(
    state: State<'_, AppState>,
    patient_id: String,
) -> Result<Vec<FileRecord>, AppError> {
    // PKG-3: implement
    Ok(vec![])
}

#[tauri::command]
pub async fn delete_file(
    state: State<'_, AppState>,
    vault_path: String,
) -> Result<(), AppError> {
    // PKG-3: implement
    Ok(())
}
