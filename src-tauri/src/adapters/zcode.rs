use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::OpenFlags;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

/// Adapter for ZCode (https://zcode.z.ai).
///
/// ZCode's authoritative session store is a SQLite database at
/// `~/.zcode/cli/db/db.sqlite` (root overridable via `ZCODE_STORAGE_DIR`). The `session`
/// table is the index (id, title, directory/cwd, parent_id, time_created/updated,
/// task_type); `message` holds per-turn metadata (role, model, tokens, contextSnapshot with
/// gitBranch); `part` holds the typed content blocks (text / reasoning / tool / file /
/// step-*). The companion `~/.zcode/cli/rollout/*.jsonl` files are raw model-I/O logs that
/// ZCode garbage-collects, so they can't be relied on as a complete source — the DB is.
///
/// Modeled on the Warp adapter: sessions are addressed via synthetic
/// `zcode://session/<id>` paths, the DB is opened read-only, and the indexer's
/// `compute_hash`/`mark_stale_sessions` already handle non-filesystem paths.
pub struct ZCodeAdapter;

impl ZCodeAdapter {
    pub fn new() -> Self {
        Self
    }

    /// `<ZCODE_STORAGE_DIR>` or `~/.zcode` — the same resolution ZCode itself uses.
    fn storage_root() -> Option<PathBuf> {
        if let Ok(custom) = std::env::var("ZCODE_STORAGE_DIR") {
            let trimmed = custom.trim().to_string();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
        dirs::home_dir().map(|h| h.join(".zcode"))
    }

    fn db_path_from_root(root: &Path) -> PathBuf {
        root.join("cli").join("db").join("db.sqlite")
    }

    /// Test-only helper asserting the macOS/Linux default lives under `~/.zcode`.
    #[cfg(test)]
    fn db_path_from_home(home: &Path) -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(Self::db_path_from_root(&home.join(".zcode")))
        } else {
            None
        }
    }

    pub(crate) fn windows_db_path(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".zcode").map(|p| Self::db_path_from_root(&p))
    }

    fn db_path() -> Option<PathBuf> {
        // Honor an explicit ZCODE_STORAGE_DIR override on every platform; otherwise fall
        // back to the platform default (Windows resolves via USERPROFILE, matching ZCode).
        let candidate = if cfg!(target_os = "windows") {
            match Self::storage_root() {
                Some(root) => Self::db_path_from_root(&root),
                None => Self::windows_db_path(&PlatformPaths::system())?,
            }
        } else {
            match Self::storage_root() {
                Some(root) => Self::db_path_from_root(&root),
                None => return None,
            }
        };
        if candidate.is_file() {
            Some(candidate)
        } else {
            None
        }
    }

    fn open_db(path: &Path) -> Result<rusqlite::Connection, String> {
        rusqlite::Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| format!("Failed to open ZCode DB: {}", e))
    }
}

