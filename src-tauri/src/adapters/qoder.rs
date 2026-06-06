use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::{params, OpenFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::{AgentType, Message, MessageRole, NormalizedSession, Session};

const QODER_DB_PATH: &str = "Library/Application Support/Qoder/SharedClientCache/cache/db/local.db";

/// Truncates `s` to at most `max_bytes` bytes, respecting UTF-8 char boundaries.
/// Appends "..." if truncation occurred.
fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find the last char boundary at or before max_bytes
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

pub struct QoderAdapter {
    cache: Mutex<Option<QoderDbSnapshot>>,
}

struct QoderDbSnapshot {
    sessions: Vec<QoderSessionRow>,
    messages: HashMap<String, Vec<QoderMessageRow>>,
}

#[derive(Clone)]
struct QoderSessionRow {
    session_id: String,
    session_title: String,
    project_uri: String,
    gmt_create: i64,
    gmt_modified: i64,
    status: String,
}

#[derive(Clone)]
struct QoderMessageRow {
    _id: String,
    role: String,
    tool_result: Option<String>,
    gmt_create: i64,
}

impl QoderAdapter {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }

    pub(crate) fn windows_candidate_db_paths(paths: &PlatformPaths) -> Vec<PathBuf> {
        [
            paths.data_join("Qoder/SharedClientCache/cache/db/local.db"),
            paths.data_local_join("Qoder/SharedClientCache/cache/db/local.db"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub(crate) fn windows_db_path(paths: &PlatformPaths) -> Option<PathBuf> {
        Self::windows_candidate_db_paths(paths)
            .into_iter()
            .find(|path| path.is_file())
    }

    pub(crate) fn windows_resume_command() -> &'static str {
        "Start-Process Qoder"
    }

    fn db_path() -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            let home = dirs::home_dir()?;
            let path = home.join(QODER_DB_PATH);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        } else if cfg!(target_os = "linux") {
            // To be implemented.
            None
        } else if cfg!(target_os = "windows") {
            Self::windows_db_path(&PlatformPaths::system())
        } else {
            None
        }
    }

    fn load_snapshot(&self) -> Result<QoderDbSnapshot, String> {
        let db_path = Self::db_path().ok_or_else(|| "Qoder DB not found".to_string())?;

        let conn = rusqlite::Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("Failed to open Qoder DB: {}", e))?;

        // Load quest sessions
        let mut sess_stmt = conn
            .prepare(
                "SELECT session_id, session_title, project_uri, gmt_create, gmt_modified, status
                 FROM chat_session
                 WHERE session_type = 'quest'
                 ORDER BY gmt_modified DESC",
            )
            .map_err(|e| format!("Failed to prepare session query: {}", e))?;

        let sessions: Vec<QoderSessionRow> = sess_stmt
            .query_map([], |row| {
                Ok(QoderSessionRow {
                    session_id: row.get(0)?,
                    session_title: row.get(1)?,
                    project_uri: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    gmt_create: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    gmt_modified: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                    status: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                })
            })
            .map_err(|e| format!("Failed to query sessions: {}", e))?
            .filter_map(|r| r.ok())
            .collect();

        // Load messages for all sessions
        let session_ids: Vec<String> = sessions.iter().map(|s| s.session_id.clone()).collect();
        let mut messages: HashMap<String, Vec<QoderMessageRow>> = HashMap::new();

        if !session_ids.is_empty() {
            let mut msg_stmt = conn
                .prepare(
                    "SELECT id, session_id, role, tool_result, gmt_create
                     FROM chat_message
                     WHERE session_id = ?1
                     ORDER BY gmt_create ASC",
                )
                .map_err(|e| format!("Failed to prepare message query: {}", e))?;

            for sid in &session_ids {
                let rows: Vec<QoderMessageRow> = msg_stmt
                    .query_map(params![sid], |row| {
                        Ok(QoderMessageRow {
                            _id: row.get(0)?,
                            role: row.get::<_, String>(2)?,
                            tool_result: row.get::<_, Option<String>>(3)?,
                            gmt_create: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        })
                    })
                    .map_err(|e| format!("Failed to query messages for {}: {}", sid, e))?
                    .filter_map(|r| r.ok())
                    .collect();
                messages.insert(sid.clone(), rows);
            }
        }

        Ok(QoderDbSnapshot { sessions, messages })
    }

    fn ensure_cache(&self) -> Result<(), String> {
        let snapshot = self.load_snapshot()?;
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        *cache = Some(snapshot);
        Ok(())
    }

    fn project_path_from_uri(uri: &str) -> String {
        // project_uri is a file path like "/Users/maf/My Files/My apps/project"
        if uri.is_empty() {
            return String::new();
        }
        // Try to extract the last directory component as the project name
        Path::new(uri)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| uri.to_string())
    }

    fn ts_to_datetime(ts_millis: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(ts_millis / 1000, ((ts_millis % 1000) * 1_000_000) as u32)
            .unwrap_or_else(Utc::now)
    }

    fn infer_tool_name(params: &serde_json::Value) -> String {
        let obj = match params.as_object() {
            Some(o) => o,
            None => return "unknown_tool".to_string(),
        };

        if obj.contains_key("command") {
            return "Bash".to_string();
        }
        if obj.contains_key("file_path") {
            if obj.contains_key("original_text") || obj.contains_key("new_text") {
                return "SearchReplace".to_string();
            }
            if obj.contains_key("file_content") {
                return "Write".to_string();
            }
            return "Read".to_string();
        }
        if obj.contains_key("regex") || obj.contains_key("pattern") {
            return "Grep".to_string();
        }
        if obj.contains_key("query") {
            return "SearchCodebase".to_string();
        }
        if obj.contains_key("path") && obj.contains_key("query") {
            return "Glob".to_string();
        }
        if obj.contains_key("file_path") && obj.contains_key("start_line") {
            return "Read".to_string();
        }

        "tool".to_string()
    }

    fn extract_tool_input(params: &serde_json::Value) -> Option<String> {
        if params.is_null() {
            return None;
        }
        let s = serde_json::to_string_pretty(params).ok()?;
        Some(truncate_utf8(&s, 2000))
    }

    fn extract_tool_output(results: &serde_json::Value) -> Option<String> {
        if results.is_null() {
            return None;
        }
        // Try to extract meaningful content from results array
        if let Some(arr) = results.as_array() {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(content) = item.get("content").and_then(|c| c.as_str()) {
                    if !content.is_empty() {
                        parts.push(truncate_utf8(content, 2000));
                    }
                }
            }
            if !parts.is_empty() {
                return Some(parts.join("\n---\n"));
            }
        }
        // Fallback: serialize the whole thing
        let s = serde_json::to_string(results).ok()?;
        if s.len() > 2 && s != "[]" && s != "{}" && s != "null" {
            Some(truncate_utf8(&s, 2000))
        } else {
            None
        }
    }
}

