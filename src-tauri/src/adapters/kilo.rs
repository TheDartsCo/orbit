use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

#[derive(Debug, Clone)]
struct CliSessionRow {
    id: String,
    title: String,
    directory: String,
    time_created: i64,
    time_updated: i64,
    model: String,
    tokens_input: i64,
    tokens_output: i64,
    tokens_reasoning: i64,
    tokens_cache_read: i64,
    tokens_cache_write: i64,
}

#[derive(Debug, Clone)]
struct CliMessageRow {
    id: String,
    session_id: String,
    time_created: i64,
    data: String,
}

#[derive(Debug, Clone)]
struct CliPartRow {
    _id: String,
    message_id: String,
    session_id: String,
    _time_created: i64,
    data: String,
}

#[derive(Debug, Clone, Default)]
struct CliSnapshot {
    sessions: Vec<CliSessionRow>,
    messages: HashMap<String, Vec<CliMessageRow>>,
    parts: HashMap<String, Vec<CliPartRow>>,
}

pub struct KiloAdapter {
    cli_cache: Mutex<Option<CliSnapshot>>,
}

impl Default for KiloAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiloAdapter {
    pub fn new() -> Self {
        Self {
            cli_cache: Mutex::new(None),
        }
    }

    pub(crate) fn windows_candidate_data_dirs(paths: &PlatformPaths) -> Vec<PathBuf> {
        [
            paths.data_local_join("kilo code.kilo-code/tasks"),
            paths.data_join("kilo code.kilo-code/tasks"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub(crate) fn windows_resume_command(session_id: &str, project_path: &str) -> String {
        let safe_path = crate::shell_quote::shell_quote(project_path);
        let safe_session = crate::shell_quote::shell_quote(session_id);
        format!("Set-Location {}; kilo --resume {}", safe_path, safe_session)
    }

    fn data_dirs() -> Vec<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            dirs::home_dir()
                .map(|home| home.join(".kilocode/globalStorage/kilo code.kilo-code/tasks"))
                .filter(|p| p.is_dir())
                .into_iter()
                .collect()
        } else if cfg!(target_os = "windows") {
            Self::windows_candidate_data_dirs(&PlatformPaths::system())
                .into_iter()
                .filter(|p| p.is_dir())
                .collect()
        } else {
            Vec::new()
        }
    }

    fn modified_at(path: &Path) -> DateTime<Utc> {
        std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_default()
    }

    fn extract_tool_output_from_result(content: &Value) -> Option<String> {
        content
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(ToString::to_string))
                    .collect::<String>()
                    .into()
            })
    }

    fn cli_db_path() -> Option<PathBuf> {
        let candidates: Vec<PathBuf> = if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            let mut paths = Vec::new();
            // XDG_DATA_HOME default (~/.local/share) — many CLI tools use this on macOS too
            if let Some(home) = dirs::home_dir() {
                paths.push(home.join(".local/share/kilo/kilo.db"));
            }
            if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
                paths.push(PathBuf::from(xdg).join("kilo/kilo.db"));
            }
            // macOS native data dir (~/Library/Application Support)
            if let Some(data) = dirs::data_dir() {
                paths.push(data.join("kilo/kilo.db"));
            }
            paths
        } else if cfg!(target_os = "windows") {
            PlatformPaths::system()
                .data_local_join("kilo/kilo.db")
                .into_iter()
                .chain(PlatformPaths::system().data_join("kilo/kilo.db"))
                .collect()
        } else {
            Vec::new()
        };
        candidates.into_iter().find(|p| p.is_file())
    }

    fn load_cli_cache(&self) {
        let mut cache = self.cli_cache.lock().unwrap();
        if cache.is_some() {
            return;
        }

        let db_path = match Self::cli_db_path() {
            Some(p) => p,
            None => {
                *cache = Some(CliSnapshot::default());
                return;
            }
        };

        let conn = match rusqlite::Connection::open_with_flags(
            &db_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => {
                *cache = Some(CliSnapshot::default());
                return;
            }
        };

        let mut snapshot = CliSnapshot::default();

        if let Ok(mut stmt) = conn
            .prepare("SELECT id, title, directory, model, time_created, time_updated, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write FROM session")
        {
            snapshot.sessions = stmt
                .query_map([], |row| {
                    Ok(CliSessionRow {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        directory: row.get(2)?,
                        model: row.get::<_, String>(3).unwrap_or_default(),
                        time_created: row.get(4)?,
                        time_updated: row.get(5)?,
                        tokens_input: row.get(6)?,
                        tokens_output: row.get(7)?,
                        tokens_reasoning: row.get(8)?,
                        tokens_cache_read: row.get(9)?,
                        tokens_cache_write: row.get(10)?,
                    })
                })
                .ok()
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default();
        }

        if let Ok(mut stmt) = conn
            .prepare("SELECT id, session_id, time_created, data FROM message ORDER BY time_created ASC")
        {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok(CliMessageRow {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    time_created: row.get(2)?,
                    data: row.get::<_, String>(3).unwrap_or_default(),
                })
            }) {
                for row in rows.flatten() {
                    snapshot.messages.entry(row.session_id.clone()).or_default().push(row);
                }
            }
        }

        if let Ok(mut stmt) = conn
            .prepare("SELECT id, message_id, session_id, time_created, data FROM part ORDER BY time_created ASC")
        {
            if let Ok(rows) = stmt.query_map([], |row| {
                Ok(CliPartRow {
                    _id: row.get(0)?,
                    message_id: row.get(1)?,
                    session_id: row.get(2)?,
                    _time_created: row.get(3)?,
                    data: row.get::<_, String>(4).unwrap_or_default(),
                })
            }) {
                for row in rows.flatten() {
                    snapshot.parts.entry(row.session_id.clone()).or_default().push(row);
                }
            }
        }

        *cache = Some(snapshot);
    }

    fn ts_to_dt(ts_ms: i64) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(ts_ms)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(Utc::now)
    }

    fn parse_cli_session(&self, session_id: &str) -> Result<NormalizedSession, String> {
        let cache = self.cli_cache.lock().unwrap();
        let snapshot = cache.as_ref().ok_or_else(|| "CLI cache not loaded".to_string())?;

        let session_row = snapshot
            .sessions
            .iter()
            .find(|s| s.id == session_id)
            .ok_or_else(|| format!("Session {} not found in CLI cache", session_id))?;

        let mut messages: Vec<Message> = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut seq: u32 = 0;
        let mut pending_tool_name: Option<String> = None;
        let mut pending_tool_input: Option<String> = None;

        let msg_rows = snapshot.messages.get(session_id).cloned().unwrap_or_default();
        let part_rows = snapshot.parts.get(session_id).cloned().unwrap_or_default();

        let created_at = Self::ts_to_dt(session_row.time_created);
        let updated_at = Self::ts_to_dt(session_row.time_updated);

        let project_path = session_row.directory.clone();

        let mut title = if !session_row.title.is_empty() {
            session_row.title.chars().take(100).collect()
        } else {
            "Untitled Session".to_string()
        };

        let model: Option<String> = {
            let parsed: Option<Value> = serde_json::from_str(&session_row.model).ok();
            parsed
                .and_then(|v| v.get("modelID").or_else(|| v.get("id")).and_then(|m| m.as_str().map(ToString::to_string)))
        };

        for msg_row in &msg_rows {
            let msg_data: Value = serde_json::from_str(&msg_row.data).unwrap_or_default();
            let role_str = msg_data.get("role").and_then(|r| r.as_str()).unwrap_or("");

            let parts_for_msg: Vec<&CliPartRow> = part_rows
                .iter()
                .filter(|p| p.message_id == msg_row.id)
                .collect();

            match role_str {
                "user" => {
                    let mut text_content = String::new();
                    let mut found_task_content = false;

                    for part in &parts_for_msg {
                        let part_data: Value = serde_json::from_str(&part.data).unwrap_or_default();
                        let part_type = part_data.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        if part_type == "text" {
                            if let Some(text) = part_data.get("text").and_then(|t| t.as_str()) {
                                if !found_task_content {
                                    let task = extract_task_content(text);
                                    if !task.is_empty() && title == "Untitled Session" {
                                        title = task;
                                        found_task_content = true;
                                    }
                                }
                                text_content.push_str(text);
                                text_content.push('\n');
                            }
                        }
                    }

                    let content = text_content.trim().to_string();
                    if !content.is_empty() {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: session_id.to_string(),
                            role: MessageRole::User,
                            content,
                            timestamp: Some(Self::ts_to_dt(msg_row.time_created)),
                            sequence: seq,
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                        });
                        seq += 1;
                    }
                }
                "assistant" => {
                    let mut has_text = false;
                    for part in &parts_for_msg {
                        let part_data: Value = serde_json::from_str(&part.data).unwrap_or_default();
                        let part_type = part_data.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        match part_type {
                            "text" => {
                                let text = part_data.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.trim().is_empty() {
                                    messages.push(Message {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        session_id: session_id.to_string(),
                                        role: MessageRole::Assistant,
                                        content: text.to_string(),
                                        timestamp: Some(Self::ts_to_dt(msg_row.time_created)),
                                        sequence: seq,
                                        tool_name: None,
                                        tool_input: None,
                                        tool_output: None,
                                    });
                                    seq += 1;
                                    has_text = true;
                                }
                            }
                            "reasoning" => {
                                let text = part_data.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.trim().is_empty() {
                                    messages.push(Message {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        session_id: session_id.to_string(),
                                        role: MessageRole::Assistant,
                                        content: text.to_string(),
                                        timestamp: Some(Self::ts_to_dt(msg_row.time_created)),
                                        sequence: seq,
                                        tool_name: None,
                                        tool_input: None,
                                        tool_output: None,
                                    });
                                    seq += 1;
                                }
                            }
                            "tool" => {
                                let tool_name = part_data.get("tool").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                let input = part_data
                                    .get("state")
                                    .and_then(|s| s.get("input"))
                                    .map(|i| i.to_string());
                                let output = part_data
                                    .get("state")
                                    .and_then(|s| s.get("output"))
                                    .and_then(|o| o.as_str().map(ToString::to_string));

                                let extracted_path = extract_tool_file_path(&tool_name, input.as_deref());
                                if let Some(ref p) = extracted_path {
                                    file_touches.push(FileTouch {
                                        path: p.clone(),
                                        operation: tool_operation_for(&tool_name).to_string(),
                                        sequence: seq,
                                    });
                                }

                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id.to_string(),
                                    role: MessageRole::Tool,
                                    content: String::new(),
                                    timestamp: Some(Self::ts_to_dt(msg_row.time_created)),
                                    sequence: seq,
                                    tool_name: Some(tool_name),
                                    tool_input: input,
                                    tool_output: output,
                                });
                                seq += 1;
                            }
                            "step-start" | "step-finish" => {}
                            "patch" => {
                                if let Some(files) = part_data.get("files").and_then(|f| f.as_array()) {
                                    for file in files {
                                        if let Some(path) = file.as_str() {
                                            file_touches.push(FileTouch {
                                                path: path.to_string(),
                                                operation: "edit".to_string(),
                                                sequence: seq,
                                            });
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if !has_text && pending_tool_name.is_some() {
                        if let (Some(name), Some(input)) = (pending_tool_name.take(), pending_tool_input.take()) {
                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.to_string(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: Some(Self::ts_to_dt(msg_row.time_created)),
                                sequence: seq,
                                tool_name: Some(name),
                                tool_input: Some(input),
                                tool_output: None,
                            });
                            seq += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        let session = Session {
            id: session_id.to_string(),
            parent_session_id: None,
            agent: AgentType::Kilo,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: format!("kilo-cli://session/{}", session_id),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            input_tokens: session_row.tokens_input as u64,
            output_tokens: session_row.tokens_output as u64,
            cached_tokens: session_row.tokens_cache_read as u64 + session_row.tokens_cache_write as u64,
            reasoning_tokens: session_row.tokens_reasoning as u64,
            file_count: 0,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }
}

#[async_trait]
impl AgentAdapter for KiloAdapter {
    fn id(&self) -> &str {
        "kilo"
    }

    fn name(&self) -> &str {
        "Kilo Code"
    }

    async fn detect(&self) -> bool {
        !Self::data_dirs().is_empty() || Self::cli_db_path().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let mut locations = Vec::new();

        // Scan extension sessions
        for data_dir in Self::data_dirs() {
            if let Ok(entries) = std::fs::read_dir(&data_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let api_history = path.join("api_conversation_history.json");
                        if api_history.is_file() {
                            locations.push(SessionLocation {
                                path: api_history.clone(),
                                last_modified: Self::modified_at(&api_history),
                            });
                        }
                    }
                }
            }
        }

        // Scan CLI sessions
        self.load_cli_cache();
        let cache = self.cli_cache.lock().unwrap();
        if let Some(ref snapshot) = *cache {
            for row in &snapshot.sessions {
                locations.push(SessionLocation {
                    path: PathBuf::from(format!("kilo-cli://session/{}", row.id)),
                    last_modified: Self::ts_to_dt(row.time_updated),
                });
            }
        }

        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let path_str = path.to_string_lossy();

        if let Some(session_id) = path_str.strip_prefix("kilo-cli://session/") {
            return self.parse_cli_session(session_id);
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read session file: {}", e))?;

        let json: Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse session JSON: {}", e))?;

        let messages_array = json.as_array().unwrap_or(&Vec::new()).clone();

        let session_id = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut messages: Vec<Message> = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut title = String::from("Untitled Session");
        let mut project_path = String::new();
        let mut seq: u32 = 0;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();

        for msg in &messages_array {
            let role_str = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let ts = msg
                .get("ts")
                .and_then(|t| t.as_i64())
                .and_then(|t| DateTime::from_timestamp_millis(t))
                .map(|dt| dt.with_timezone(&Utc));

            if seq == 0 {
                created_at = ts.unwrap_or_else(Utc::now);
            }
            updated_at = ts.unwrap_or_else(Utc::now);

            let content_blocks = msg.get("content").and_then(|c| c.as_array()).cloned().unwrap_or_default();

            match role_str {
                "user" => {
                    let is_tool_result_only = content_blocks.iter().all(|b| {
                        b.get("type").and_then(|t| t.as_str()) == Some("tool_result")
                    });

                    if is_tool_result_only {
                        for block in &content_blocks {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                if let Some(output) = block.get("content").and_then(|c| Self::extract_tool_output_from_result(c)) {
                                    for msg in messages.iter_mut().rev() {
                                        if msg.tool_name.is_some() && msg.tool_output.is_none() {
                                            msg.tool_output = Some(output.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        continue;
                    }

                    for block in &content_blocks {
                        let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");

                        if let Some(extracted_path) = extract_project_path(text) {
                            project_path = extracted_path;
                        }
                        let was_untitled = title == "Untitled Session";
                        let clean_text = extract_task_content(text);
                        if !clean_text.is_empty() && was_untitled {
                            title = clean_text.chars().take(100).collect();
                        }

                        if !text.is_empty() {
                            let msg_content = if was_untitled {
                                extract_task_content(text)
                            } else {
                                text.to_string()
                            };
                            if !msg_content.is_empty() {
                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id.clone(),
                                    role: MessageRole::User,
                                    content: msg_content,
                                    timestamp: ts,
                                    sequence: seq,
                                    tool_name: None,
                                    tool_input: None,
                                    tool_output: None,
                                });
                                seq += 1;
                            }
                        }
                    }
                }
                "assistant" => {
                    for block in &content_blocks {
                        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");

                        match block_type {
                            "text" => {
                                let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.is_empty() {
                                    messages.push(Message {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        session_id: session_id.clone(),
                                        role: MessageRole::Assistant,
                                        content: text.to_string(),
                                        timestamp: ts,
                                        sequence: seq,
                                        tool_name: None,
                                        tool_input: None,
                                        tool_output: None,
                                    });
                                    seq += 1;
                                }
                            }
                            "reasoning" => {
                                let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                                if !text.is_empty() {
                                    messages.push(Message {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        session_id: session_id.clone(),
                                        role: MessageRole::Assistant,
                                        content: text.to_string(),
                                        timestamp: ts,
                                        sequence: seq,
                                        tool_name: None,
                                        tool_input: None,
                                        tool_output: None,
                                    });
                                    seq += 1;
                                }
                            }
                            "tool_use" => {
                                let name = block.get("name").and_then(|t| t.as_str()).unwrap_or("").to_string();
                                let input = block.get("input").map(|i| i.to_string());

                                let extracted_path = extract_tool_file_path(&name, input.as_deref());

                                if extracted_path.is_some() {
                                    file_touches.push(FileTouch {
                                        path: extracted_path.clone().unwrap(),
                                        operation: tool_operation_for(&name).to_string(),
                                        sequence: seq,
                                    });
                                }

                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id.clone(),
                                    role: MessageRole::Tool,
                                    content: String::new(),
                                    timestamp: ts,
                                    sequence: seq,
                                    tool_name: Some(name),
                                    tool_input: input,
                                    tool_output: None,
                                });
                                seq += 1;
                            }
                            _ => {}
                        }
                    }
                }
                "system" => {
                    for block in &content_blocks {
                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            if !text.is_empty() {
                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id.clone(),
                                    role: MessageRole::System,
                                    content: text.to_string(),
                                    timestamp: ts,
                                    sequence: seq,
                                    tool_name: None,
                                    tool_input: None,
                                    tool_output: None,
                                });
                                seq += 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let session = Session {
            id: session_id.clone(),
            parent_session_id: None,
            agent: AgentType::Kilo,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }

    fn resume_command(&self, session_id: &str, project_path: &str) -> String {
        if cfg!(target_os = "windows") {
            return Self::windows_resume_command(session_id, project_path);
        }

        let safe_path = crate::shell_quote::shell_quote(project_path);
        let safe_session = crate::shell_quote::shell_quote(session_id);
        format!("cd {} && kilo --resume {}", safe_path, safe_session)
    }

    async fn is_active(&self, session_path: &Path) -> bool {
        let path_str = session_path.to_string_lossy();
        if path_str.starts_with("kilo-cli://session/") {
            if let Some(db_path) = Self::cli_db_path() {
                return std::fs::metadata(&db_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    })
                    .map(|mtime| {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        now.saturating_sub(mtime) < 300
                    })
                    .unwrap_or(false);
            }
            return false;
        }
        false
    }
}

fn extract_project_path(text: &str) -> Option<String> {
    for line in text.lines() {
        if line.contains("# Current Workspace Directory (") && line.contains(")" ) {
            let start = line.find('(')?;
            let end = line.rfind(')')?;
            if end > start {
                return Some(line[start + 1..end].to_string());
            }
        }
    }
    None
}

fn extract_task_content(text: &str) -> String {
    let mut content = String::new();
    let mut in_task = false;

    for line in text.lines() {
        if line.trim() == "<task>" {
            in_task = true;
            continue;
        }
        if in_task {
            if line.contains("</task>") {
                let before_end = line.split("</task>").next().unwrap_or("").trim();
                if !before_end.is_empty() {
                    content.push_str(before_end);
                    content.push('\n');
                }
                break;
            }
            content.push_str(line.trim());
            content.push('\n');
        }
    }

    content.trim().to_string()
}

fn tool_operation_for(tool_name: &str) -> &'static str {
    match tool_name {
        "read_file" => "read",
        "edit_file" => "edit",
        "write_to_file" => "write",
        "write_file" => "write",
        _ => "unknown",
    }
}

fn extract_tool_file_path(tool_name: &str, input: Option<&str>) -> Option<String> {
    let raw = input?;
    let parsed: Value = serde_json::from_str(raw).ok()?;

    match tool_name {
        "read_file" => parsed
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|arr| arr.first())
            .and_then(|f| f.get("path"))
            .and_then(|p| p.as_str())
            .map(ToString::to_string),
        "write_file" | "write_to_file" => parsed
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(ToString::to_string),
        "edit_file" => parsed
            .get("file_path")
            .and_then(|p| p.as_str())
            .map(ToString::to_string),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;

    fn write_kilo_session(dir: &std::path::Path, session_id: &str, content: &str) -> std::path::PathBuf {
        let task_dir = dir.join(session_id);
        std::fs::create_dir_all(&task_dir).unwrap();
        let path = task_dir.join("api_conversation_history.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn candidate_data_dirs_includes_kilocode_tasks() {
        let paths = PlatformPaths {
            home: Some(std::path::PathBuf::from("/home/test")),
            data: Some(std::path::PathBuf::from("/home/test/.local/share")),
            data_local: Some(std::path::PathBuf::from("/home/test/.local/share")),
        };
        let candidates = KiloAdapter::windows_candidate_data_dirs(&paths);
        assert!(candidates.iter().any(|p| p.ends_with("kilo code.kilo-code/tasks")));
    }

    #[tokio::test]
    async fn parses_kilo_session_with_tool_calls() {
        let tmp = tempfile::tempdir().unwrap();
        let jsonl = r#"[
            {"role":"user","ts":1773355464835,"content":[{"type":"text","text":"<task>\nTest task\n</task><environment_details>\n# Current Workspace Directory (/Users/test/project)\n</environment_details>"}]},
            {"role":"assistant","ts":1773355475539,"content":[{"type":"text","text":"I will help with that."}]},
            {"role":"assistant","ts":1773355475790,"content":[{"type":"tool_use","id":"call_1","name":"read_file","input":{"files":[{"path":"src/main.rs"}]}}]},
            {"role":"user","ts":1773355475789,"content":[{"type":"tool_result","content":[{"type":"text","text":"File contents here"}]}]}
        ]"#;
        let path = write_kilo_session(tmp.path(), "test-session-1", jsonl);
        let adapter = KiloAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.id, "test-session-1");
        assert_eq!(parsed.session.agent, AgentType::Kilo);
        assert_eq!(parsed.session.project_path, "/Users/test/project");
        assert_eq!(parsed.session.title, "Test task");
        assert_eq!(parsed.messages.len(), 3);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Test task");
        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("read_file"));
        assert_eq!(parsed.messages[2].tool_output.as_deref(), Some("File contents here"));
        assert_eq!(parsed.file_touches.len(), 1);
    }

    #[tokio::test]
    async fn extracts_project_path_from_environment_details() {
        let path = extract_project_path("# Current Workspace Directory (/Users/test/project)\nmore text");
        assert_eq!(path, Some("/Users/test/project".to_string()));
    }

    #[tokio::test]
    async fn windows_resume_command_is_powershell_compatible() {
        let cmd = KiloAdapter::windows_resume_command("abc123", r"C:\Projects\Test");
        assert_eq!(cmd, "Set-Location 'C:\\Projects\\Test'; kilo --resume 'abc123'");
    }

    #[tokio::test]
    async fn extracts_file_path_from_read_file_tool() {
        let input = r#"{"files":[{"path":"src/lib.rs"}]}"#;
        let extracted = extract_tool_file_path("read_file", Some(input));
        assert_eq!(extracted, Some("src/lib.rs".to_string()));
    }

    #[tokio::test]
    async fn extracts_file_path_from_edit_file_tool() {
        let input = r#"{"file_path":"src/main.rs"}"#;
        let extracted = extract_tool_file_path("edit_file", Some(input));
        assert_eq!(extracted, Some("src/main.rs".to_string()));
    }

    #[tokio::test]
    async fn parses_kilo_cli_session_from_cache() {
        let adapter = KiloAdapter::new();
        let session_id = "ses_test123";

        // Build a mock CLI snapshot
        let mut snapshot = CliSnapshot::default();
        snapshot.sessions.push(CliSessionRow {
            id: session_id.to_string(),
            title: "Test CLI session".to_string(),
            directory: "/Users/test/myproject".to_string(),
            model: r#"{"providerID":"kilo","modelID":"kilo-auto/free"}"#.to_string(),
            time_created: 1782215834654,
            time_updated: 1782471067294,
            tokens_input: 5000,
            tokens_output: 1200,
            tokens_reasoning: 300,
            tokens_cache_read: 100,
            tokens_cache_write: 50,
        });

        let msg1_id = "msg_aaa".to_string();
        let msg2_id = "msg_bbb".to_string();

        snapshot.messages.insert(
            session_id.to_string(),
            vec![
                CliMessageRow {
                    id: msg1_id.clone(),
                    session_id: session_id.to_string(),
                    time_created: 1782215834679,
                    data: r#"{"role":"user","mode":"code","agent":"code"}"#.to_string(),
                },
                CliMessageRow {
                    id: msg2_id.clone(),
                    session_id: session_id.to_string(),
                    time_created: 1782215834710,
                    data: r#"{"role":"assistant","mode":"code","agent":"code","finish":"tool-calls"}"#.to_string(),
                },
            ],
        );

        snapshot.parts.insert(
            session_id.to_string(),
            vec![
                CliPartRow {
                    _id: "prt_1".to_string(),
                    message_id: msg1_id.clone(),
                    session_id: session_id.to_string(),
                    _time_created: 1782215834679,
                    data: r#"{"type":"text","text":"Fix the login bug"}"#.to_string(),
                },
                CliPartRow {
                    _id: "prt_2".to_string(),
                    message_id: msg2_id.clone(),
                    session_id: session_id.to_string(),
                    _time_created: 1782215834710,
                    data: r#"{"type":"text","text":"I'll fix the login bug now."}"#.to_string(),
                },
                CliPartRow {
                    _id: "prt_3".to_string(),
                    message_id: msg2_id.clone(),
                    session_id: session_id.to_string(),
                    _time_created: 1782215834715,
                    data: r#"{"type":"tool","tool":"read_file","state":{"status":"completed","input":{"files":[{"path":"src/login.ts"}]},"output":"export function login() {}"}}"#.to_string(),
                },
            ],
        );

        // Inject into adapter cache
        *adapter.cli_cache.lock().unwrap() = Some(snapshot);

        let path = PathBuf::from(format!("kilo-cli://session/{}", session_id));
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.agent, AgentType::Kilo);
        assert_eq!(parsed.session.title, "Test CLI session");
        assert_eq!(parsed.session.project_path, "/Users/test/myproject");
        assert_eq!(parsed.session.model.as_deref(), Some("kilo-auto/free"));
        assert_eq!(parsed.session.input_tokens, 5000);
        assert_eq!(parsed.session.output_tokens, 1200);
        assert_eq!(parsed.session.reasoning_tokens, 300);
        assert_eq!(parsed.messages.len(), 3);

        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert!(parsed.messages[0].content.contains("Fix the login bug"));

        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert!(parsed.messages[1].content.contains("I'll fix the login bug"));

        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("read_file"));
        assert_eq!(parsed.messages[2].tool_output.as_deref(), Some("export function login() {}"));
        assert_eq!(parsed.file_touches.len(), 1);
        assert_eq!(parsed.file_touches[0].path, "src/login.ts");
        assert_eq!(parsed.file_touches[0].operation, "read");
    }
}