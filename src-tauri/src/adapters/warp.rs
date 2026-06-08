use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::OpenFlags;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::{AgentType, FileTouch, Message, MessageRole, NormalizedSession, Session};

const WARP_DB_PATH: &str = "Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/warp.sqlite";

pub struct WarpAdapter {
    cache: Mutex<HashMap<String, Vec<u8>>>,
}

impl Default for WarpAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl WarpAdapter {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) fn windows_candidate_db_paths(paths: &PlatformPaths) -> Vec<PathBuf> {
        [
            paths.data_local_join("warp/Warp/data/warp.sqlite"),
            paths.data_join("Warp/warp.sqlite"),
            paths.data_join("dev.warp.Warp-Stable/warp.sqlite"),
            paths.data_local_join("Warp/warp.sqlite"),
            paths.data_local_join("dev.warp.Warp-Stable/warp.sqlite"),
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
        "Start-Process Warp"
    }

    fn db_path() -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            let home = dirs::home_dir()?;
            let path = home.join(WARP_DB_PATH);
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

    fn read_all_tasks(&self) -> Vec<(String, DateTime<Utc>, Vec<u8>)> {
        let db_path = match Self::db_path() {
            Some(p) => p,
            None => return Vec::new(),
        };

        let conn = match rusqlite::Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            "SELECT task_id, last_modified_at, task FROM agent_tasks ORDER BY last_modified_at DESC"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        let rows = match stmt.query_map([], |row| {
            let task_id: String = row.get(0)?;
            let modified_str: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;
            let modified = chrono::DateTime::parse_from_rfc3339(&modified_str)
                .map(|dt| dt.with_timezone(&Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&modified_str, "%Y-%m-%d %H:%M:%S")
                        .map(|nd| Utc.from_utc_datetime(&nd))
                })
                .unwrap_or_default();
            Ok((task_id, modified, blob))
        }) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        let mut results = Vec::new();
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        cache.clear();
        for row in rows.flatten() {
            let (task_id, _last_modified, blob) = &row;
            cache.insert(task_id.clone(), blob.clone());
            results.push(row);
        }
        results
    }
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct ProtoTimestamp {
    #[prost(int64, tag = "1")]
    pub seconds: i64,
    #[prost(int32, tag = "2")]
    pub nanos: i32,
}

impl ProtoTimestamp {
    fn to_chrono(&self) -> DateTime<Utc> {
        Utc.timestamp_opt(self.seconds, self.nanos as u32)
            .single()
            .unwrap_or_else(|| Utc::now())
    }
}

#[derive(Clone, prost::Message)]
pub struct UserContext {
    #[prost(message, tag = "1")]
    pub cwd_info: Option<CwdInfo>,
    #[prost(message, repeated, tag = "8")]
    pub projects: Vec<ProjectInfo>,
}

#[derive(Clone, prost::Message)]
pub struct CwdInfo {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, prost::Message)]
pub struct ProjectInfo {
    #[prost(string, tag = "1")]
    pub name: String,
    #[prost(string, tag = "2")]
    pub path: String,
}

#[derive(Clone, prost::Message)]
pub struct UserMessage {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(message, tag = "2")]
    pub context: Option<UserContext>,
}

#[derive(Clone, prost::Message)]
pub struct AssistantText {
    #[prost(string, tag = "1")]
    pub text: String,
}

#[derive(Clone, prost::Message)]
pub struct ToolCall {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(message, tag = "2")]
    pub run_command: Option<RunCommand>,
    #[prost(message, tag = "5")]
    pub read_files: Option<ReadFiles>,
    #[prost(message, tag = "6")]
    pub apply_diff: Option<ApplyFileDiff>,
    #[prost(message, tag = "9")]
    pub grep_search: Option<GrepSearch>,
}

#[derive(Clone, prost::Message)]
pub struct RunCommand {
    #[prost(string, tag = "1")]
    pub command: String,
}

#[derive(Clone, prost::Message)]
pub struct ReadFiles {
    #[prost(message, repeated, tag = "1")]
    pub file_paths: Vec<FilePathEntry>,
}

#[derive(Clone, prost::Message)]
pub struct FilePathEntry {
    #[prost(string, tag = "1")]
    pub path: String,
}

