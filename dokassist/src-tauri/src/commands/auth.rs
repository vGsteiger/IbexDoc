use tauri::State;
use crate::error::AppError;
use crate::state::AppState;

/// Returns "first_run" | "locked" | "recovery_required"
#[tauri::command]
pub async fn check_auth(state: State<'_, AppState>) -> Result<String, AppError> {
    // PKG-1: implement with keychain check
    Ok("first_run".to_string())
}

/// First run: generate keys, store in Keychain. Returns 24 mnemonic words.
#[tauri::command]
pub async fn initialize_app(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    // PKG-1: implement
    Err(AppError::Llm("Not implemented".to_string()))
}

/// Unlock: triggers Touch ID, retrieves keys from Keychain.
#[tauri::command]
pub async fn unlock_app(state: State<'_, AppState>) -> Result<bool, AppError> {
    // PKG-1: implement
    Ok(true)
}

/// Recover keys from 24-word mnemonic.
#[tauri::command]
pub async fn recover_app(
    state: State<'_, AppState>,
    words: Vec<String>,
) -> Result<bool, AppError> {
    // PKG-1: implement
    Err(AppError::InvalidRecoveryPhrase)
}

/// Lock: zero keys from memory.
#[tauri::command]
pub async fn lock_app(state: State<'_, AppState>) -> Result<(), AppError> {
    // PKG-1: implement
    Ok(())
}
