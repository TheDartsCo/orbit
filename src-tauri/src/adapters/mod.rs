pub mod antigravity;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod cursor;
pub mod jetbrains;
pub mod opencode;
pub mod qoder;
pub mod warp;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::models::NormalizedSession;

#[derive(Debug, Clone)]
pub(crate) struct PlatformPaths {
    pub home: Option<PathBuf>,
    pub data: Option<PathBuf>,
    pub data_local: Option<PathBuf>,
}

impl PlatformPaths {
    pub(crate) fn system() -> Self {
        Self {
            home: dirs::home_dir(),
            data: dirs::data_dir(),
            data_local: dirs::data_local_dir(),
        }
    }

    pub(crate) fn home_join(&self, path: impl AsRef<Path>) -> Option<PathBuf> {
        self.home.as_ref().map(|root| root.join(path))
    }

    pub(crate) fn data_join(&self, path: impl AsRef<Path>) -> Option<PathBuf> {
        self.data.as_ref().map(|root| root.join(path))
    }

    pub(crate) fn data_local_join(&self, path: impl AsRef<Path>) -> Option<PathBuf> {
        self.data_local.as_ref().map(|root| root.join(path))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLocation {
    pub path: PathBuf,
    pub last_modified: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::claude::ClaudeAdapter;
    use super::codex::CodexAdapter;
    use super::copilot::CopilotAdapter;
    use super::cursor::CursorAdapter;
    use super::jetbrains::JetBrainsAdapter;
    use super::opencode::OpenCodeAdapter;
    use super::qoder::QoderAdapter;
    use super::warp::WarpAdapter;
    use super::PlatformPaths;
    use std::fs;
    use std::path::PathBuf;

    fn windows_paths() -> PlatformPaths {
        PlatformPaths {
            home: Some(PathBuf::from(r"C:\Users\orbit")),
            data: Some(PathBuf::from(r"C:\Users\orbit\AppData\Roaming")),
            data_local: Some(PathBuf::from(r"C:\Users\orbit\AppData\Local")),
        }
    }

    #[test]
    fn platform_paths_join_each_windows_root() {
        let paths = windows_paths();

        assert_eq!(
            paths.home_join(".codex"),
            Some(PathBuf::from(r"C:\Users\orbit").join(".codex"))
        );
        assert_eq!(
            paths.data_join("JetBrains"),
            Some(PathBuf::from(r"C:\Users\orbit\AppData\Roaming").join("JetBrains"))
        );
        assert_eq!(
            paths.data_local_join("Warp"),
            Some(PathBuf::from(r"C:\Users\orbit\AppData\Local").join("Warp"))
        );
    }

    #[test]
    fn all_adapters_define_windows_discovery_candidates() {
        let paths = windows_paths();
        let home = PathBuf::from(r"C:\Users\orbit");
        let roaming = PathBuf::from(r"C:\Users\orbit\AppData\Roaming");
        let local = PathBuf::from(r"C:\Users\orbit\AppData\Local");

        assert_eq!(
            ClaudeAdapter::windows_projects_root(&paths),
            Some(home.join(".claude").join("projects"))
        );
        assert_eq!(
            CodexAdapter::windows_data_dir(&paths),
            Some(home.join(".codex"))
        );
        assert_eq!(
            CopilotAdapter::windows_data_dir(&paths),
            Some(home.join(".copilot").join("session-state"))
        );
        assert_eq!(
            CursorAdapter::windows_data_dir(&paths),
            Some(home.join(".cursor"))
        );
        assert_eq!(
            OpenCodeAdapter::windows_candidate_data_dirs(&paths),
            vec![
                home.join(".local").join("share").join("opencode"),
                local.join("opencode"),
                roaming.join("opencode"),
            ]
        );
        assert_eq!(
            JetBrainsAdapter::windows_candidate_data_dirs(&paths),
            vec![roaming.join("JetBrains"), local.join("JetBrains")]
        );
        assert_eq!(
            QoderAdapter::windows_candidate_db_paths(&paths),
            vec![
                roaming
                    .join("Qoder")
                    .join("SharedClientCache/cache/db/local.db"),
                local
                    .join("Qoder")
                    .join("SharedClientCache/cache/db/local.db"),
            ]
        );
        assert_eq!(
            WarpAdapter::windows_candidate_db_paths(&paths),
            vec![
                local
                    .join("warp")
                    .join("Warp")
                    .join("data")
                    .join("warp.sqlite"),
                roaming.join("Warp").join("warp.sqlite"),
                roaming.join("dev.warp.Warp-Stable").join("warp.sqlite"),
                local.join("Warp").join("warp.sqlite"),
                local.join("dev.warp.Warp-Stable").join("warp.sqlite"),
            ]
        );
    }

    #[test]
    fn database_backed_adapters_select_only_valid_windows_locations() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let roaming = temp.path().join("roaming");
        let local = temp.path().join("local");
        let paths = PlatformPaths {
            home: Some(home),
            data: Some(roaming.clone()),
            data_local: Some(local.clone()),
        };

        let jetbrains = local.join("JetBrains");
        fs::create_dir_all(jetbrains.join("Idea2026.1/aia-task-history")).unwrap();

        let qoder = roaming.join("Qoder/SharedClientCache/cache/db/local.db");
        fs::create_dir_all(qoder.parent().unwrap()).unwrap();
        fs::write(&qoder, []).unwrap();

        let warp = local.join("warp/Warp/data/warp.sqlite");
        fs::create_dir_all(warp.parent().unwrap()).unwrap();
        fs::write(&warp, []).unwrap();

        assert_eq!(JetBrainsAdapter::windows_data_dir(&paths), Some(jetbrains));
        assert_eq!(QoderAdapter::windows_db_path(&paths), Some(qoder));
        assert_eq!(WarpAdapter::windows_db_path(&paths), Some(warp));
    }

    #[test]
    fn windows_manual_resume_commands_are_powershell_compatible() {
        assert_eq!(
            ClaudeAdapter::windows_resume_command("session-1", r"C:\Work\Orbit"),
            "Set-Location 'C:\\Work\\Orbit'; claude --resume 'session-1'"
        );
        assert_eq!(
            OpenCodeAdapter::windows_resume_command("session-2", r"C:\Work\Orbit"),
            "Set-Location 'C:\\Work\\Orbit'; opencode --session 'session-2'"
        );
        assert_eq!(
            QoderAdapter::windows_resume_command(),
            "Start-Process Qoder"
        );
        assert_eq!(WarpAdapter::windows_resume_command(), "Start-Process Warp");
    }
}

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    async fn detect(&self) -> bool;
    async fn scan(&self) -> Vec<SessionLocation>;
    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String>;
    fn supports_resume(&self) -> bool {
        true
    }
    fn resume_command(&self, session_id: &str, project_path: &str) -> String;
    async fn is_active(&self, session_path: &Path) -> bool;
}

pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn AgentAdapter>>,
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AdapterRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            adapters: HashMap::new(),
        };
        reg.register(Box::new(claude::ClaudeAdapter::new()));
        reg.register(Box::new(codex::CodexAdapter::new()));
        reg.register(Box::new(copilot::CopilotAdapter::new()));
        reg.register(Box::new(cursor::CursorAdapter::new()));
        reg.register(Box::new(jetbrains::JetBrainsAdapter::new()));
        reg.register(Box::new(opencode::OpenCodeAdapter::new()));
        reg.register(Box::new(qoder::QoderAdapter::new()));
        reg.register(Box::new(warp::WarpAdapter::new()));
        reg.register(Box::new(antigravity::AntigravityAdapter::new()));
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