#[async_trait]
impl AgentAdapter for QoderAdapter {
    fn id(&self) -> &str {
        "qoder"
    }

    fn name(&self) -> &str {
        "Qoder"
    }

    async fn detect(&self) -> bool {
        Self::db_path().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        if let Err(e) = self.ensure_cache() {
            tracing::warn!("Failed to load Qoder DB: {}", e);
            return Vec::new();
        }

        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = match cache.as_ref() {
            Some(s) => s,
            None => return Vec::new(),
        };

        snapshot
            .sessions
            .iter()
            .map(|row| SessionLocation {
                path: PathBuf::from(format!("qoder://session/{}", row.session_id)),
                last_modified: Self::ts_to_datetime(row.gmt_modified),
            })
            .collect()
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let path_str = path.to_string_lossy();
        let session_id = path_str
            .strip_prefix("qoder://session/")
            .ok_or_else(|| "Invalid qoder session path".to_string())?
            .to_string();

        let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = cache
            .as_ref()
            .ok_or_else(|| "Qoder DB not loaded".to_string())?;

        let session_row = snapshot
            .sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;

        let msg_rows = snapshot
            .messages
            .get(&session_id)
            .cloned()
            .unwrap_or_default();

        let project_path = Self::project_path_from_uri(&session_row.project_uri);
        let mut title = session_row.session_title.clone();
        let created_at = Self::ts_to_datetime(session_row.gmt_create);
        let updated_at = Self::ts_to_datetime(session_row.gmt_modified);

        let mut messages = Vec::new();
        let mut seq: u32 = 0;
        let mut first_user_msg = true;

        // session_title is the user's original query (stored in plaintext)
        let user_query = if !title.is_empty() {
            Some(title.clone())
        } else {
            None
        };

        for msg in &msg_rows {
            let ts = Self::ts_to_datetime(msg.gmt_create);

            match msg.role.as_str() {
                "user" => {
                    // Emit only the first user message using the readable session_title;
                    // subsequent user messages are encrypted and add no value as placeholders.
                    if first_user_msg {
                        if let Some(ref query) = user_query {
                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::User,
                                content: query.clone(),
                                timestamp: Some(ts),
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                            });
                            seq += 1;
                        }
                        first_user_msg = false;
                    }
                }
                "assistant" => {
                    // Assistant content is encrypted — skip to avoid noisy placeholders.
                    // Tool calls that follow carry the useful information.
                }
                "tool" => {
                    if let Some(ref tool_result_json) = msg.tool_result {
                        if let Ok(tr) = serde_json::from_str::<serde_json::Value>(tool_result_json)
                        {
                            let params = tr
                                .get("parameters")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let results = tr
                                .get("results")
                                .cloned()
                                .unwrap_or(serde_json::Value::Null);
                            let tool_name = Self::infer_tool_name(&params);
                            let tool_input = Self::extract_tool_input(&params);
                            let tool_output = Self::extract_tool_output(&results);

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: Some(ts),
                                sequence: seq,
                                tool_name: Some(tool_name),
                                tool_input,
                                tool_output,
                            });
                            seq += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        if title.is_empty() {
            title = format!(
                "Qoder Session ({})",
                &session_id.chars().take(12).collect::<String>()
            );
        }

        let session = Session {
            id: session_id,
            parent_session_id: None,
            agent: AgentType::Qoder,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: session_row.status == "Running",
            message_count: messages.len() as u32,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches: vec![],
        })
    }

    fn resume_command(&self, _session_id: &str, _project_path: &str) -> String {
        if cfg!(target_os = "windows") {
            Self::windows_resume_command().to_string()
        } else {
            "open -a Qoder".to_string()
        }
    }

    async fn is_active(&self, session_path: &Path) -> bool {
        let path_str = session_path.to_string_lossy();
        if let Some(session_id) = path_str.strip_prefix("qoder://session/") {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(snapshot) = cache.as_ref() {
                return snapshot
                    .sessions
                    .iter()
                    .any(|s| s.session_id == session_id && s.status == "Running");
            }
        }
        false
    }
}
