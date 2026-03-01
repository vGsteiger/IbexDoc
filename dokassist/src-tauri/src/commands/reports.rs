use tauri::State;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn generate_report(
    state: State<'_, AppState>,
    patient_id: String,
) -> Result<String, AppError> {
    // PKG-5: implement
    Err(AppError::Llm("Not implemented".to_string()))
}
