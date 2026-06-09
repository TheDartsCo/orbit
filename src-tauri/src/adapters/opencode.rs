use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

pub struct OpenCodeAdapter;

fn opencode_file_operation_for(tool_name: &str) -> &'static str {
    match tool_name {
        "read" | "Read" => "read",
        "edit" | "Edit" | "patch" => "edit",
        "write" | "Write" => "write",
        "delete" | "Delete" => "delete",
        _ => "unknown",
    }
}

fn opencode_extract_file_path(input: &Value) -> Option<String> {
    if let Some(s) = input.get("filePath").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = input.get("file_path").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = input.get("path").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    None
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn candidate_data_dirs_from_sources(
        home: Option<PathBuf>,
        data_local: Option<PathBuf>,
        data: Option<PathBuf>,
        config: Option<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let mut seen = HashSet::new();

        let mut push = |path: Option<PathBuf>| {
            if let Some(path) = path {
                if seen.insert(path.clone()) {
                    dirs.push(path);
                }
            }
        };

        push(home.map(|home| home.join(".local/share/opencode")));
        push(data_local.map(|dir| dir.join("opencode")));
        push(data.map(|dir| dir.join("opencode")));
        push(config.map(|dir| dir.join("opencode")));

        dirs
    }

    pub(crate) fn windows_candidate_data_dirs(paths: &PlatformPaths) -> Vec<PathBuf> {
        [
            paths.home_join(".local/share/opencode"),
            paths.data_local_join("opencode"),
            paths.data_join("opencode"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub(crate) fn windows_resume_command(session_id: &str, project_path: &str) -> String {
        let safe_path = crate::shell_quote::shell_quote(project_path);
        let safe_session = crate::shell_quote::shell_quote(session_id);
        format!(
            "Set-Location {}; opencode --session {}",
            safe_path, safe_session
        )
    }

    fn candidate_data_dirs() -> Vec<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Self::candidate_data_dirs_from_sources(
                dirs::home_dir(),
                dirs::data_local_dir(),
                dirs::data_dir(),
                dirs::config_dir(),
            )
        } else if cfg!(target_os = "windows") {
            Self::windows_candidate_data_dirs(&PlatformPaths::system())
        } else {
            Vec::new()
        }
    }

    fn has_known_store(dir: &Path) -> bool {
        dir.join("opencode.db").is_file()
            || dir.join("storage/session").is_dir()
            || dir.join("sessions").is_dir()
    }

    fn existing_data_dirs_from_candidates(candidates: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let mut seen = HashSet::new();

        for dir in candidates {
            if !seen.insert(dir.clone()) || !Self::has_known_store(&dir) {
                continue;
            }
            dirs.push(dir);
        }

        dirs
    }

    fn existing_data_dirs() -> Vec<PathBuf> {
        Self::existing_data_dirs_from_candidates(Self::candidate_data_dirs())
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

    fn scan_storage_sessions(dir: &Path, locations: &mut Vec<SessionLocation>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    Self::scan_storage_sessions(&path, locations);
                } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    locations.push(SessionLocation {
                        last_modified: Self::modified_at(&path),
                        path,
                    });
                }
            }
        }
    }

    fn scan_db_sessions(data_dir: &Path, locations: &mut Vec<SessionLocation>) {
        let db_path = data_dir.join("opencode.db");
        let conn = match Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to open OpenCode DB at {:?}: {}", db_path, e);
                return;
            }
        };
        let mut stmt = match conn.prepare("SELECT id, time_updated FROM session") {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to prepare OpenCode session query: {}", e);
                return;
            }
        };
        let rows = match stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("Failed to query_map OpenCode sessions: {}", e);
                return;
            }
        };

        let marker_dir = data_dir.join("storage/session_diff");
        for row in rows {
            match row {
                Ok((id, time_updated)) => {
                    locations.push(SessionLocation {
                        last_modified: Self::timestamp_from_millis(Some(time_updated)).unwrap_or_default(),
                        path: marker_dir.join(format!("{}.json", id)),
                    });
                }
                Err(e) => tracing::error!("Failed to read OpenCode session row: {}", e),
            }
        }
    }

    fn scan_legacy_jsonl(dir: &Path, locations: &mut Vec<SessionLocation>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name == "node_modules" || name == "snapshot" || name == "storage" {
                        continue;
                    }
                    Self::scan_legacy_jsonl(&path, locations);
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    locations.push(SessionLocation {
                        last_modified: Self::modified_at(&path),
                        path,
                    });
                }
            }
        }
    }

    fn scan_data_dirs(data_dirs: Vec<PathBuf>) -> Vec<SessionLocation> {
        let mut locations = Vec::new();
        let mut seen_dirs = HashSet::new();
        let mut seen_paths = HashSet::new();

        for data_dir in data_dirs {
            if !seen_dirs.insert(data_dir.clone()) {
                continue;
            }

            let mut root_locations = Vec::new();
            if data_dir.join("opencode.db").is_file() {
                Self::scan_db_sessions(&data_dir, &mut root_locations);
            } else {
                Self::scan_storage_sessions(&data_dir.join("storage/session"), &mut root_locations);
            }
            Self::scan_legacy_jsonl(&data_dir.join("sessions"), &mut root_locations);

            for location in root_locations {
                if seen_paths.insert(location.path.clone()) {
                    locations.push(location);
                }
            }
        }

        locations
    }

    fn data_dir_from_storage_path(path: &Path) -> Option<PathBuf> {
        for ancestor in path.ancestors() {
            let Some(name) = ancestor.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if name == "session" || name == "session_diff" {
                let storage = ancestor.parent()?;
                if storage.file_name().and_then(|name| name.to_str()) == Some("storage") {
                    return storage.parent().map(Path::to_path_buf);
                }
            }
        }

        None
    }

    fn is_session_diff_marker(path: &Path) -> bool {
        path.parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some("session_diff")
    }

    fn timestamp_from_millis(value: Option<i64>) -> Option<DateTime<Utc>> {
        let millis = value?;
        DateTime::from_timestamp_millis(millis)
    }

    fn json_i64_at(json: &Value, path: &[&str]) -> Option<i64> {
        let mut current = json;
        for key in path {
            current = current.get(*key)?;
        }
        current.as_i64()
    }

    fn role_from_str(role: &str) -> Option<MessageRole> {
        match role {
            "user" | "human" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            "tool" => Some(MessageRole::Tool),
            _ => None,
        }
    }

    fn read_json(path: &Path) -> Option<Value> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn read_json_files_sorted(dir: &Path) -> Vec<(PathBuf, Value)> {
        let mut files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(json) = Self::read_json(&path) {
                        files.push((path, json));
                    }
                }
            }
        }

        files.sort_by(|(left_path, left_json), (right_path, right_json)| {
            let left_time = Self::json_i64_at(left_json, &["time", "created"])
                .or_else(|| Self::json_i64_at(left_json, &["time", "start"]))
                .unwrap_or_default();
            let right_time = Self::json_i64_at(right_json, &["time", "created"])
                .or_else(|| Self::json_i64_at(right_json, &["time", "start"]))
                .unwrap_or_default();

            left_time
                .cmp(&right_time)
                .then_with(|| left_path.cmp(right_path))
        });

        files
    }

    fn merge_part_json(
        part: &Value,
        content_parts: &mut Vec<String>,
        tool_name: &mut Option<String>,
        tool_input: &mut Option<String>,
        tool_output: &mut Option<String>,
    ) {
        match part.get("type").and_then(|value| value.as_str()) {
            Some("text") => {
                if let Some(text) = part.get("text").and_then(|value| value.as_str()) {
                    if !text.is_empty() {
                        content_parts.push(text.to_string());
                    }
                }
            }
            Some("tool") => {
                if tool_name.is_none() {
                    *tool_name = part
                        .get("tool")
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string);
                }
                if tool_input.is_none() {
                    *tool_input = part
                        .get("state")
                        .and_then(|state| state.get("input"))
                        .map(Value::to_string);
                }
                if tool_output.is_none() {
                    *tool_output = part
                        .get("state")
                        .and_then(|state| state.get("output"))
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string);
                }
            }
            _ => {}
        }
    }

    fn content_from_parts(
        parts_dir: &Path,
    ) -> (String, Option<String>, Option<String>, Option<String>) {
        let mut content_parts = Vec::new();
        let mut tool_name = None;
        let mut tool_input = None;
        let mut tool_output = None;

        for (_, part) in Self::read_json_files_sorted(parts_dir) {
            Self::merge_part_json(
                &part,
                &mut content_parts,
                &mut tool_name,
                &mut tool_input,
                &mut tool_output,
            );
        }

        (
            content_parts.join("\n\n"),
            tool_name,
            tool_input,
            tool_output,
        )
    }

    fn content_from_db_parts(
        conn: &Connection,
        message_id: &str,
    ) -> (String, Option<String>, Option<String>, Option<String>) {
        let mut content_parts = Vec::new();
        let mut tool_name = None;
        let mut tool_input = None;
        let mut tool_output = None;

        let Ok(mut stmt) = conn.prepare(
            "SELECT data FROM part WHERE message_id = ?1 ORDER BY time_created ASC, id ASC",
        ) else {
            return (String::new(), None, None, None);
        };

        let Ok(rows) = stmt.query_map([message_id], |row| row.get::<_, String>(0)) else {
            return (String::new(), None, None, None);
        };

        for data in rows.flatten() {
            if let Ok(part) = serde_json::from_str::<Value>(&data) {
                Self::merge_part_json(
                    &part,
                    &mut content_parts,
                    &mut tool_name,
                    &mut tool_input,
                    &mut tool_output,
                );
            }
        }

        (
            content_parts.join("\n\n"),
            tool_name,
            tool_input,
            tool_output,
        )
    }

    async fn parse_storage_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let session_json = Self::read_json(path)
            .ok_or_else(|| format!("Failed to read OpenCode session JSON: {}", path.display()))?;

        let session_id = session_json
            .get("id")
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
        let data_dir = Self::data_dir_from_storage_path(path)
            .ok_or_else(|| format!("Invalid OpenCode storage path: {}", path.display()))?;
        let message_dir = data_dir.join("storage/message").join(&session_id);

        let mut messages = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        for (seq, (_, message_json)) in Self::read_json_files_sorted(&message_dir)
            .into_iter()
            .enumerate()
        {
            let role_str = message_json
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let Some(role) = Self::role_from_str(role_str) else {
                continue;
            };
            let message_id = message_json
                .get("id")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let timestamp =
                Self::timestamp_from_millis(Self::json_i64_at(&message_json, &["time", "created"]));
            let (content, tool_name, tool_input, tool_output) =
                Self::content_from_parts(&data_dir.join("storage/part").join(&message_id));

            if let Some(ref name) = tool_name {
                if let Some(input_str) = tool_input.as_deref() {
                    if let Ok(parsed) = serde_json::from_str::<Value>(input_str) {
                        if let Some(path) = opencode_extract_file_path(&parsed) {
                            file_touches.push(FileTouch {
                                path,
                                operation: opencode_file_operation_for(name).to_string(),
                                sequence: seq as u32,
                            });
                        }
                    }
                }
            }

            messages.push(Message {
                id: message_id,
                session_id: session_id.clone(),
                role,
                content,
                timestamp,
                sequence: seq as u32,
                tool_name,
                tool_input,
                tool_output,
            });
        }

        let created_at =
            Self::timestamp_from_millis(Self::json_i64_at(&session_json, &["time", "created"]))
                .unwrap_or_else(Utc::now);
        let updated_at =
            Self::timestamp_from_millis(Self::json_i64_at(&session_json, &["time", "updated"]))
                .unwrap_or(created_at);
        let title = session_json
            .get("title")
            .and_then(|value| value.as_str())
            .filter(|title| !title.is_empty())
            .map(ToString::to_string)
            .or_else(|| {
                messages
                    .iter()
                    .find(|message| {
                        message.role == MessageRole::User && !message.content.is_empty()
                    })
                    .map(|message| message.content.chars().take(100).collect())
            })
            .unwrap_or_else(|| "Untitled".to_string());

        let session = Session {
            id: session_id,
            parent_session_id: None,
            agent: AgentType::OpenCode,
            title,
            project_path: session_json
                .get("directory")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string(),
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

    async fn parse_database_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let data_dir = Self::data_dir_from_storage_path(path)
            .ok_or_else(|| format!("Invalid OpenCode database marker path: {}", path.display()))?;
        let db_path = data_dir.join("opencode.db");
        let session_id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| {
            format!(
                "Failed to open OpenCode database {}: {}",
                db_path.display(),
                e
            )
        })?;

        let (
            parent_session_id,
            title,
            project_path,
            created_millis,
            updated_millis,
            db_model,
            tokens_input,
            tokens_output,
            tokens_reasoning,
            tokens_cache_read,
            tokens_cache_write,
        ): (Option<String>, String, String, i64, i64, Option<String>, i64, i64, i64, i64, i64) = conn
            .query_row(
                "SELECT parent_id, title, directory, time_created, time_updated, model, tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write FROM session WHERE id = ?1",
                [&session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?)),
            )
            .or_else(|_| {
                conn.query_row(
                    "SELECT parent_id, title, directory, time_created, time_updated, NULL as model, 0 as tokens_input, 0 as tokens_output, 0 as tokens_reasoning, 0 as tokens_cache_read, 0 as tokens_cache_write FROM session WHERE id = ?1",
                    [&session_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?, row.get(10)?)),
                )
            })
            .map_err(|e| format!("Failed to load OpenCode session {}: {}", session_id, e))?;

        let model_parsed = db_model.as_deref().and_then(|m| {
            if m.is_empty() {
                return None;
            }
            serde_json::from_str::<Value>(m)
                .ok()
                .and_then(|v| {
                    v.get("id")
                        .and_then(|id| id.as_str())
                        .map(ToString::to_string)
                })
                .or_else(|| Some(m.to_string()))
        });

        let mut stmt = conn
            .prepare(
                "SELECT id, time_created, data FROM message
                 WHERE session_id = ?1 ORDER BY time_created ASC, id ASC",
            )
            .map_err(|e| format!("Failed to prepare OpenCode message query: {}", e))?;
        let rows = stmt
            .query_map([&session_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| format!("Failed to query OpenCode messages: {}", e))?;

        let mut messages = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        for row in rows {
            let (message_id, time_created, data) =
                row.map_err(|e| format!("Failed to read OpenCode message row: {}", e))?;
            let json: Value = match serde_json::from_str(&data) {
                Ok(value) => value,
                Err(_) => continue,
            };
            let role_str = json
                .get("role")
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            let Some(role) = Self::role_from_str(role_str) else {
                continue;
            };
            let (content, tool_name, tool_input, tool_output) =
                Self::content_from_db_parts(&conn, &message_id);

            if let Some(ref name) = tool_name {
                if let Some(input_str) = tool_input.as_deref() {
                    if let Ok(parsed) = serde_json::from_str::<Value>(input_str) {
                        if let Some(path) = opencode_extract_file_path(&parsed) {
                            file_touches.push(FileTouch {
                                path,
                                operation: opencode_file_operation_for(name).to_string(),
                                sequence: messages.len() as u32,
                            });
                        }
                    }
                }
            }

            messages.push(Message {
                id: message_id,
                session_id: session_id.clone(),
                role,
                content,
                timestamp: Self::timestamp_from_millis(Some(time_created)),
                sequence: messages.len() as u32,
                tool_name,
                tool_input,
                tool_output,
            });
        }

        let session = Session {
            id: session_id,
            parent_session_id,
            agent: AgentType::OpenCode,
            title: if title.is_empty() {
                messages
                    .iter()
                    .find(|message| {
                        message.role == MessageRole::User && !message.content.is_empty()
                    })
                    .map(|message| message.content.chars().take(100).collect())
                    .unwrap_or_else(|| "Untitled".to_string())
            } else {
                title
            },
            project_path,
            created_at: Self::timestamp_from_millis(Some(created_millis)).unwrap_or_else(Utc::now),
            updated_at: Self::timestamp_from_millis(Some(updated_millis)).unwrap_or_else(Utc::now),
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model: model_parsed,
            input_tokens: tokens_input as u64,
            output_tokens: tokens_output as u64,
            reasoning_tokens: tokens_reasoning as u64,
            cached_tokens: (tokens_cache_read + tokens_cache_write) as u64,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }

    async fn parse_legacy_jsonl_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;

        let file_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut messages = Vec::new();
        let mut title = String::from("Untitled");
        let mut seq: u32 = 0;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut file_touches: Vec<FileTouch> = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let role_str = json.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let msg_content = json
                .get("content")
                .and_then(|c| {
                    if c.is_string() {
                        c.as_str().map(|s| s.to_string())
                    } else {
                        Some(c.to_string())
                    }
                })
                .unwrap_or_default();

            let Some(role) = Self::role_from_str(role_str) else {
                continue;
            };

            if seq == 0 && role == MessageRole::User && !msg_content.is_empty() {
                title = msg_content.chars().take(100).collect();
            }

            let timestamp = json
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if seq == 0 {
                created_at = timestamp.unwrap_or_else(Utc::now);
            }
            updated_at = timestamp.unwrap_or_else(Utc::now);

            let tool_name_str = json
                .get("tool_name")
                .and_then(|t| t.as_str())
                .map(ToString::to_string);
            let tool_input_value = json.get("tool_input").cloned();

            if let Some(ref name) = tool_name_str {
                if let Some(ref input) = tool_input_value {
                    if let Some(path) = opencode_extract_file_path(input) {
                        file_touches.push(FileTouch {
                            path,
                            operation: opencode_file_operation_for(name).to_string(),
                            sequence: seq,
                        });
                    }
                }
            }

            messages.push(Message {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: file_name.clone(),
                role,
                content: msg_content,
                timestamp,
                sequence: seq,
                tool_name: tool_name_str,
                tool_input: tool_input_value.map(|v| v.to_string()),
                tool_output: json.get("tool_output").map(|o| o.to_string()),
            });
            seq += 1;
        }

        let session = Session {
            id: file_name,
            parent_session_id: None,
            agent: AgentType::OpenCode,
            title,
            project_path: String::new(),
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
        !Self::existing_data_dirs().is_empty()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        Self::scan_data_dirs(Self::existing_data_dirs())
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        if Self::is_session_diff_marker(path) {
            self.parse_database_session(path).await
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            self.parse_storage_session(path).await
        } else {
            self.parse_legacy_jsonl_session(path).await
        }
    }

    fn resume_command(&self, session_id: &str, project_path: &str) -> String {
        if cfg!(target_os = "windows") {
            return Self::windows_resume_command(session_id, project_path);
        }

        let safe_path = crate::shell_quote::shell_quote(project_path);
        format!("cd {} && opencode --session {}", safe_path, session_id)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;

    #[test]
    fn candidate_data_dirs_from_sources_deduplicates_xdg_paths() {
        let home = std::path::PathBuf::from("/home/orbit-user");
        let data_local = std::path::PathBuf::from("/home/orbit-user/.local/share");
        let data = std::path::PathBuf::from("/home/orbit-user/.local/share");
        let config = std::path::PathBuf::from("/home/orbit-user/.config");

        let dirs = OpenCodeAdapter::candidate_data_dirs_from_sources(
            Some(home.clone()),
            Some(data_local),
            Some(data),
            Some(config.clone()),
        );

        assert_eq!(
            dirs,
            vec![home.join(".local/share/opencode"), config.join("opencode"),]
        );
    }

    #[test]
    fn existing_data_dirs_from_candidates_filters_known_stores_and_deduplicates() {
        let temp = tempfile::tempdir().unwrap();
        let root_a = temp.path().join("data-a/opencode");
        let root_b = temp.path().join("data-b/opencode");
        let root_empty = temp.path().join("empty/opencode");

        std::fs::create_dir_all(root_a.join("storage/session/project_123")).unwrap();
        std::fs::create_dir_all(root_b.join("sessions")).unwrap();
        std::fs::create_dir_all(&root_empty).unwrap();

        let dirs = OpenCodeAdapter::existing_data_dirs_from_candidates(vec![
            root_a.clone(),
            root_empty,
            root_b.clone(),
            root_a.clone(),
        ]);

        assert_eq!(dirs, vec![root_a, root_b]);
    }

    #[test]
    fn scan_data_dirs_returns_sessions_from_multiple_roots_without_duplicate_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let root_a = temp.path().join("data-a/opencode");
        let root_b = temp.path().join("data-b/opencode");

        std::fs::create_dir_all(root_a.join("storage/session/project_123")).unwrap();
        std::fs::create_dir_all(root_a.join("storage/session_diff")).unwrap();
        std::fs::create_dir_all(root_b.join("sessions/project_456")).unwrap();

        let storage_path = root_a.join("storage/session/project_123/ses_storage.json");

        let legacy_path = root_b.join("sessions/project_456/ses_legacy.jsonl");
        std::fs::write(&storage_path, "{}").unwrap();

        std::fs::write(&legacy_path, "{}").unwrap();

        let locations =
            OpenCodeAdapter::scan_data_dirs(vec![root_a.clone(), root_b.clone(), root_a]);
        let mut paths: Vec<_> = locations.into_iter().map(|loc| loc.path).collect();
        paths.sort();

        let mut expected = vec![storage_path, legacy_path];
        expected.sort();
        assert_eq!(paths, expected);
    }

    #[tokio::test]
    async fn parses_current_opencode_storage_session() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("opencode");
        let session_dir = data_dir.join("storage/session/project_123");
        let message_dir = data_dir.join("storage/message/ses_123");
        let user_part_dir = data_dir.join("storage/part/msg_user");
        let assistant_part_dir = data_dir.join("storage/part/msg_assistant");

        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::create_dir_all(&message_dir).unwrap();
        std::fs::create_dir_all(&user_part_dir).unwrap();
        std::fs::create_dir_all(&assistant_part_dir).unwrap();

        std::fs::write(
            session_dir.join("ses_123.json"),
            r#"{
  "id": "ses_123",
  "title": "Fix failing import",
  "directory": "/tmp/project",
  "time": { "created": 1700000000000, "updated": 1700000060000 }
}"#,
        )
        .unwrap();
        std::fs::write(
            message_dir.join("msg_assistant.json"),
            r#"{
  "id": "msg_assistant",
  "sessionID": "ses_123",
  "role": "assistant",
  "time": { "created": 1700000020000 }
}"#,
        )
        .unwrap();
        std::fs::write(
            message_dir.join("msg_user.json"),
            r#"{
  "id": "msg_user",
  "sessionID": "ses_123",
  "role": "user",
  "time": { "created": 1700000010000 }
}"#,
        )
        .unwrap();
        std::fs::write(
            user_part_dir.join("prt_text.json"),
            r#"{
  "id": "prt_user",
  "messageID": "msg_user",
  "type": "text",
  "text": "Can you fix the import?"
}"#,
        )
        .unwrap();
        std::fs::write(
            assistant_part_dir.join("prt_text.json"),
            r#"{
  "id": "prt_assistant",
  "messageID": "msg_assistant",
  "type": "text",
  "text": "I fixed the import."
}"#,
        )
        .unwrap();

        let adapter = OpenCodeAdapter::new();
        let parsed = adapter
            .parse_session(&session_dir.join("ses_123.json"))
            .await
            .unwrap();

        assert_eq!(parsed.session.id, "ses_123");
        assert_eq!(parsed.session.parent_session_id, None);
        assert_eq!(parsed.session.title, "Fix failing import");
        assert_eq!(parsed.session.project_path, "/tmp/project");
        assert_eq!(parsed.session.message_count, 2);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Can you fix the import?");
        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert_eq!(parsed.messages[1].content, "I fixed the import.");
    }

    #[tokio::test]
    async fn parses_database_backed_session_from_session_diff_marker() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("opencode");
        let session_diff_dir = data_dir.join("storage/session_diff");
        std::fs::create_dir_all(&session_diff_dir).unwrap();
        std::fs::write(session_diff_dir.join("ses_db.json"), "[]").unwrap();

        let conn = rusqlite::Connection::open(data_dir.join("opencode.db")).unwrap();
        conn.execute_batch(
            r#"
CREATE TABLE session (
    id text PRIMARY KEY,
    project_id text NOT NULL,
    parent_id text,
    slug text NOT NULL,
    directory text NOT NULL,
    title text NOT NULL,
    version text NOT NULL,
    time_created integer NOT NULL,
    time_updated integer NOT NULL
);
CREATE TABLE message (
    id text PRIMARY KEY,
    session_id text NOT NULL,
    time_created integer NOT NULL,
    time_updated integer NOT NULL,
    data text NOT NULL
);
CREATE TABLE part (
    id text PRIMARY KEY,
    message_id text NOT NULL,
    session_id text NOT NULL,
    time_created integer NOT NULL,
    time_updated integer NOT NULL,
    data text NOT NULL
);
"#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version, time_created, time_updated)
             VALUES (?1, 'global', NULL, 'parent-session', '/tmp/db-project', 'Parent session', '1.15.13', 1699990000000, 1699990060000)",
            ["ses_parent"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (id, project_id, parent_id, slug, directory, title, version, time_created, time_updated)
             VALUES (?1, 'global', 'ses_parent', 'db-session', '/tmp/db-project', 'Database session', '1.15.13', 1700000000000, 1700000060000)",
            ["ses_db"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, 'ses_db', 1700000010000, 1700000010000, ?2)",
            [
                "msg_user",
                r#"{"role":"user","time":{"created":1700000010000}}"#,
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, 'msg_user', 'ses_db', 1700000010001, 1700000010001, ?2)",
            [
                "prt_user",
                r#"{"type":"text","text":"Read me from sqlite"}"#,
            ],
        )
        .unwrap();

        let adapter = OpenCodeAdapter::new();
        let parsed = adapter
            .parse_session(&session_diff_dir.join("ses_db.json"))
            .await
            .unwrap();

        assert_eq!(parsed.session.id, "ses_db");
        assert_eq!(
            parsed.session.parent_session_id.as_deref(),
            Some("ses_parent")
        );
        assert_eq!(parsed.session.title, "Database session");
        assert_eq!(parsed.session.project_path, "/tmp/db-project");
        assert_eq!(parsed.session.message_count, 1);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Read me from sqlite");
    }

    #[tokio::test]
    async fn extract_file_touches_from_legacy_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let sessions_dir = temp.path().join("opencode/sessions");
        std::fs::create_dir_all(&sessions_dir).unwrap();
        let path = sessions_dir.join("ses_legacy.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"role\":\"user\",\"content\":\"fix it\"}\n",
                "{\"role\":\"assistant\",\"content\":\"\",\"tool_name\":\"read\",\"tool_input\":{\"filePath\":\"/src/foo.rs\"}}\n",
                "{\"role\":\"assistant\",\"content\":\"\",\"tool_name\":\"edit\",\"tool_input\":{\"filePath\":\"/src/bar.rs\"}}\n",
            ),
        )
        .unwrap();

        let adapter = OpenCodeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        let paths: Vec<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert!(paths.contains(&"/src/foo.rs"), "got {:?}", paths);
        assert!(paths.contains(&"/src/bar.rs"), "got {:?}", paths);
        assert_eq!(parsed.file_touches.len(), 2);
    }

    #[tokio::test]
    async fn extract_file_touches_from_storage_session_parts() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path().join("opencode");
        let session_dir = data_dir.join("storage/session/project_123");
        let message_dir = data_dir.join("storage/message/ses_storage");
        let part_dir = data_dir.join("storage/part/msg_tool");

        std::fs::create_dir_all(&session_dir).unwrap();
        std::fs::create_dir_all(&message_dir).unwrap();
        std::fs::create_dir_all(&part_dir).unwrap();

        std::fs::write(
            session_dir.join("ses_storage.json"),
            r#"{
  "id": "ses_storage",
  "title": "Storage test",
  "directory": "/tmp/project",
  "time": { "created": 1700000000000, "updated": 1700000060000 }
}"#,
        )
        .unwrap();
        std::fs::write(
            message_dir.join("msg_tool.json"),
            r#"{
  "id": "msg_tool",
  "sessionID": "ses_storage",
  "role": "assistant",
  "time": { "created": 1700000010000 }
}"#,
        )
        .unwrap();
        std::fs::write(
            part_dir.join("prt_tool.json"),
            r#"{
  "id": "prt_tool",
  "messageID": "msg_tool",
  "type": "tool",
  "tool": "edit",
  "state": { "input": { "filePath": "/src/baz.rs" } }
}"#,
        )
        .unwrap();

        let adapter = OpenCodeAdapter::new();
        let parsed = adapter
            .parse_session(&session_dir.join("ses_storage.json"))
            .await
            .unwrap();

        let paths: Vec<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert_eq!(paths, vec!["/src/baz.rs"]);
    }
}
