use crate::audit::{query_log, AuditEntry};
use crate::error::AppError;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct QueryAuditLogRequest {
    pub entity_type: Option<String>,
    pub entity_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Query the audit log
/// This command is for administrative/settings purposes to view audit history
#[tauri::command]
pub async fn query_audit_log(
    state: State<'_, AppState>,
    request: QueryAuditLogRequest,
) -> Result<Vec<AuditEntry>, AppError> {
    let pool = state.get_db()?;
    let conn = pool.conn()?;

    query_log(
        &conn,
        request.entity_type.as_deref(),
        request.entity_id.as_deref(),
        request.from.as_deref(),
        request.to.as_deref(),
        request.limit.unwrap_or(100),
        request.offset.unwrap_or(0),
    )
}
