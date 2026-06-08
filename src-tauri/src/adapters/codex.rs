use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

pub struct CodexAdapter;

impl CodexAdapter {
    pub fn new() -> Self {
        Self
    }

    fn data_dir_path_from_home(home: &Path) -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(home.join(".codex"))
        } else {
            None
        }
    }

    pub(crate) fn windows_data_dir(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".codex")
    }

    fn data_dir() -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            let home = dirs::home_dir()?;
            Self::data_dir_path_from_home(&home).filter(|path| path.is_dir())
        } else if cfg!(target_os = "windows") {
            Self::windows_data_dir(&PlatformPaths::system()).filter(|path| path.is_dir())
        } else {
            None
        }
    }

    fn scan_dir_recursive(root: &Path, dir: &Path, locations: &mut Vec<SessionLocation>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with('.') || name == "node_modules" {
                        continue;
                    }
                    Self::scan_dir_recursive(root, &path, locations);
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    if path.parent() == Some(root) {
                        continue;
                    }
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
}

fn extract_text_from_content(content: &serde_json::Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            if let Some(text) = item.as_str() {
                if !text.trim().is_empty() {
                    parts.push(text.to_string());
                }
                continue;
            }
            let item_type = item.get("type").and_then(|t| t.as_str());
            if matches!(
                item_type,
                None | Some("text") | Some("input_text") | Some("output_text")
            ) {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    if !text.trim().is_empty() {
                        parts.push(text.to_string());
                    }
                }
            }
        }
        return parts.join("\n");
    }
    String::new()
}

fn is_preamble(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<turn_aborted>")
        || trimmed.starts_with("<permissions instructions>")
        || trimmed.starts_with("# AGENTS.md instructions for ")
        || trimmed.starts_with("# AGENTS.md instructions\n")
        || trimmed.starts_with("# Context from my IDE setup")
        || trimmed.starts_with("# AGENTS.md from")
        || trimmed.starts_with("# Codebase Context")
}

fn codex_file_operation_for(tool_name: &str) -> String {
    match tool_name {
        "read_file" | "view_image" => "read".to_string(),
        "edit_file" | "apply_patch" | "multi_edit" => "edit".to_string(),
        "create_file" | "write_file" => "write".to_string(),
        _ => "unknown".to_string(),
    }
}

