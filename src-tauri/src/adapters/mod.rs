pub mod claude;
pub mod codex;
pub mod cursor;
pub mod opencode;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::NormalizedSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLocation {
    pub path: PathBuf,
    pub last_modified: DateTime<Utc>,
}

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    async fn detect(&self) -> bool;
    async fn scan(&self) -> Vec<SessionLocation>;
    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String>;
    fn resume_command(&self, session_id: &str, project_path: &str) -> String;
    async fn is_active(&self, session_path: &Path) -> bool;
}

pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn AgentAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            adapters: HashMap::new(),
        };
        reg.register(Box::new(claude::ClaudeAdapter::new()));
        reg.register(Box::new(codex::CodexAdapter::new()));
        reg.register(Box::new(cursor::CursorAdapter::new()));
        reg.register(Box::new(opencode::OpenCodeAdapter::new()));
        reg
    }

    fn register(&mut self, adapter: Box<dyn AgentAdapter>) {
        self.adapters.insert(adapter.id().to_string(), adapter);
    }

    pub async fn detect_available(&self) -> Vec<&dyn AgentAdapter> {
        let mut available = Vec::new();
        for adapter in self.adapters.values() {
            if adapter.detect().await {
                available.push(adapter.as_ref());
            }
        }
        available
    }

    pub fn get(&self, id: &str) -> Option<&dyn AgentAdapter> {
        self.adapters.get(id).map(|a| a.as_ref())
    }

    pub fn all(&self) -> Vec<&dyn AgentAdapter> {
        self.adapters.values().map(|a| a.as_ref()).collect()
    }
}
