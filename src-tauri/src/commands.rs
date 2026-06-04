use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;

use crate::adapters::AdapterRegistry;
use crate::db::queries::DbQueries;
use crate::indexer::{IndexStats, Indexer, ProviderSyncStats};
use crate::models::*;

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    pub registry: Arc<AdapterRegistry>,
    pub indexer: Arc<Indexer>,
}

fn terminal_applescript() -> &'static str {
    "on run argv\n\
     tell application \"Terminal\"\n\
     activate\n\
     do script (item 1 of argv)\n\
     end tell\n\
     end run"
}

fn script_terminal_app_name(terminal: &str) -> Option<&'static str> {
    match terminal {
        "iterm" => Some("iTerm"),
        "warp" => Some("Warp"),
        "ghostty" => Some("Ghostty"),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str, command: &str) -> Result<(), String> {
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .arg("--")
        .arg(command)
        .output()
        .map_err(|e| format!("Failed to launch osascript: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            format!("osascript exited with {}", output.status)
        } else {
            stderr
        })
    }
}

#[tauri::command]
pub async fn get_sessions(
    state: State<'_, AppState>,
    filters: SessionFilters,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<Vec<Session>, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    let result = queries
        .get_sessions(&filters, offset.unwrap_or(0), limit.unwrap_or(100))
        .map_err(|e| e.to_string());
    match &result {
        Ok(sessions) => tracing::info!("get_sessions returned {} sessions", sessions.len()),
        Err(e) => tracing::error!("get_sessions error: {}", e),
    }
    result
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, AppState>,
    session_id: String,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    queries
        .get_messages(&session_id, offset.unwrap_or(0), limit.unwrap_or(500))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_sessions(
    state: State<'_, AppState>,
    query: String,
    _filters: SessionFilters,
    limit: Option<u32>,
) -> Result<Vec<Message>, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    queries
        .search_messages(&query, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_resume_command(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<String, String> {
    let db = state.db.lock().await;

    let session = {
        let mut stmt = db
            .prepare("SELECT id, agent, project_path FROM sessions WHERE id = ?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(rusqlite::params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            ))
        })
        .map_err(|e| e.to_string())
    }?;

    let adapter = state
        .registry
        .get(&session.1)
        .ok_or_else(|| format!("Adapter {} not found", session.1))?;

    Ok(adapter.resume_command(&session.0, &session.2))
}

#[tauri::command]
pub async fn launch_resume(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let preferred = {
        let db = state.db.lock().await;
        let queries = DbQueries::new(&db);
        queries.get_setting("preferred_terminal").ok().flatten()
    };

    let cmd = get_resume_command(state, session_id).await?;

    let terminal = preferred.unwrap_or_else(|| "terminal".to_string());

    #[cfg(target_os = "macos")]
    {
        match terminal.as_str() {
            "iterm" | "warp" | "ghostty" => {
                let app_name = script_terminal_app_name(&terminal)
                    .ok_or_else(|| format!("Unsupported terminal: {}", terminal))?;
                let tmp_dir = std::env::temp_dir();
                let script_path =
                    tmp_dir.join(format!("orbit-resume-{}.command", uuid::Uuid::new_v4()));
                std::fs::write(&script_path, format!("#!/bin/sh\n{}\n", cmd))
                    .map_err(|e| format!("Failed to write temp script: {}", e))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("Failed to chmod script: {}", e))?;
                }
                std::process::Command::new("open")
                    .args(["-a", app_name, &script_path.to_string_lossy()])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                run_osascript(terminal_applescript(), &cmd)?;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("xterm -e {} &", cmd))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &cmd])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn get_active_sessions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.indexer.check_active_sessions().await
}

#[tauri::command]
pub async fn reindex_all(state: State<'_, AppState>) -> Result<IndexStats, String> {
    tracing::info!("Starting reindex_all");
    let result = state.indexer.index_all().await;
    match &result {
        Ok(stats) => tracing::info!("Reindex complete: {:?}", stats),
        Err(e) => tracing::error!("Reindex failed: {}", e),
    }
    result
}

#[derive(serde::Serialize)]
pub struct SyncStatus {
    pub last_sync_at: Option<String>,
    pub provider_stats: HashMap<String, ProviderSyncStats>,
}

#[tauri::command]
pub async fn get_sync_status(state: State<'_, AppState>) -> Result<SyncStatus, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);

    let last_sync_at = queries
        .get_setting("last_sync_at")
        .map_err(|e| e.to_string())?;

    let provider_stats = queries
        .get_setting("last_sync_stats")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<HashMap<String, ProviderSyncStats>>(&json).ok())
        .unwrap_or_default();

    Ok(SyncStatus {
        last_sync_at,
        provider_stats,
    })
}

#[tauri::command]
pub async fn detect_terminals() -> Result<Vec<TerminalInfo>, String> {
    let terminals = vec![
        TerminalInfo {
            id: "terminal".to_string(),
            name: "Terminal".to_string(),
            available: true,
        },
        TerminalInfo {
            id: "iterm".to_string(),
            name: "iTerm2".to_string(),
            available: std::path::Path::new("/Applications/iTerm.app").exists(),
        },
        TerminalInfo {
            id: "warp".to_string(),
            name: "Warp".to_string(),
            available: std::path::Path::new("/Applications/Warp.app").exists(),
        },
        TerminalInfo {
            id: "ghostty".to_string(),
            name: "Ghostty".to_string(),
            available: std::path::Path::new("/Applications/Ghostty.app").exists(),
        },
    ];
    Ok(terminals)
}

#[derive(serde::Serialize)]
pub struct AppSettings {
    pub preferred_terminal: Option<String>,
}

#[tauri::command]
pub async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    let preferred_terminal = queries
        .get_setting("preferred_terminal")
        .map_err(|e| e.to_string())?;
    Ok(AppSettings { preferred_terminal })
}

#[tauri::command]
pub async fn save_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    queries
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_applescript_reads_command_from_argv() {
        let command = r#"cd '/tmp/quoted path' && printf '"hello" \ world'"#;
        let script = terminal_applescript();

        assert!(script.contains("do script (item 1 of argv)"));
        assert!(!script.contains(command));
    }

    #[test]
    fn iterm_uses_an_executable_command_file() {
        assert_eq!(script_terminal_app_name("iterm"), Some("iTerm"));
    }
}
