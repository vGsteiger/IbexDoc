use tauri::State;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn search_patients(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<String>, AppError> {
    // PKG-2: implement
    Ok(vec![])
}
