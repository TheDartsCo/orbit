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

#[tauri::command]
pub fn get_platform() -> &'static str {
    std::env::consts::OS
}

#[cfg(any(target_os = "macos", test))]
fn terminal_applescript() -> &'static str {
    "on run argv\n\
     tell application \"Terminal\"\n\
     activate\n\
     do script (item 1 of argv)\n\
     end tell\n\
     end run"
}

#[cfg(any(target_os = "macos", test))]
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

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxTerminalArgStyle {
    DoubleDashArgs,
    DashEArgs,
    DashXArgs,
    DashEString,
    Xdg,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct LinuxTerminalDefinition {
    id: &'static str,
    name: &'static str,
    executable: &'static str,
    arg_style: LinuxTerminalArgStyle,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedLinuxTerminal {
    executable: String,
    arg_style: LinuxTerminalArgStyle,
}

#[cfg(target_os = "linux")]
const LINUX_TERMINALS: &[LinuxTerminalDefinition] = &[
    LinuxTerminalDefinition {
        id: "xdg-terminal-exec",
        name: "XDG Terminal",
        executable: "xdg-terminal-exec",
        arg_style: LinuxTerminalArgStyle::Xdg,
    },
    LinuxTerminalDefinition {
        id: "kgx",
        name: "GNOME Console",
        executable: "kgx",
        arg_style: LinuxTerminalArgStyle::DashEString,
    },
    LinuxTerminalDefinition {
        id: "gnome-terminal",
        name: "GNOME Terminal",
        executable: "gnome-terminal",
        arg_style: LinuxTerminalArgStyle::DoubleDashArgs,
    },
    LinuxTerminalDefinition {
        id: "konsole",
        name: "Konsole",
        executable: "konsole",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
    LinuxTerminalDefinition {
        id: "xfce4-terminal",
        name: "XFCE Terminal",
        executable: "xfce4-terminal",
        arg_style: LinuxTerminalArgStyle::DashXArgs,
    },
    LinuxTerminalDefinition {
        id: "mate-terminal",
        name: "MATE Terminal",
        executable: "mate-terminal",
        arg_style: LinuxTerminalArgStyle::DashXArgs,
    },
    LinuxTerminalDefinition {
        id: "tilix",
        name: "Tilix",
        executable: "tilix",
        arg_style: LinuxTerminalArgStyle::DashEString,
    },
    LinuxTerminalDefinition {
        id: "alacritty",
        name: "Alacritty",
        executable: "alacritty",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
    LinuxTerminalDefinition {
        id: "kitty",
        name: "Kitty",
        executable: "kitty",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
    LinuxTerminalDefinition {
        id: "ghostty",
        name: "Ghostty",
        executable: "ghostty",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
    LinuxTerminalDefinition {
        id: "wezterm",
        name: "WezTerm",
        executable: "wezterm",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
    LinuxTerminalDefinition {
        id: "xterm",
        name: "xterm",
        executable: "xterm",
        arg_style: LinuxTerminalArgStyle::DashEArgs,
    },
];

#[cfg(target_os = "linux")]
fn linux_terminal_args(style: LinuxTerminalArgStyle, command: &str) -> Vec<String> {
    match style {
        LinuxTerminalArgStyle::DoubleDashArgs => vec![
            "--".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            command.to_string(),
        ],
        LinuxTerminalArgStyle::DashEArgs => vec![
            "-e".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            command.to_string(),
        ],
        LinuxTerminalArgStyle::DashXArgs => vec![
            "-x".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            command.to_string(),
        ],
        LinuxTerminalArgStyle::DashEString => vec![
            "-e".to_string(),
            format!("sh -lc {}", crate::shell_quote::shell_quote(command)),
        ],
        LinuxTerminalArgStyle::Xdg => {
            vec!["sh".to_string(), "-lc".to_string(), command.to_string()]
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_definition_for_id(id: &str) -> Option<&'static LinuxTerminalDefinition> {
    LINUX_TERMINALS.iter().find(|terminal| terminal.id == id)
}

#[cfg(target_os = "linux")]
fn linux_arg_style_for_executable(executable: &str) -> LinuxTerminalArgStyle {
    let basename = std::path::Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable);

    LINUX_TERMINALS
        .iter()
        .find(|terminal| terminal.executable == basename)
        .map(|terminal| terminal.arg_style)
        .unwrap_or(LinuxTerminalArgStyle::DashEArgs)
}

#[cfg(target_os = "linux")]
fn resolve_linux_terminal_with<F>(
    preferred: Option<&str>,
    terminal_env: Option<&str>,
    is_available: F,
) -> Option<ResolvedLinuxTerminal>
where
    F: Fn(&str) -> bool,
{
    if let Some(preferred) = preferred.and_then(linux_definition_for_id) {
        if is_available(preferred.executable) {
            return Some(ResolvedLinuxTerminal {
                executable: preferred.executable.to_string(),
                arg_style: preferred.arg_style,
            });
        }
    }

    if let Some(terminal_env) = terminal_env.filter(|value| !value.trim().is_empty()) {
        if is_available(terminal_env) {
            return Some(ResolvedLinuxTerminal {
                executable: terminal_env.to_string(),
                arg_style: linux_arg_style_for_executable(terminal_env),
            });
        }
    }

    LINUX_TERMINALS.iter().find_map(|terminal| {
        is_available(terminal.executable).then(|| ResolvedLinuxTerminal {
            executable: terminal.executable.to_string(),
            arg_style: terminal.arg_style,
        })
    })
}

#[cfg(target_os = "linux")]
fn linux_terminal_infos_with<F>(is_available: F) -> Vec<TerminalInfo>
where
    F: Fn(&str) -> bool,
{
    LINUX_TERMINALS
        .iter()
        .map(|terminal| TerminalInfo {
            id: terminal.id.to_string(),
            name: terminal.name.to_string(),
            available: is_available(terminal.executable),
        })
        .collect()
}

#[cfg(target_os = "linux")]
fn linux_path_is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn linux_executable_available(executable: &str) -> bool {
    let executable_path = std::path::Path::new(executable);
    if executable_path.components().count() > 1 {
        return linux_path_is_executable(executable_path);
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| linux_path_is_executable(&dir.join(executable)))
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn resolve_linux_terminal(preferred: Option<&str>) -> Option<ResolvedLinuxTerminal> {
    let terminal_env = std::env::var("TERMINAL").ok();
    resolve_linux_terminal_with(
        preferred,
        terminal_env.as_deref(),
        linux_executable_available,
    )
}

#[cfg(target_os = "linux")]
fn linux_missing_terminal_error() -> String {
    let supported = LINUX_TERMINALS
        .iter()
        .map(|terminal| terminal.id)
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "No supported Linux terminal found. Install one of: {}. You can also set TERMINAL to an executable terminal command.",
        supported
    )
}

#[cfg(target_os = "linux")]
fn launch_linux_terminal(command: &str, preferred: Option<&str>) -> Result<(), String> {
    let terminal = resolve_linux_terminal(preferred).ok_or_else(linux_missing_terminal_error)?;

    let mut process = std::process::Command::new(&terminal.executable);
    for arg in linux_terminal_args(terminal.arg_style, command) {
        process.arg(arg);
    }

    process
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch terminal {}: {}", terminal.executable, e))
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

    if !adapter.supports_resume() {
        return Err(format!("{} sessions do not support resume", adapter.name()));
    }

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

    #[cfg(target_os = "macos")]
    {
        let terminal = preferred.as_deref().unwrap_or("terminal");
        match terminal {
            "iterm" | "warp" | "ghostty" => {
                let app_name = script_terminal_app_name(terminal)
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
        launch_linux_terminal(&cmd, preferred.as_deref())?;
    }

    #[cfg(target_os = "windows")]
    {
        let _ = cmd;
        return Err(
            "Automatic resume is not supported on Windows yet. Copy the command instead."
                .to_string(),
        );
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
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "linux")]
    {
        Ok(linux_terminal_infos_with(linux_executable_available))
    }

    #[cfg(target_os = "windows")]
    {
        Ok(vec![TerminalInfo {
            id: "cmd".to_string(),
            name: "Command Prompt".to_string(),
            available: true,
        }])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
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
    queries.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_terminal_args_match_terminal_definitions() {
        let cases: &[(&str, &[&str])] = &[
            ("xdg-terminal-exec", &["sh", "-lc", "echo hello"]),
            ("gnome-terminal", &["--", "sh", "-lc", "echo hello"]),
            ("kgx", &["-e", "sh -lc 'echo hello'"]),
            ("xfce4-terminal", &["-x", "sh", "-lc", "echo hello"]),
            ("mate-terminal", &["-x", "sh", "-lc", "echo hello"]),
            ("tilix", &["-e", "sh -lc 'echo hello'"]),
            ("konsole", &["-e", "sh", "-lc", "echo hello"]),
            ("alacritty", &["-e", "sh", "-lc", "echo hello"]),
            ("kitty", &["-e", "sh", "-lc", "echo hello"]),
            ("ghostty", &["-e", "sh", "-lc", "echo hello"]),
            ("wezterm", &["-e", "sh", "-lc", "echo hello"]),
            ("xterm", &["-e", "sh", "-lc", "echo hello"]),
        ];

        for (terminal_id, expected) in cases {
            let terminal = linux_definition_for_id(terminal_id).unwrap();
            assert_eq!(
                linux_terminal_args(terminal.arg_style, "echo hello"),
                expected
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<_>>(),
                "unexpected argv for {terminal_id}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn preferred_linux_terminal_wins_when_available() {
        let resolved = resolve_linux_terminal_with(Some("kitty"), None, |exe| {
            exe == "xdg-terminal-exec" || exe == "kitty"
        })
        .unwrap();

        assert_eq!(resolved.executable, "kitty");
        assert_eq!(resolved.arg_style, LinuxTerminalArgStyle::DashEArgs);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn terminal_env_is_used_before_fallbacks() {
        let terminal_env = "/tmp/orbit-custom-terminal";
        let resolved = resolve_linux_terminal_with(None, Some(terminal_env), |exe| {
            exe == terminal_env || exe == "xdg-terminal-exec"
        })
        .unwrap();

        assert_eq!(resolved.executable, terminal_env);
        assert_eq!(resolved.arg_style, LinuxTerminalArgStyle::DashEArgs);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_terminal_detection_reports_common_terminals() {
        let terminals = linux_terminal_infos_with(|exe| exe == "kgx" || exe == "xterm");

        assert!(terminals.iter().any(|t| t.id == "kgx" && t.available));
        assert!(terminals.iter().any(|t| t.id == "xterm" && t.available));
        assert!(terminals
            .iter()
            .any(|t| t.id == "gnome-terminal" && !t.available));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_executable_available_requires_execute_bit_for_direct_paths() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!(
            "orbit-non-executable-terminal-{}",
            uuid::Uuid::new_v4()
        ));
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh").unwrap();

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!linux_executable_available(&path.to_string_lossy()));

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(linux_executable_available(&path.to_string_lossy()));

        std::fs::remove_file(path).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_missing_terminal_error_lists_all_supported_terminals() {
        let error = linux_missing_terminal_error();

        for terminal in LINUX_TERMINALS {
            assert!(
                error.contains(terminal.id),
                "missing terminal id {} in {}",
                terminal.id,
                error
            );
        }
        assert!(error.contains("TERMINAL"));
    }

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

    #[test]
    fn platform_name_matches_the_compilation_target() {
        assert_eq!(get_platform(), std::env::consts::OS);
    }
}
