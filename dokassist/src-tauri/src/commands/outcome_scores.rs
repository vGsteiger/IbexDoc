use crate::audit::{self, AuditAction};
use crate::error::AppError;
use crate::models::outcome_score::{self, CreateOutcomeScore, OutcomeScore, UpdateOutcomeScore};
use crate::search;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_outcome_score(
    state: State<'_, AppState>,
    input: CreateOutcomeScore,
) -> Result<OutcomeScore, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    let score = outcome_score::create_outcome_score(&tx, input)?;

    audit::log(
        &tx,
        AuditAction::Create,
        "outcome_score",
        Some(&score.id),
        None,
    )?;

    tx.commit()?;

    // Index for search
    search::index_outcome_score(&conn, &score)?;

    Ok(score)
}

#[tauri::command]
pub async fn get_outcome_score(
    state: State<'_, AppState>,
    id: String,
) -> Result<OutcomeScore, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    outcome_score::get_outcome_score(&conn, &id)
}

#[tauri::command]
pub async fn list_scores_for_session(
    state: State<'_, AppState>,
    session_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<OutcomeScore>, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    outcome_score::list_scores_for_session(
        &conn,
        &session_id,
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub async fn list_scores_by_scale(
    state: State<'_, AppState>,
    scale_type: String,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<OutcomeScore>, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    outcome_score::list_scores_by_scale(
        &conn,
        &scale_type,
        limit.unwrap_or(100),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub async fn update_outcome_score(
    state: State<'_, AppState>,
    id: String,
    input: UpdateOutcomeScore,
) -> Result<OutcomeScore, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    let score = outcome_score::update_outcome_score(&tx, &id, input)?;

    audit::log(
        &tx,
        AuditAction::Update,
        "outcome_score",
        Some(&score.id),
        None,
    )?;

    tx.commit()?;

    // Update search index
    search::update_outcome_score(&conn, &score)?;

    Ok(score)
}

#[tauri::command]
pub async fn delete_outcome_score(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    let tx = conn.unchecked_transaction()?;

    outcome_score::delete_outcome_score(&tx, &id)?;

    audit::log(&tx, AuditAction::Delete, "outcome_score", Some(&id), None)?;

    tx.commit()?;

    // Remove from search index
    search::remove_from_index(&conn, "outcome_score", &id)?;

    Ok(())
}