#[derive(Clone, prost::Message)]
pub struct ApplyFileDiff {
    #[prost(string, tag = "1")]
    pub description: String,
    #[prost(message, repeated, tag = "2")]
    pub diffs: Vec<FileDiffEntry>,
}

#[derive(Clone, prost::Message)]
pub struct FileDiffEntry {
    #[prost(string, tag = "1")]
    pub file_path: String,
    #[prost(string, tag = "2")]
    pub original: String,
    #[prost(string, tag = "3")]
    pub replacement: String,
}

#[derive(Clone, prost::Message)]
pub struct GrepSearch {
    #[prost(string, repeated, tag = "1")]
    pub patterns: Vec<String>,
    #[prost(string, tag = "2")]
    pub path: String,
}

#[derive(Clone, prost::Message)]
pub struct ToolResult {
    #[prost(string, tag = "1")]
    pub tool_call_id: String,
    #[prost(message, tag = "2")]
    pub command_result: Option<CommandResult>,
    #[prost(message, tag = "5")]
    pub read_files_result: Option<ReadFilesResult>,
}

#[derive(Clone, prost::Message)]
pub struct CommandResult {
    #[prost(string, tag = "1")]
    pub output: String,
}

#[derive(Clone, prost::Message)]
pub struct ReadFilesResult {
    #[prost(message, repeated, tag = "3")]
    pub files: Vec<FileContentEntry>,
}

#[derive(Clone, prost::Message)]
pub struct FileContentEntry {
    #[prost(string, tag = "2")]
    pub content: String,
}

#[derive(Clone, prost::Message)]
pub struct ThinkingMessage {
    #[prost(string, tag = "1")]
    pub data: String,
}

#[derive(Clone, prost::Message)]
pub struct DetailedThinking {
    #[prost(string, tag = "1")]
    pub text: String,
}

#[derive(Clone, prost::Message)]
pub struct FileAttachment {
    #[prost(string, tag = "1")]
    pub path: String,
    #[prost(uint32, tag = "2")]
    pub line_count: u32,
}

#[derive(Clone, prost::Message)]
pub struct TaskEntry {
    #[prost(string, tag = "1")]
    pub message_id: String,
    #[prost(message, tag = "2")]
    pub user_message: Option<UserMessage>,
    #[prost(message, tag = "3")]
    pub assistant_text: Option<AssistantText>,
    #[prost(message, tag = "4")]
    pub tool_call: Option<ToolCall>,
    #[prost(message, tag = "5")]
    pub tool_result: Option<ToolResult>,
    #[prost(message, tag = "6")]
    pub thinking: Option<ThinkingMessage>,
    #[prost(bytes, tag = "7")]
    pub continuation_token: Vec<u8>,
    #[prost(message, tag = "8")]
    pub file_attachment: Option<FileAttachment>,
    #[prost(string, tag = "11")]
    pub conversation_id: String,
    #[prost(string, tag = "13")]
    pub model_id: String,
    #[prost(message, tag = "14")]
    pub timestamp: Option<ProtoTimestamp>,
    #[prost(message, tag = "15")]
    pub detailed_thinking: Option<DetailedThinking>,
}

#[derive(Clone, prost::Message)]
pub struct WarpTask {
    #[prost(string, tag = "1")]
    pub task_id: String,
    #[prost(string, tag = "2")]
    pub title: String,
    #[prost(message, repeated, tag = "5")]
    pub entries: Vec<TaskEntry>,
}

fn tool_call_summary(tc: &ToolCall) -> (String, Option<String>) {
    if let Some(ref rc) = tc.run_command {
        return ("run_command".to_string(), Some(rc.command.clone()));
    }
    if let Some(ref rf) = tc.read_files {
        let paths: Vec<&str> = rf.file_paths.iter().map(|e| e.path.as_str()).collect();
        return ("read_files".to_string(), Some(paths.join(", ")));
    }
    if let Some(ref ad) = tc.apply_diff {
        return ("apply_file_diff".to_string(), Some(ad.description.clone()));
    }
    if let Some(ref gs) = tc.grep_search {
        return (
            "grep_search".to_string(),
            Some(format!("{:?} in {}", gs.patterns, gs.path)),
        );
    }
    ("unknown_tool".to_string(), None)
}

