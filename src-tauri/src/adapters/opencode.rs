use async_trait::async_trait;
use std::path::Path;

use super::{AgentAdapter, SessionLocation};
use crate::models::*;

pub struct OpenCodeAdapter;

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentAdapter for OpenCodeAdapter {
    fn id(&self) -> &str {
        "opencode"
    }

    fn name(&self) -> &str {
        "OpenCode"
    }

    async fn detect(&self) -> bool {
        let config_dir = dirs::config_dir().unwrap_or_default();
        config_dir.join("opencode").exists()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        Vec::new()
    }

    async fn parse_session(&self, _path: &Path) -> Result<NormalizedSession, String> {
        Err("OpenCode adapter not yet implemented".to_string())
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        format!("opencode --resume {}", session_id)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}
