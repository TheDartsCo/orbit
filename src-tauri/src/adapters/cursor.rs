use async_trait::async_trait;
use std::path::Path;

use super::{AgentAdapter, SessionLocation};
use crate::models::*;

pub struct CursorAdapter;

impl CursorAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentAdapter for CursorAdapter {
    fn id(&self) -> &str {
        "cursor"
    }

    fn name(&self) -> &str {
        "Cursor"
    }

    async fn detect(&self) -> bool {
        let home = dirs::home_dir().unwrap_or_default();
        home.join(".cursor").exists()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        Vec::new()
    }

    async fn parse_session(&self, _path: &Path) -> Result<NormalizedSession, String> {
        Err("Cursor adapter not yet implemented".to_string())
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        format!("cursor --resume {}", session_id)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}