fn warp_file_touches(tc: &ToolCall, sequence: u32) -> Vec<FileTouch> {
    let mut out = Vec::new();
    if let Some(ref rf) = tc.read_files {
        for entry in &rf.file_paths {
            if !entry.path.is_empty() {
                out.push(FileTouch {
                    path: entry.path.clone(),
                    operation: "read".to_string(),
                    sequence,
                });
            }
        }
    }
    if let Some(ref ad) = tc.apply_diff {
        for diff in &ad.diffs {
            if !diff.file_path.is_empty() {
                out.push(FileTouch {
                    path: diff.file_path.clone(),
                    operation: "edit".to_string(),
                    sequence,
                });
            }
        }
    }
    out
}

fn tool_result_output(tr: &ToolResult) -> String {
    if let Some(ref cr) = tr.command_result {
        return cr.output.clone();
    }
    if let Some(ref rf) = tr.read_files_result {
        let contents: Vec<&str> = rf.files.iter().map(|f| f.content.as_str()).collect();
        return contents.join("\n---\n");
    }
    String::new()
}

fn extract_project_path(entries: &[TaskEntry]) -> String {
    for entry in entries {
        if let Some(ref um) = entry.user_message {
            if let Some(ref ctx) = um.context {
                if let Some(ref cwd) = ctx.cwd_info {
                    if !cwd.path.is_empty() {
                        return cwd.path.clone();
                    }
                }
                for proj in &ctx.projects {
                    if !proj.path.is_empty() {
                        return proj.path.clone();
                    }
                }
            }
        }
    }
    String::new()
}

#[async_trait]
impl AgentAdapter for WarpAdapter {
    fn id(&self) -> &str {
        "warp"
    }

    fn name(&self) -> &str {
        "Warp"
    }

