use tauri::State;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    patient_id: String,
) -> Result<String, AppError> {
    // PKG-4: implement
    Err(AppError::Llm("Not implemented".to_string()))
}
