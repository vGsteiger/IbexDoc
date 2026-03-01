use std::sync::Mutex;

/// Application state shared across all Tauri commands.
pub struct AppState {
    pub auth: Mutex<AuthState>,
    pub data_dir: std::path::PathBuf,
    // db: Option<DbPool>,       // added in PKG-2
    // llm: Option<LlmEngine>,   // added in PKG-4
}

pub enum AuthState {
    FirstRun,
    Locked,
    Unlocked {
        db_key: zeroize::Zeroizing<[u8; 32]>,
        fs_key: zeroize::Zeroizing<[u8; 32]>,
    },
    RecoveryRequired,
}

impl AppState {
    pub fn new(data_dir: std::path::PathBuf) -> Self {
        Self {
            auth: Mutex::new(AuthState::Locked),
            data_dir,
        }
    }
}