    async fn detect(&self) -> bool {
        Self::db_path().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let all_tasks = self.read_all_tasks();
        all_tasks
            .into_iter()
            .map(|(task_id, last_modified, _)| SessionLocation {
                path: PathBuf::from(format!("warp://task/{}", task_id)),
                last_modified,
            })
            .collect()
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let path_str = path.to_string_lossy();
        let task_id = path_str
            .strip_prefix("warp://task/")
            .ok_or_else(|| "Invalid warp session path".to_string())?
            .to_string();

        let blob = {
            let cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
            cache
                .get(&task_id)
                .cloned()
                .ok_or_else(|| format!("Task {} not in cache", task_id))?
        };

        let task = <WarpTask as prost::Message>::decode(blob.as_slice())
            .map_err(|e| format!("Failed to decode protobuf for {}: {}", task_id, e))?;

        let mut messages = Vec::new();
        let mut seq: u32 = 0;
        let mut title = String::new();
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut model: Option<String> = None;

        if !task.title.is_empty() {
            title = task.title.clone();
        }

        let project_path = extract_project_path(&task.entries);

        for entry in &task.entries {
            if !entry.model_id.is_empty() && model.is_none() {
                model = Some(entry.model_id.clone());
            }
        }

        let mut pending_tool_outputs: HashMap<String, String> = HashMap::new();

        for entry in &task.entries {
            let ts = entry.timestamp.as_ref().map(|t| t.to_chrono());

            if seq == 0 {
                created_at = ts.unwrap_or(Utc::now());
            }
            if ts.is_some() {
                updated_at = ts.unwrap_or(Utc::now());
            }

            if let Some(ref um) = entry.user_message {
                if title.is_empty() && !um.text.is_empty() {
                    title = um.text.chars().take(100).collect();
                }

                messages.push(Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: task_id.clone(),
                    role: MessageRole::User,
                    content: um.text.clone(),
                    timestamp: ts,
                    sequence: seq,
                    tool_name: None,
                    tool_input: None,
                    tool_output: None,
                });
                seq += 1;
            }

            if let Some(ref at) = entry.assistant_text {
                if !at.text.is_empty() {
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: task_id.clone(),
                        role: MessageRole::Assistant,
                        content: at.text.clone(),
                        timestamp: ts,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;
                }
            }

            if let Some(ref dt) = entry.detailed_thinking {
                if !dt.text.is_empty() {
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: task_id.clone(),
                        role: MessageRole::Assistant,
                        content: dt.text.clone(),
                        timestamp: ts,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;
                }
            }

            if let Some(ref tc) = entry.tool_call {
                let (tool_name, tool_input) = tool_call_summary(tc);
                let tc_id = tc.tool_call_id.clone();

                for ft in warp_file_touches(tc, seq) {
                    file_touches.push(ft);
                }

                if let Some(output) = pending_tool_outputs.remove(&tc_id) {
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: task_id.clone(),
                        role: MessageRole::Tool,
                        content: String::new(),
                        timestamp: ts,
                        sequence: seq,
                        tool_name: Some(tool_name),
                        tool_input,
                        tool_output: Some(output),
                    });
                } else {
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: task_id.clone(),
                        role: MessageRole::Tool,
                        content: String::new(),
                        timestamp: ts,
                        sequence: seq,
                        tool_name: Some(tool_name),
                        tool_input,
                        tool_output: None,
                    });
                }
                seq += 1;
            }

            if let Some(ref tr) = entry.tool_result {
                let output = tool_result_output(tr);
                if !output.is_empty() {
                    let matched = messages.iter_mut().rev().find(|m| {
                        m.role == MessageRole::Tool
                            && m.tool_name.is_some()
                            && m.tool_output.is_none()
                    });

                    if let Some(msg) = matched {
                        msg.tool_output = Some(output);
                    } else {
                        pending_tool_outputs.insert(tr.tool_call_id.clone(), output);
                    }
                }
            }
        }

        if title.is_empty() {
            title = format!(
                "Warp Session {}",
                &task_id.chars().take(8).collect::<String>()
            );
        }

        let session = Session {
            id: task_id,
            parent_session_id: None,
            agent: AgentType::Warp,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }

    fn resume_command(&self, _session_id: &str, _project_path: &str) -> String {
        if cfg!(target_os = "windows") {
            Self::windows_resume_command().to_string()
        } else {
            "open -a Warp".to_string()
        }
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        if let Some(db_path) = Self::db_path() {
            if let Ok(metadata) = std::fs::metadata(&db_path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        return elapsed.as_secs() < 30;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_file_touches_extracts_read_files_and_apply_diff_paths() {
        let mut read = ReadFiles::default();
        read.file_paths.push(FilePathEntry {
            path: "/src/foo.rs".to_string(),
        });
        read.file_paths.push(FilePathEntry {
            path: "/src/bar.rs".to_string(),
        });
        let mut apply = ApplyFileDiff::default();
        apply.diffs.push(FileDiffEntry {
            file_path: "/src/baz.rs".to_string(),
            original: "a".to_string(),
            replacement: "b".to_string(),
        });
        let tc = ToolCall {
            tool_call_id: "tc1".to_string(),
            run_command: None,
            read_files: Some(read),
            apply_diff: Some(apply),
            grep_search: None,
        };
        let touches = warp_file_touches(&tc, 5);
        assert_eq!(touches.len(), 3, "got {:?}", touches);
        let paths: Vec<&str> = touches.iter().map(|t| t.path.as_str()).collect();
        assert!(paths.contains(&"/src/foo.rs"), "got {:?}", paths);
        assert!(paths.contains(&"/src/bar.rs"), "got {:?}", paths);
        assert!(paths.contains(&"/src/baz.rs"), "got {:?}", paths);
        assert!(touches.iter().all(|t| t.sequence == 5));
        let read_op = touches
            .iter()
            .find(|t| t.path == "/src/foo.rs")
            .map(|t| t.operation.as_str());
        let edit_op = touches
            .iter()
            .find(|t| t.path == "/src/baz.rs")
            .map(|t| t.operation.as_str());
        assert_eq!(read_op, Some("read"));
        assert_eq!(edit_op, Some("edit"));
    }

    #[test]
    fn warp_file_touches_skips_empty_paths() {
        let read = ReadFiles {
            file_paths: vec![FilePathEntry {
                path: String::new(),
            }],
        };
        let tc = ToolCall {
            tool_call_id: "tc1".to_string(),
            run_command: None,
            read_files: Some(read),
            apply_diff: None,
            grep_search: None,
        };
        let touches = warp_file_touches(&tc, 0);
        assert!(touches.is_empty(), "expected no touches, got {:?}", touches);
    }
}
