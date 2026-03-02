use tauri::{AppHandle, Emitter, State};
use crate::error::AppError;
use crate::llm::{
    self, download, LlmEngine, ReportType, EngineStatus, ModelChoice, SYSTEM_PROMPT_DE,
};
use crate::state::AppState;

/// Return the current engine status (safe to call before a model is loaded).
#[tauri::command]
pub async fn get_engine_status(state: State<'_, AppState>) -> Result<EngineStatus, AppError> {
    let llm = state.llm.lock().unwrap();
    match &*llm {
        Some(engine) => Ok(engine.status()),
        None => Ok(EngineStatus {
            is_loaded: false,
            model_name: None,
            model_path: None,
            available_ram_bytes: LlmEngine::available_ram(),
        }),
    }
}

/// Return the model tier recommended for this machine's RAM.
#[tauri::command]
pub async fn get_recommended_model() -> Result<ModelChoice, AppError> {
    Ok(LlmEngine::recommended_model())
}

/// Return the built-in German system prompt so the frontend can pre-populate its editor.
#[tauri::command]
pub async fn get_default_system_prompt() -> Result<String, AppError> {
    Ok(SYSTEM_PROMPT_DE.to_string())
}

/// Download a GGUF model from HuggingFace to ~/DokAssist/models/.
/// Streams progress via `"model-download-progress"` (f64) and `"model-download-done"` events.
#[tauri::command]
pub async fn download_model(
    app: AppHandle,
    model: ModelChoice,
) -> Result<(), AppError> {
    let dest_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("DokAssist")
        .join("models");
    tokio::fs::create_dir_all(&dest_dir).await?;

    let dest_path = dest_dir.join(&model.filename);
    let url = download::model_url(&model.filename);
    download::download_model_with_progress(&app, &url, &dest_path).await
}

/// Load a GGUF model from ~/DokAssist/models/ into memory (Metal-accelerated).
/// Uses spawn_blocking because model loading is a long blocking C-FFI operation.
#[tauri::command]
pub async fn load_model(
    state: State<'_, AppState>,
    model_filename: String,
) -> Result<(), AppError> {
    let model_path = dirs::home_dir()
        .unwrap_or_default()
        .join("DokAssist")
        .join("models")
        .join(&model_filename);
    let model_name = model_filename.clone();

    let engine = tokio::task::spawn_blocking(move || LlmEngine::load(model_path, model_name))
        .await
        .map_err(|e| AppError::Llm(format!("spawn_blocking error: {e}")))??;

    *state.llm.lock().unwrap() = Some(engine);
    Ok(())
}

/// Extract structured metadata from a document using the loaded LLM.
/// `system_prompt`: optional override; falls back to the built-in German prompt.
#[tauri::command]
pub async fn extract_file_metadata(
    state: State<'_, AppState>,
    document_text: String,
    system_prompt: Option<String>,
) -> Result<llm::FileMetadata, AppError> {
    let llm = state.llm.lock().unwrap();
    let engine = llm
        .as_ref()
        .ok_or_else(|| AppError::Llm("Model not loaded".to_string()))?;
    let prompt = system_prompt.as_deref().unwrap_or(SYSTEM_PROMPT_DE);
    llm::extract_metadata_with_prompt(engine, &document_text, prompt)
}

/// Generate a psychiatric report with streaming output.
/// Emits `"report-chunk"` events for each token and `"report-done"` on completion.
/// `system_prompt`: optional override; falls back to the built-in German prompt.
#[tauri::command]
pub async fn generate_report(
    app: AppHandle,
    state: State<'_, AppState>,
    patient_context: String,
    report_type: String,
    session_notes: String,
    system_prompt: Option<String>,
) -> Result<String, AppError> {
    let rt = match report_type.as_str() {
        "Befundbericht" => ReportType::Befundbericht,
        "Verlaufsbericht" => ReportType::Verlaufsbericht,
        "Ueberweisungsschreiben" => ReportType::Ueberweisungsschreiben,
        other => return Err(AppError::Validation(format!("Unknown report type: {other}"))),
    };

    let llm = state.llm.lock().unwrap();
    let engine = llm
        .as_ref()
        .ok_or_else(|| AppError::Llm("Model not loaded".to_string()))?;
    let prompt = system_prompt.as_deref().unwrap_or(SYSTEM_PROMPT_DE);

    let report =
        llm::generate_report_streaming_with_prompt(&app, engine, rt, &patient_context, &session_notes, prompt)?;
    let _ = app.emit("report-done", ());
    Ok(report)
}