/// Map a ZCode tool name to a file-operation classification for `FileTouch`.
fn file_operation_for(tool_name: &str) -> String {
    match tool_name {
        "Read" => "read".to_string(),
        "Write" => "write".to_string(),
        "Edit" => "edit".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Pull a `file_path` / `path` out of a tool-call's structured `input` object.
fn extract_file_path(input: &serde_json::Value) -> Option<String> {
    for key in &["file_path", "path"] {
        if let Some(p) = input.get(*key).and_then(|v| v.as_str()) {
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    None
}

/// Epoch-milliseconds (ZCode's `time_created`/`time_updated`) → UTC DateTime.
fn ms_to_datetime(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(ms / 1000, ((ms % 1000) * 1_000_000) as u32)
        .single()
        .unwrap_or_else(Utc::now)
}

#[async_trait]
impl AgentAdapter for ZCodeAdapter {
    fn id(&self) -> &str {
        "zcode"
    }

    fn name(&self) -> &str {
        "ZCode"
    }

    async fn detect(&self) -> bool {
        Self::db_path().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let db_path = match Self::db_path() {
            Some(p) => p,
            None => return Vec::new(),
        };
        let conn = match Self::open_db(&db_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            "SELECT id, time_updated FROM session WHERE task_type = 'interactive' \
             ORDER BY time_updated DESC",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let time_updated: i64 = row.get(1)?;
            Ok((id, time_updated))
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        rows.flatten()
            .map(|(id, time_updated)| SessionLocation {
                path: PathBuf::from(format!("zcode://session/{}", id)),
                last_modified: ms_to_datetime(time_updated),
            })
            .collect()
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let session_id = path
            .to_string_lossy()
            .strip_prefix("zcode://session/")
            .ok_or_else(|| "Invalid zcode session path".to_string())?
            .to_string();

        let db_path = Self::db_path().ok_or_else(|| "ZCode DB not found".to_string())?;
        let conn = Self::open_db(&db_path)?;

        // --- session row: title, cwd, parent, timestamps ---
        let (title, project_path, parent_session_id, time_created, time_updated): (
            String,
            String,
            Option<String>,
            i64,
            i64,
        ) = conn
            .prepare(
                "SELECT title, directory, parent_id, time_created, time_updated \
                 FROM session WHERE id = ?1",
            )
            .map_err(|e| e.to_string())?
            .query_row([&session_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(|e| format!("Session {} not found: {}", session_id, e))?;

        // --- messages (ordered) ---
        let mut messages_query = conn
            .prepare("SELECT id, data FROM message WHERE session_id = ?1 ORDER BY time_created, id")
            .map_err(|e| e.to_string())?;
        let message_rows: Vec<(String, serde_json::Value)> = messages_query
            .query_map([&session_id], |row| {
                let id: String = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((id, serde_json::from_str(&data).unwrap_or(serde_json::Value::Null)))
            })
            .map_err(|e| e.to_string())?
            .flatten()
            .collect();
        drop(messages_query);

        // --- parts (ordered), grouped by message_id ---
        let mut parts_query = conn
            .prepare("SELECT message_id, data FROM part WHERE session_id = ?1 ORDER BY time_created, id")
            .map_err(|e| e.to_string())?;
        let mut parts_by_message: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
        let part_rows = parts_query
            .query_map([&session_id], |row| {
                let message_id: String = row.get(0)?;
                let data: String = row.get(1)?;
                Ok((message_id, serde_json::from_str(&data).unwrap_or(serde_json::Value::Null)))
            })
            .map_err(|e| e.to_string())?;
        for (message_id, data) in part_rows.flatten() {
            parts_by_message.entry(message_id).or_default().push(data);
        }
        drop(parts_query);

        // --- emit Orbit messages ---
        let mut messages: Vec<Message> = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut model: Option<String> = None;
        let mut git_branch: Option<String> = None;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cached_tokens: u64 = 0;
        let mut reasoning_tokens: u64 = 0;
        let mut seq: u32 = 0;

        for (message_id, data) in &message_rows {
            let role = data.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let parts = parts_by_message.get(message_id);

            // First assistant message wins for the model; any message's contextSnapshot
            // (user messages carry it) can supply the git branch.
            if role == "assistant" && model.is_none() {
                if let Some(m) = data.get("modelID").and_then(|m| m.as_str()) {
                    if !m.is_empty() {
                        model = Some(m.to_string());
                    }
                }
            }
            if git_branch.is_none() {
                git_branch = data
                    .get("contextSnapshot")
                    .and_then(|c| c.get("envInfo"))
                    .and_then(|e| e.get("gitBranch"))
                    .and_then(|b| b.as_str())
                    .filter(|s| !s.is_empty())
                    .map(ToString::to_string);
            }
            // Assistant tokens accumulate per-turn.
            if role == "assistant" {
                if let Some(tokens) = data.get("tokens") {
                    input_tokens =
                        input_tokens.saturating_add(tokens.get("input").and_then(|v| v.as_u64()).unwrap_or(0));
                    output_tokens = output_tokens
                        .saturating_add(tokens.get("output").and_then(|v| v.as_u64()).unwrap_or(0));
                    cached_tokens = cached_tokens
                        .saturating_add(tokens.get("cache").and_then(|c| c.get("read")).and_then(|v| v.as_u64()).unwrap_or(0));
                    reasoning_tokens = reasoning_tokens
                        .saturating_add(tokens.get("reasoning").and_then(|v| v.as_u64()).unwrap_or(0));
                }
            }

            match role {
                "user" => {
                    let text = parts
                        .map(|ps| {
                            ps.iter()
                                .filter_map(|p| {
                                    (p.get("type").and_then(|t| t.as_str()) == Some("text"))
                                        .then(|| p.get("text").and_then(|t| t.as_str()).unwrap_or(""))
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    if text.trim().is_empty() {
                        continue;
                    }
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::User,
                        content: text,
                        timestamp: None,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;
                }
                "assistant" => {
                    // Emit visible text first (if any), then each tool call in order.
                    if let Some(ps) = parts {
                        let text: String = ps
                            .iter()
                            .filter_map(|p| {
                                (p.get("type").and_then(|t| t.as_str()) == Some("text"))
                                    .then(|| p.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.trim().is_empty() {
                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Assistant,
                                content: text,
                                timestamp: None,
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                            });
                            seq += 1;
                        }

                        for p in ps {
                            if p.get("type").and_then(|t| t.as_str()) != Some("tool") {
                                continue;
                            }
                            let tool_name = p
                                .get("tool")
                                .and_then(|t| t.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let state = p.get("state").unwrap_or(&serde_json::Value::Null);
                            let input = state.get("input").cloned().unwrap_or(serde_json::Value::Null);
                            let tool_input = serde_json::to_string(&input)
                                .ok()
                                .filter(|s| s != "null" && s != "{}");
                            // Prefer `output`; fall back to `error` (failed tool calls).
                            let output = state
                                .get("output")
                                .and_then(|o| o.as_str())
                                .or_else(|| state.get("error").and_then(|e| e.as_str()))
                                .unwrap_or("")
                                .to_string();

                            if let Some(fp) = extract_file_path(&input) {
                                file_touches.push(FileTouch {
                                    path: fp,
                                    operation: file_operation_for(&tool_name),
                                    sequence: seq,
                                });
                            }

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: None,
                                sequence: seq,
                                tool_name: Some(tool_name),
                                tool_input,
                                tool_output: if output.is_empty() { None } else { Some(output) },
                            });
                            seq += 1;
                        }
                    }
                }
                _ => {} // system + any unknown role skipped
            }
        }

        let session = Session {
            id: session_id,
            parent_session_id,
            agent: AgentType::Zcode,
            title,
            project_path,
            created_at: ms_to_datetime(time_created),
            updated_at: ms_to_datetime(time_updated),
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            git_branch,
            input_tokens,
            output_tokens,
            cached_tokens,
            reasoning_tokens,
            file_count: 0,
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }

    fn supports_resume(&self) -> bool {
        false
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        let safe = crate::shell_quote::shell_quote(session_id);
        format!("zcode --resume {}", safe)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        // Synthetic `zcode://session/<id>` paths have no mtime/lockfile to inspect. A
        // recency heuristic over `session.time_updated` could power this later; for now,
        // opt out.
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;
    use rusqlite::Connection;

    /// Build a temp ZCode DB with the subset of the schema the adapter queries, then reopen
    /// it read-only via the adapter by pointing `ZCODE_STORAGE_DIR` at the temp root.
    struct TempDb {
        root: PathBuf,
        _tmp: tempfile::TempDir,
    }

    impl TempDb {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path().to_path_buf();
            std::fs::create_dir_all(root.join("cli").join("db")).unwrap();
            Self { root, _tmp: tmp }
        }

        fn db_path(&self) -> PathBuf {
            self.root.join("cli").join("db").join("db.sqlite")
        }

        fn seed(&self) {
            let conn = Connection::open(self.db_path()).unwrap();
            conn.execute_batch(
                "CREATE TABLE session (
                    id text primary key, project_id text not null default '', workspace_id text,
                    parent_id text, slug text not null default '', directory text not null default '',
                    path text, title text not null default '', version text not null default '',
                    time_created integer not null default 0, time_updated integer not null default 0,
                    task_type text not null default 'interactive'
                );
                CREATE TABLE message (
                    id text primary key, session_id text not null,
                    time_created integer not null default 0, time_updated integer not null default 0,
                    data text not null
                );
                CREATE TABLE part (
                    id text primary key, message_id text not null, session_id text not null,
                    time_created integer not null default 0, time_updated integer not null default 0,
                    data text not null
                );",
            )
            .unwrap();
        }

        fn insert_session(
            &self,
            id: &str,
            title: &str,
            directory: &str,
            parent_id: Option<&str>,
            task_type: &str,
            time_created: i64,
            time_updated: i64,
        ) {
            let conn = Connection::open(self.db_path()).unwrap();
            conn.execute(
                "INSERT INTO session (id, title, directory, parent_id, task_type, time_created, time_updated) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, title, directory, parent_id, task_type, time_created, time_updated],
            )
            .unwrap();
        }

        fn insert_message(&self, id: &str, session_id: &str, time_created: i64, data: serde_json::Value) {
            let conn = Connection::open(self.db_path()).unwrap();
            conn.execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data) \
                 VALUES (?1, ?2, ?3, ?3, ?4)",
                rusqlite::params![id, session_id, time_created, data.to_string()],
            )
            .unwrap();
        }

        fn insert_part(&self, id: &str, message_id: &str, session_id: &str, time_created: i64, data: serde_json::Value) {
            let conn = Connection::open(self.db_path()).unwrap();
            conn.execute(
                "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) \
                 VALUES (?1, ?2, ?3, ?4, ?4, ?5)",
                rusqlite::params![id, message_id, session_id, time_created, data.to_string()],
            )
            .unwrap();
        }
    }

    fn with_env_storage_root(root: &Path) {
        std::env::set_var("ZCODE_STORAGE_DIR", root);
    }

    fn clear_env_storage_root() {
        std::env::remove_var("ZCODE_STORAGE_DIR");
    }

    #[test]
    fn db_path_from_home_uses_dot_zcode_on_unix() {
        let home = std::path::Path::new("/home/orbit-user");
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert_eq!(
                ZCodeAdapter::db_path_from_home(home),
                Some(home.join(".zcode").join("cli").join("db").join("db.sqlite"))
            );
        } else {
            assert!(ZCodeAdapter::db_path_from_home(home).is_none());
        }
    }

    #[test]
    fn resume_command_uses_zcode_resume_flag() {
        assert_eq!(
            ZCodeAdapter::new().resume_command("sess_abc-123", ""),
            "zcode --resume 'sess_abc-123'"
        );
    }

    #[test]
    fn does_not_support_resume() {
        assert!(!ZCodeAdapter::new().supports_resume());
    }

    #[tokio::test]
    async fn scan_returns_interactive_sessions_only_as_synthetic_paths() {
        let db = TempDb::new();
        db.seed();
        db.insert_session("sess_interactive_1", "First", "/p/a", None, "interactive", 1000, 5000);
        db.insert_session("sess_interactive_2", "Second", "/p/b", None, "interactive", 1000, 6000);
        db.insert_session("sess_subagent_1", "Child", "/p/a", Some("sess_interactive_1"), "subagent_child", 1000, 5500);

        with_env_storage_root(&db.root);
        let locations = ZCodeAdapter::new().scan().await;
        clear_env_storage_root();

        // Subagent excluded; ordered by time_updated DESC → newest first.
        let ids: Vec<String> = locations
            .iter()
            .map(|l| l.path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            ids,
            vec![
                "zcode://session/sess_interactive_2".to_string(),
                "zcode://session/sess_interactive_1".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn parse_session_extracts_title_cwd_branch_tokens_and_messages() {
        let db = TempDb::new();
        db.seed();
        let session_id = "sess_b7b1b5dd-45a7-4165-8b1a-3d507abbe295";
        db.insert_session(
            session_id,
            "Add ZCode adapter support",
            "/Users/maf/orbit",
            None,
            "interactive",
            1_782_322_453_561,
            1_784_651_441_848,
        );

        // User message carrying contextSnapshot.envInfo.gitBranch.
        db.insert_message(
            "msg_u1",
            session_id,
            1_782_322_453_562,
            serde_json::json!({
                "role": "user",
                "contextSnapshot": {"envInfo": {"cwd": "/Users/maf/orbit", "gitBranch": "main"}}
            }),
        );
        db.insert_part("p_u1", "msg_u1", session_id, 1_782_322_453_562, serde_json::json!({
            "type": "text", "text": "Add ZCode adapter support"
        }));

        // Assistant message: text + a tool call, with tokens + modelID.
        db.insert_message(
            "msg_a1",
            session_id,
            1_782_322_453_563,
            serde_json::json!({
                "role": "assistant",
                "modelID": "GLM-5.2",
                "tokens": {"input": 10916, "output": 270, "reasoning": 0, "cache": {"read": 8064}}
            }),
        );
        db.insert_part("p_a1_text", "msg_a1", session_id, 1_782_322_453_563, serde_json::json!({
            "type": "text", "text": "Let me look."
        }));
        db.insert_part("p_a1_tool", "msg_a1", session_id, 1_782_322_453_564, serde_json::json!({
            "type": "tool",
            "tool": "Read",
            "state": {
                "status": "completed",
                "input": {"file_path": "/src/main.rs"},
                "output": "file contents"
            }
        }));

        with_env_storage_root(&db.root);
        let parsed = ZCodeAdapter::new()
            .parse_session(Path::new(&format!("zcode://session/{}", session_id)))
            .await
            .unwrap();
        clear_env_storage_root();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.agent, AgentType::Zcode);
        assert_eq!(parsed.session.title, "Add ZCode adapter support");
        assert_eq!(parsed.session.project_path, "/Users/maf/orbit");
        assert_eq!(parsed.session.model.as_deref(), Some("GLM-5.2"));
        assert_eq!(parsed.session.git_branch.as_deref(), Some("main"));
        assert_eq!(parsed.session.parent_session_id, None);
        assert_eq!(parsed.session.input_tokens, 10916);
        assert_eq!(parsed.session.output_tokens, 270);
        assert_eq!(parsed.session.cached_tokens, 8064);
        assert_eq!(parsed.session.reasoning_tokens, 0);
        assert_eq!(
            parsed.session.created_at.to_rfc3339(),
            ms_to_datetime(1_782_322_453_561).to_rfc3339()
        );

        // user text, assistant text, assistant tool call.
        assert_eq!(parsed.messages.len(), 3);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Add ZCode adapter support");
        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert_eq!(parsed.messages[1].content, "Let me look.");
        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("Read"));
        assert!(parsed.messages[2].tool_input.as_deref().unwrap().contains("/src/main.rs"));
        assert_eq!(parsed.messages[2].tool_output.as_deref(), Some("file contents"));

        let touches: Vec<&str> = parsed.file_touches.iter().map(|t| t.path.as_str()).collect();
        assert!(touches.contains(&"/src/main.rs"));
        assert_eq!(parsed.file_touches[0].operation, "read");
    }

    #[tokio::test]
    async fn parse_session_resolves_subagent_parent() {
        let db = TempDb::new();
        db.seed();
        db.insert_session("sess_parent", "Parent", "/p", None, "interactive", 1000, 2000);
        db.insert_session(
            "sess_subagent_child",
            "Child",
            "/p",
            Some("sess_parent"),
            "subagent_child",
            1100,
            1900,
        );

        with_env_storage_root(&db.root);
        // parse_session works regardless of task_type — scan() is what filters, but a direct
        // parse of a subagent path should still resolve its parent_id.
        let parsed = ZCodeAdapter::new()
            .parse_session(Path::new("zcode://session/sess_subagent_child"))
            .await;
        clear_env_storage_root();

        let parsed = parsed.unwrap();
        assert_eq!(parsed.session.parent_session_id.as_deref(), Some("sess_parent"));
    }

    #[tokio::test]
    async fn parse_session_uses_error_as_output_when_no_output() {
        let db = TempDb::new();
        db.seed();
        let session_id = "sess_err";
        db.insert_session(session_id, "Err", "/p", None, "interactive", 1000, 2000);
        db.insert_message("msg_a1", session_id, 1100, serde_json::json!({"role": "assistant", "modelID": "GLM-5.2"}));
        db.insert_part("p_tool", "msg_a1", session_id, 1100, serde_json::json!({
            "type": "tool",
            "tool": "Read",
            "state": {
                "status": "error",
                "input": {"file_path": "/missing"},
                "error": "File not found"
            }
        }));

        with_env_storage_root(&db.root);
        let parsed = ZCodeAdapter::new()
            .parse_session(Path::new(&format!("zcode://session/{}", session_id)))
            .await
            .unwrap();
        clear_env_storage_root();

        let tool_msg = parsed.messages.iter().find(|m| m.role == MessageRole::Tool).unwrap();
        assert_eq!(tool_msg.tool_output.as_deref(), Some("File not found"));
    }
}