fn codex_extract_file_path(tool_name: &str, arguments: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(arguments).ok()?;
    for key in &["file_path", "path"] {
        if let Some(p) = parsed.get(*key).and_then(|v| v.as_str()) {
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    if tool_name == "apply_patch" {
        let s = arguments;
        for line in s.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("*** Update File: ") || trimmed.starts_with("*** Add File: ") {
                let path = trimmed
                    .trim_start_matches("*** Update File: ")
                    .trim_start_matches("*** Add File: ")
                    .trim();
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }
    None
}

#[async_trait]
impl AgentAdapter for CodexAdapter {
    fn id(&self) -> &str {
        "codex"
    }

    fn name(&self) -> &str {
        "Codex"
    }

    async fn detect(&self) -> bool {
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let Some(data_dir) = Self::data_dir() else {
            return Vec::new();
        };

        let mut locations = Vec::new();
        Self::scan_dir_recursive(&data_dir, &data_dir, &mut locations);
        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;

        let file_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut session_id = file_name.clone();
        let mut parent_session_id: Option<String> = None;
        let mut messages = Vec::new();
        let mut title = String::new();
        let mut seq: u32 = 0;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut project_path = String::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut model: Option<String> = None;
        let mut git_branch: Option<String> = None;
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cached_tokens: u64 = 0;
        let mut reasoning_tokens: u64 = 0;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let entry_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

            let timestamp = json
                .get("timestamp")
                .and_then(|t| t.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if seq == 0 {
                created_at = timestamp.unwrap_or(Utc::now());
            }
            if timestamp.is_some() {
                updated_at = timestamp.clone().unwrap_or(Utc::now());
            }

            match entry_type {
                "session_meta" => {
                    if let Some(payload) = json.get("payload") {
                        if let Some(id) = payload.get("id").and_then(|v| v.as_str()) {
                            if !id.is_empty() {
                                session_id = id.to_string();
                            }
                        }
                        if let Some(cwd) = payload.get("cwd").and_then(|c| c.as_str()) {
                            project_path = cwd.to_string();
                        }
                        parent_session_id = payload
                            .get("source")
                            .and_then(|s| s.get("subagent"))
                            .and_then(|s| s.get("thread_spawn"))
                            .and_then(|t| t.get("parent_thread_id"))
                            .and_then(|v| v.as_str())
                            .map(ToString::to_string);
                        if git_branch.is_none() {
                            git_branch = payload
                                .get("gitInfo")
                                .and_then(|g| g.get("branch"))
                                .and_then(|b| b.as_str())
                                .map(ToString::to_string)
                                .or_else(|| {
                                    payload
                                        .get("git")
                                        .and_then(|g| g.get("branch"))
                                        .and_then(|b| b.as_str())
                                        .map(ToString::to_string)
                                });
                        }
                    }
                    continue;
                }
                "turn_context" => {
                    if model.is_none() {
                        if let Some(m) = json
                            .get("payload")
                            .and_then(|p| p.get("model"))
                            .and_then(|m| m.as_str())
                        {
                            model = Some(m.to_string());
                        }
                    }
                    continue;
                }
                "response_item" => {
                    let payload = match json.get("payload") {
                        Some(p) => p,
                        None => continue,
                    };
                    let payload_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match payload_type {
                        "message" => {
                            let role_str =
                                payload.get("role").and_then(|r| r.as_str()).unwrap_or("");
                            let role = match role_str {
                                "user" | "human" => MessageRole::User,
                                "assistant" => MessageRole::Assistant,
                                "system" | "developer" => MessageRole::System,
                                _ => continue,
                            };

                            if model.is_none() {
                                model = payload
                                    .get("model")
                                    .and_then(|m| m.as_str())
                                    .map(ToString::to_string);
                            }

                            let msg_content = extract_text_from_content(
                                payload.get("content").unwrap_or(&serde_json::Value::Null),
                            );

                            if role == MessageRole::User
                                && title.is_empty()
                                && !msg_content.trim().is_empty()
                                && !is_preamble(&msg_content)
                            {
                                title = msg_content.chars().take(100).collect();
                            }

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role,
                                content: msg_content,
                                timestamp: timestamp.clone(),
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                            });
                            seq += 1;
                        }
                        "function_call" => {
                            let tool_name = payload
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let arguments_str = payload.get("arguments").and_then(|a| a.as_str());
                            let tool_input = arguments_str.map(|a| a.to_string());

                            if let Some(args) = arguments_str {
                                if let Some(path) = codex_extract_file_path(&tool_name, args) {
                                    let op = codex_file_operation_for(&tool_name);
                                    file_touches.push(FileTouch {
                                        path,
                                        operation: op,
                                        sequence: seq,
                                    });
                                }
                            }

                            if title.is_empty() {
                                title = format!("[Tool: {}]", tool_name);
                            }

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: timestamp.clone(),
                                sequence: seq,
                                tool_name: Some(tool_name),
                                tool_input,
                                tool_output: None,
                            });
                            seq += 1;
                        }
                        "function_call_output" => {
                            let output = payload
                                .get("output")
                                .and_then(|o| o.as_str())
                                .unwrap_or("")
                                .to_string();

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: timestamp.clone(),
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: Some(output),
                            });
                            seq += 1;
                        }
                        _ => continue,
                    }
                }
                "event_msg" => {
                    let payload = match json.get("payload") {
                        Some(p) => p,
                        None => continue,
                    };
                    let event_type = payload.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "user_message" => {
                            let msg_content = payload
                                .get("message")
                                .and_then(|m| m.as_str())
                                .unwrap_or("")
                                .to_string();

                            if title.is_empty()
                                && !msg_content.trim().is_empty()
                                && !is_preamble(&msg_content)
                            {
                                title = msg_content.chars().take(100).collect();
                            }

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::User,
                                content: msg_content,
                                timestamp: timestamp.clone(),
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: None,
                            });
                            seq += 1;
                        }
                        "agent_message" => {
                            let msg_content = payload
                                .get("message")
                                .or_else(|| payload.get("text"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("")
                                .to_string();

                            if !msg_content.trim().is_empty() {
                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: session_id.clone(),
                                    role: MessageRole::Assistant,
                                    content: msg_content,
                                    timestamp: timestamp.clone(),
                                    sequence: seq,
                                    tool_name: None,
                                    tool_input: None,
                                    tool_output: None,
                                });
                                seq += 1;
                            }
                        }
                        "tool_result" => {
                            let output = payload
                                .get("output")
                                .or_else(|| payload.get("result"))
                                .and_then(|o| o.as_str())
                                .unwrap_or("")
                                .to_string();

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp: timestamp.clone(),
                                sequence: seq,
                                tool_name: None,
                                tool_input: None,
                                tool_output: Some(output),
                            });
                            seq += 1;
                        }
                        "token_count" => {
                            if let Some(usage) =
                                payload.get("info").and_then(|i| i.get("total_token_usage"))
                            {
                                if let Some(n) = usage.get("input_tokens").and_then(|v| v.as_u64())
                                {
                                    input_tokens = input_tokens.saturating_add(n);
                                }
                                if let Some(n) = usage.get("output_tokens").and_then(|v| v.as_u64())
                                {
                                    output_tokens = output_tokens.saturating_add(n);
                                }
                                if let Some(n) =
                                    usage.get("cached_input_tokens").and_then(|v| v.as_u64())
                                {
                                    cached_tokens = cached_tokens.saturating_add(n);
                                }
                                if let Some(n) =
                                    usage.get("reasoning_tokens").and_then(|v| v.as_u64())
                                {
                                    reasoning_tokens = reasoning_tokens.saturating_add(n);
                                }
                            }
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            }
        }

        if title.is_empty() {
            let dir_name = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if dir_name == "archived_sessions" || dir_name.is_empty() {
                title = format!(
                    "Session {}",
                    &session_id.chars().take(8).collect::<String>()
                );
            } else {
                let display = dir_name
                    .trim_start_matches("rollout-")
                    .chars()
                    .take(40)
                    .collect::<String>();
                title = display;
            }
        }

        let session = Session {
            id: session_id,
            parent_session_id,
            agent: AgentType::Codex,
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

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        let safe = crate::shell_quote::shell_quote(session_id);
        format!("codex resume {}", safe)
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
    fn data_dir_path_from_home_uses_dot_codex_on_unix() {
        let home = std::path::Path::new("/home/orbit-user");

        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert_eq!(
                CodexAdapter::data_dir_path_from_home(home),
                Some(home.join(".codex"))
            );
        } else {
            assert!(CodexAdapter::data_dir_path_from_home(home).is_none());
        }
    }

    #[test]
    fn resume_command_uses_current_codex_resume_subcommand() {
        let adapter = CodexAdapter::new();

        assert_eq!(
            adapter.resume_command("019e9481-bb65-72f3-a053-27f3c85d7671", ""),
            "codex resume '019e9481-bb65-72f3-a053-27f3c85d7671'"
        );
    }

    #[tokio::test]
    async fn parses_subagent_rollout_with_parent_thread_id() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "019da610-77cd-74f1-8d45-90d8610199b5";
        let parent_id = "019da5e7-d63a-7791-9a33-ddb11c729cdb";
        let rollout = temp
            .path()
            .join(format!("rollout-2026-04-19T18-06-30-{}.jsonl", session_id));

        std::fs::write(
            &rollout,
            format!(
                concat!(
                    "{{\"timestamp\":\"2026-04-19T18:06:30.000Z\",\"type\":\"session_meta\",\"payload\":{{",
                    "\"id\":\"{sid}\",\"timestamp\":\"2026-04-19T18:06:30.000Z\",",
                    "\"cwd\":\"/tmp/parent-project\",\"originator\":\"Codex Desktop\",\"cli_version\":\"0.1.0\",",
                    "\"source\":{{\"subagent\":{{\"thread_spawn\":{{",
                    "\"parent_thread_id\":\"{pid}\",\"depth\":1,",
                    "\"agent_path\":null,\"agent_nickname\":\"Pauli\",\"agent_role\":\"default\"",
                    "}}}}}}",
                    "}}}}\n",
                    "{{\"timestamp\":\"2026-04-19T18:06:35.000Z\",\"type\":\"event_msg\",\"payload\":{{",
                    "\"type\":\"user_message\",\"message\":\"Investigate the failing test\"",
                    "}}}}\n",
                    "{{\"timestamp\":\"2026-04-19T18:06:40.000Z\",\"type\":\"event_msg\",\"payload\":{{",
                    "\"type\":\"agent_message\",\"message\":\"I found the bug.\"",
                    "}}}}\n"
                ),
                sid = session_id,
                pid = parent_id
            ),
        )
        .unwrap();

        let adapter = CodexAdapter::new();
        let parsed = adapter.parse_session(&rollout).await.unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.parent_session_id.as_deref(), Some(parent_id));
        assert_eq!(parsed.session.agent, AgentType::Codex);
        assert_eq!(parsed.session.project_path, "/tmp/parent-project");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Investigate the failing test");
        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert_eq!(parsed.messages[1].content, "I found the bug.");
    }

    #[tokio::test]
    async fn extracts_model_tokens_branch_and_file_touches() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rollout-2026-04-19T18-06-30-test.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"timestamp\":\"2026-04-19T18:00:00Z\",\"type\":\"session_meta\",\"payload\":{",
                "\"id\":\"ses_x\",\"timestamp\":\"2026-04-19T18:00:00Z\",",
                "\"cwd\":\"/tmp/proj\",",
                "\"gitInfo\":{\"branch\":\"feat/auth\",\"commit\":\"abc123\"},",
                "\"source\":null",
                "}}\n",
                "{\"timestamp\":\"2026-04-19T18:00:05Z\",\"type\":\"response_item\",\"payload\":{",
                "\"type\":\"function_call\",\"name\":\"read_file\",\"call_id\":\"c1\",\"arguments\":\"{\\\"file_path\\\":\\\"/src/foo.rs\\\"}\"",
                "}}\n",
                "{\"timestamp\":\"2026-04-19T18:00:10Z\",\"type\":\"response_item\",\"payload\":{",
                "\"type\":\"function_call\",\"name\":\"edit_file\",\"call_id\":\"c2\",\"arguments\":\"{\\\"file_path\\\":\\\"/src/bar.rs\\\"}\"",
                "}}\n",
                "{\"timestamp\":\"2026-04-19T18:00:15Z\",\"type\":\"response_item\",\"payload\":{",
                "\"type\":\"message\",\"role\":\"assistant\",\"model\":\"gpt-4o\",\"content\":[{\"type\":\"text\",\"text\":\"done\"}]",
                "}}\n",
                "{\"timestamp\":\"2026-04-19T18:00:20Z\",\"type\":\"event_msg\",\"payload\":{",
                "\"type\":\"token_count\",\"info\":{\"total_token_usage\":{",
                "\"input_tokens\":100,\"output_tokens\":50,\"cached_input_tokens\":80,\"reasoning_tokens\":10",
                ",\"total_tokens\":240}}}}\n",
            ),
        )
        .unwrap();

        let adapter = CodexAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.model.as_deref(), Some("gpt-4o"));
        assert_eq!(parsed.session.git_branch.as_deref(), Some("feat/auth"));
        assert_eq!(parsed.session.input_tokens, 100);
        assert_eq!(parsed.session.output_tokens, 50);
        assert_eq!(parsed.session.cached_tokens, 80);
        assert_eq!(parsed.session.reasoning_tokens, 10);

        let paths: Vec<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert!(paths.contains(&"/src/foo.rs"));
        assert!(paths.contains(&"/src/bar.rs"));

        let ops: std::collections::HashSet<&str> = parsed
            .file_touches
            .iter()
            .map(|t| t.operation.as_str())
            .collect();
        assert!(ops.contains("read"));
        assert!(ops.contains("edit"));
    }
}
