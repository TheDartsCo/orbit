use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

pub struct ClaudeAdapter;

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn windows_projects_root(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".claude/projects")
    }

    pub(crate) fn windows_resume_command(session_id: &str, project_path: &str) -> String {
        let safe_path = crate::shell_quote::shell_quote(project_path);
        let safe_session = crate::shell_quote::shell_quote(session_id);
        format!(
            "Set-Location {}; claude --resume {}",
            safe_path, safe_session
        )
    }

    fn project_dirs_from_projects_dir(projects_dir: &Path) -> Vec<PathBuf> {
        if !projects_dir.is_dir() {
            return Vec::new();
        }

        std::fs::read_dir(projects_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .collect()
    }

    fn project_dirs(&self) -> Vec<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            dirs::home_dir()
                .map(|home| Self::project_dirs_from_projects_dir(&home.join(".claude/projects")))
                .unwrap_or_default()
        } else if cfg!(target_os = "windows") {
            Self::windows_projects_root(&PlatformPaths::system())
                .map(|root| Self::project_dirs_from_projects_dir(&root))
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

#[async_trait]
impl AgentAdapter for ClaudeAdapter {
    fn id(&self) -> &str {
        "claude"
    }

    fn name(&self) -> &str {
        "Claude Code"
    }

    async fn detect(&self) -> bool {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            dirs::home_dir().is_some_and(|home| home.join(".claude").exists())
        } else if cfg!(target_os = "windows") {
            Self::windows_projects_root(&PlatformPaths::system()).is_some_and(|path| path.is_dir())
        } else {
            false
        }
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let mut locations = Vec::new();
        for dir in self.project_dirs() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl")
                    {
                        let modified = std::fs::metadata(&path)
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| {
                                DateTime::from_timestamp(
                                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                                    0,
                                )
                            })
                            .unwrap_or_default();
                        locations.push(SessionLocation {
                            path,
                            last_modified: modified,
                        });
                    }
                }
            }
        }
        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read session file: {}", e))?;

        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut project_path = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut messages = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut title = String::from("Untitled Session");
        let mut seq: u32 = 0;
        let mut first_user_msg = true;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut model: Option<String> = None;
        let mut git_branch: Option<String> = None;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cached_tokens: u64 = 0;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

            if project_path.contains('-') && !project_path.contains('/') {
                if let Some(cwd) = json.get("cwd").and_then(|c| c.as_str()) {
                    project_path = cwd.to_string();
                }
            }

            match msg_type {
                "summary" => {
                    if let Some(sum) = json.get("summary").and_then(|s| s.as_str()) {
                        title = sum.to_string();
                    }
                    if git_branch.is_none() {
                        git_branch = json
                            .get("gitBranch")
                            .and_then(|b| b.as_str())
                            .map(ToString::to_string);
                    }
                }
                "user" | "human" => {
                    let content_text = json
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();

                    if first_user_msg && !content_text.is_empty() {
                        if is_subagent_prompt(&content_text) {
                            return Err("Subagent/plugin session, skipping".to_string());
                        }
                        title = content_text.chars().take(100).collect();
                        first_user_msg = false;
                    }

                    let timestamp = json
                        .get("timestamp")
                        .and_then(|t| t.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    if seq == 0 {
                        created_at = timestamp.unwrap_or(Utc::now());
                    }
                    updated_at = timestamp.unwrap_or(Utc::now());

                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: file_name.clone(),
                        role: MessageRole::User,
                        content: content_text,
                        timestamp,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;
                }
                "assistant" => {
                    let message_obj = json.get("message");
                    let content_text = message_obj
                        .and_then(|m| m.get("content"))
                        .and_then(|c| {
                            if c.is_string() {
                                c.as_str().map(|s| s.to_string())
                            } else if c.is_array() {
                                let texts: Vec<String> = c
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .filter_map(|block| {
                                        if block.get("type").and_then(|t| t.as_str())
                                            == Some("text")
                                        {
                                            block
                                                .get("text")
                                                .and_then(|t| t.as_str())
                                                .map(|s| s.to_string())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                Some(texts.join("\n"))
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    if model.is_none() {
                        model = message_obj
                            .and_then(|m| m.get("model"))
                            .and_then(|m| m.as_str())
                            .map(ToString::to_string);
                    }

                    if let Some(usage) = message_obj.and_then(|m| m.get("usage")) {
                        if let Some(n) = usage.get("input_tokens").and_then(|v| v.as_u64()) {
                            input_tokens += n;
                        }
                        if let Some(n) = usage.get("output_tokens").and_then(|v| v.as_u64()) {
                            output_tokens += n;
                        }
                        let cache_read = usage
                            .get("cache_read_input_tokens")
                            .and_then(|v| v.as_u64());
                        let cache_creation = usage
                            .get("cache_creation_input_tokens")
                            .and_then(|v| v.as_u64());
                        cached_tokens += cache_read.unwrap_or(0) + cache_creation.unwrap_or(0);
                    }

                    let tool_calls: Vec<ToolCallInfo> = message_obj
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter_map(|block| {
                                    if block.get("type").and_then(|t| t.as_str())
                                        == Some("tool_use")
                                    {
                                        Some(ToolCallInfo {
                                            name: block
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("")
                                                .to_string(),
                                            input: block.get("input").map(|i| i.to_string()),
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    let timestamp = json
                        .get("timestamp")
                        .and_then(|t| t.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    updated_at = timestamp.unwrap_or(Utc::now());

                    if !content_text.is_empty() {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: file_name.clone(),
                            role: MessageRole::Assistant,
                            content: content_text,
                            timestamp,
                            sequence: seq,
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                        });
                        seq += 1;
                    }

                    for tc in &tool_calls {
                        if let Some(path) = extract_file_path(&tc.name, tc.input.as_deref()) {
                            let op = file_operation_for(&tc.name);
                            file_touches.push(FileTouch {
                                path,
                                operation: op,
                                sequence: seq,
                            });
                        }
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: file_name.clone(),
                            role: MessageRole::Tool,
                            content: String::new(),
                            timestamp,
                            sequence: seq,
                            tool_name: Some(tc.name.clone()),
                            tool_input: tc.input.clone(),
                            tool_output: None,
                        });
                        seq += 1;
                    }
                }
                "tool_result" => {
                    let content_text = json
                        .get("content")
                        .and_then(|c| {
                            if c.is_string() {
                                c.as_str().map(|s| s.to_string())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    for msg in messages.iter_mut().rev() {
                        if msg.tool_name.is_some() && msg.tool_output.is_none() {
                            msg.tool_output = Some(content_text.clone());
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        let session = Session {
            id: file_name.clone(),
            parent_session_id: None,
            agent: AgentType::Claude,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            git_branch,
            input_tokens,
            output_tokens,
            cached_tokens,
            reasoning_tokens: 0,
            file_count: 0,
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
        format!("cd {} && claude --resume {}", safe_path, safe_session)
    }

    async fn is_active(&self, session_path: &Path) -> bool {
        let home = dirs::home_dir().unwrap_or_default();
        let lock_path = home.join(".claude").join(format!(
            "{}.lock",
            session_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
        ));
        lock_path.exists()
    }
}

struct ToolCallInfo {
    name: String,
    input: Option<String>,
}

fn is_subagent_prompt(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("claude-mem") || content.trim().starts_with("Hello memory agent")
}

fn file_operation_for(tool_name: &str) -> String {
    match tool_name {
        "Read" => "read".to_string(),
        "Edit" | "MultiEdit" | "Update" => "edit".to_string(),
        "Write" | "Create" | "NotebookEdit" => "write".to_string(),
        _ => "unknown".to_string(),
    }
}

fn extract_file_path(tool_name: &str, input: Option<&str>) -> Option<String> {
    let raw = input?;
    let parsed: serde_json::Value = serde_json::from_str(raw).ok()?;
    for key in &["file_path", "path", "notebook_path"] {
        if let Some(p) = parsed.get(*key).and_then(|v| v.as_str()) {
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    if matches!(tool_name, "Edit" | "MultiEdit" | "Update") {
        if let Some(arr) = parsed.get("edits").and_then(|v| v.as_array()) {
            if let Some(first) = arr.first() {
                if let Some(p) = first.get("file_path").and_then(|v| v.as_str()) {
                    if !p.is_empty() {
                        return Some(p.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;
    use std::fs;

    fn write_claude_session(
        dir: &std::path::Path,
        name: &str,
        content: &str,
    ) -> std::path::PathBuf {
        let project_dir = dir.join("project1");
        fs::create_dir_all(&project_dir).unwrap();
        let path = project_dir.join(format!("{}.jsonl", name));
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn project_dirs_from_projects_dir_returns_only_direct_project_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let projects_dir = tmp.path().join("projects");
        let project_a = projects_dir.join("project-a");
        let project_b = projects_dir.join("project-b");
        let nested_session_dir = project_a.join("session-1");
        let nested_subagents_dir = nested_session_dir.join("subagents");

        fs::create_dir_all(&project_a).unwrap();
        fs::create_dir_all(&project_b).unwrap();
        fs::create_dir_all(&nested_subagents_dir).unwrap();
        fs::write(projects_dir.join("not-a-project.jsonl"), "{}").unwrap();

        let mut dirs = ClaudeAdapter::project_dirs_from_projects_dir(&projects_dir);
        dirs.sort();

        assert_eq!(dirs, vec![project_a, project_b]);
    }

    #[tokio::test]
    async fn extracts_model_tokens_branch_and_file_touches() {
        let tmp = tempfile::tempdir().unwrap();
        let jsonl = "\
            {\"type\":\"summary\",\"summary\":\"Fix bug\",\"gitBranch\":\"feat/auth\"}\n\
            {\"type\":\"user\",\"timestamp\":\"2026-04-19T18:00:00Z\",\"message\":{\"content\":\"fix it\"}}\n\
            {\"type\":\"assistant\",\"timestamp\":\"2026-04-19T18:00:05Z\",\"message\":{\"model\":\"claude-sonnet-4-5\",\"usage\":{\"input_tokens\":100,\"output_tokens\":50,\"cache_read_input_tokens\":80},\"content\":[\
                {\"type\":\"text\",\"text\":\"reading file\"},\
                {\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"file_path\":\"/src/foo.rs\"}}\
            ]}}\n\
            {\"type\":\"assistant\",\"timestamp\":\"2026-04-19T18:00:10Z\",\"message\":{\"model\":\"claude-sonnet-4-5\",\"usage\":{\"input_tokens\":200,\"output_tokens\":30,\"cache_read_input_tokens\":150},\"content\":[\
                {\"type\":\"tool_use\",\"name\":\"Edit\",\"input\":{\"file_path\":\"/src/foo.rs\"}}\
            ]}}\n\
        ";
        let path = write_claude_session(tmp.path(), "s1", jsonl);
        let adapter = ClaudeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(parsed.session.input_tokens, 300);
        assert_eq!(parsed.session.output_tokens, 80);
        assert_eq!(parsed.session.cached_tokens, 230);
        assert_eq!(parsed.session.git_branch.as_deref(), Some("feat/auth"));

        let paths: Vec<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert!(paths.contains(&"/src/foo.rs"));
        let ops: Vec<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.operation.as_str())
            .collect();
        assert!(ops.contains(&"read"));
        assert!(ops.contains(&"edit"));
    }

    #[tokio::test]
    async fn missing_metadata_leaves_fields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let jsonl = "\
            {\"type\":\"user\",\"timestamp\":\"2026-04-19T18:00:00Z\",\"message\":{\"content\":\"hi\"}}\n\
        ";
        let path = write_claude_session(tmp.path(), "s2", jsonl);
        let adapter = ClaudeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();
        assert!(parsed.session.model.is_none());
        assert_eq!(parsed.session.input_tokens, 0);
        assert!(parsed.session.git_branch.is_none());
        assert!(parsed.file_touches.is_empty());
    }
}
