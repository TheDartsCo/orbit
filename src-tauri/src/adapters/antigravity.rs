use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, SessionLocation};
use crate::models::*;

pub struct AntigravityAdapter;

impl Default for AntigravityAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AntigravityAdapter {
    pub fn new() -> Self {
        Self
    }

    fn data_dir() -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        let path = home.join(".gemini").join("antigravity").join("brain");
        if path.is_dir() {
            Some(path)
        } else {
            None
        }
    }
}

fn clean_user_message(content: &str) -> String {
    if let (Some(start), Some(end)) = (
        content.find("<USER_REQUEST>"),
        content.find("</USER_REQUEST>"),
    ) {
        content[start + "<USER_REQUEST>".len()..end]
            .trim()
            .to_string()
    } else {
        content.trim().to_string()
    }
}

fn clean_json_val(val: &serde_json::Value) -> Option<String> {
    let s = val.as_str()?;
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        Some(s[1..s.len() - 1].to_string())
    } else {
        Some(s.to_string())
    }
}

fn antigravity_file_operation_for(tool_name: &str) -> String {
    match tool_name {
        "view_file" => "read".to_string(),
        "replace_file_content" | "multi_replace_file_content" => "edit".to_string(),
        "write_to_file" => "write".to_string(),
        _ => "unknown".to_string(),
    }
}

fn antigravity_extract_file_path(
    _tool_name: &str,
    arguments: &serde_json::Value,
) -> Option<String> {
    for key in &["AbsolutePath", "TargetFile"] {
        if let Some(val) = arguments.get(*key) {
            if let Some(p) = clean_json_val(val) {
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn extract_project_path(arguments: &serde_json::Value) -> Option<String> {
    for key in &["Cwd", "DirectoryPath", "SearchPath"] {
        if let Some(val) = arguments.get(*key) {
            if let Some(p) = clean_json_val(val) {
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn estimate_tokens(text: &str) -> u64 {
    if text.is_empty() {
        0
    } else {
        (text.len() as u64 / 4).max(1)
    }
}

#[async_trait]
impl AgentAdapter for AntigravityAdapter {
    fn id(&self) -> &str {
        "antigravity"
    }

    fn name(&self) -> &str {
        "Antigravity"
    }

    async fn detect(&self) -> bool {
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let Some(brain_dir) = Self::data_dir() else {
            return Vec::new();
        };

        let mut locations = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&brain_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let transcript_path = path
                        .join(".system_generated")
                        .join("logs")
                        .join("transcript.jsonl");
                    if transcript_path.is_file() {
                        let modified = std::fs::metadata(&transcript_path)
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
                            path: transcript_path,
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
            .map_err(|e| format!("Failed to read transcript file: {}", e))?;

        let mut messages = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut title = String::new();
        let mut seq: u32 = 0;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut project_path = String::new();

        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut reasoning_tokens: u64 = 0;

        let session_id = path
            .parent() // logs
            .and_then(|p| p.parent()) // .system_generated
            .and_then(|p| p.parent()) // <session-id>
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

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
                .get("created_at")
                .and_then(|t| t.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc));

            if seq == 0 {
                created_at = timestamp.unwrap_or(Utc::now());
            }
            if timestamp.is_some() {
                updated_at = timestamp.unwrap_or(Utc::now());
            }

            match entry_type {
                "USER_INPUT" => {
                    let raw_content = json.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    input_tokens += estimate_tokens(raw_content);
                    let cleaned = clean_user_message(raw_content);

                    if title.is_empty() && !cleaned.is_empty() {
                        title = cleaned.chars().take(100).collect();
                    }

                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::User,
                        content: cleaned,
                        timestamp,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;
                }
                "PLANNER_RESPONSE" => {
                    let content_text = json
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let thinking_text = json
                        .get("thinking")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string();

                    output_tokens += estimate_tokens(&content_text);
                    if !thinking_text.is_empty() {
                        let r_tok = estimate_tokens(&thinking_text);
                        reasoning_tokens += r_tok;
                        output_tokens += r_tok;
                    }

                    let content = if !thinking_text.is_empty() {
                        format!("```thinking\n{}\n```\n\n{}", thinking_text, content_text)
                    } else {
                        content_text
                    };

                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::Assistant,
                        content,
                        timestamp,
                        sequence: seq,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    seq += 1;

                    if let Some(tool_calls) = json.get("tool_calls").and_then(|tc| tc.as_array()) {
                        for tc in tool_calls {
                            let name = tc
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("")
                                .to_string();
                            let args = tc.get("args").unwrap_or(&serde_json::Value::Null);

                            let tool_input = if args.is_null() {
                                None
                            } else {
                                let s = serde_json::to_string(args).ok();
                                if let Some(ref arg_str) = s {
                                    input_tokens += estimate_tokens(arg_str);
                                }
                                s
                            };

                            if project_path.is_empty() {
                                if let Some(p) = extract_project_path(args) {
                                    project_path = p;
                                }
                            }

                            if let Some(path) = antigravity_extract_file_path(&name, args) {
                                let op = antigravity_file_operation_for(&name);
                                file_touches.push(FileTouch {
                                    path,
                                    operation: op,
                                    sequence: seq,
                                });
                            }

                            messages.push(Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                session_id: session_id.clone(),
                                role: MessageRole::Tool,
                                content: String::new(),
                                timestamp,
                                sequence: seq,
                                tool_name: Some(name),
                                tool_input,
                                tool_output: None,
                            });
                            seq += 1;
                        }
                    }
                }
                "CONVERSATION_HISTORY" => {
                    // Ignore metadata entries
                }
                _ => {
                    let raw_content = json.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    input_tokens += estimate_tokens(raw_content);

                    let matched = messages.iter_mut().rev().find(|m| {
                        m.role == MessageRole::Tool
                            && m.tool_name.is_some()
                            && m.tool_output.is_none()
                    });

                    if let Some(msg) = matched {
                        msg.tool_output = Some(raw_content.to_string());
                    }
                }
            }
        }

        if title.is_empty() {
            title = format!(
                "Antigravity Session {}",
                &session_id.chars().take(8).collect::<String>()
            );
        }

        let session = Session {
            id: session_id,
            parent_session_id: None,
            agent: AgentType::Antigravity,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model: Some("Gemini 3.5 Flash".to_string()),
            git_branch: None,
            input_tokens,
            output_tokens,
            cached_tokens: 0,
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

    fn resume_command(&self, _session_id: &str, _project_path: &str) -> String {
        String::new()
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;

    #[tokio::test]
    async fn parses_antigravity_transcript_jsonl() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "7f5305ea-4b37-4ba5-9163-9f232757d0cd";
        let session_dir = temp.path().join(session_id);
        let logs_dir = session_dir.join(".system_generated").join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();

        let transcript_path = logs_dir.join("transcript.jsonl");
        std::fs::write(
            &transcript_path,
            concat!(
                "{\"step_index\":0,\"source\":\"USER_EXPLICIT\",\"type\":\"USER_INPUT\",\"status\":\"DONE\",\"created_at\":\"2026-06-08T09:38:37Z\",\"content\":\"<USER_REQUEST>\\nHello World\\n</USER_REQUEST>\"}\n",
                "{\"step_index\":1,\"source\":\"SYSTEM\",\"type\":\"CONVERSATION_HISTORY\",\"status\":\"DONE\",\"created_at\":\"2026-06-08T09:38:37Z\"}\n",
                "{\"step_index\":2,\"source\":\"MODEL\",\"type\":\"PLANNER_RESPONSE\",\"status\":\"DONE\",\"created_at\":\"2026-06-08T09:38:37Z\",\"content\":\"Starting work...\",\"thinking\":\"Plan: do it.\",\"tool_calls\":[{\"name\":\"list_dir\",\"args\":{\"DirectoryPath\":\"\\\"/tmp/project\\\"\"}}]}\n",
                "{\"step_index\":3,\"source\":\"MODEL\",\"type\":\"LIST_DIRECTORY\",\"status\":\"DONE\",\"created_at\":\"2026-06-08T09:38:39Z\",\"content\":\"[file1, file2]\"}\n"
            )
        ).unwrap();

        let adapter = AntigravityAdapter::new();
        let parsed = adapter.parse_session(&transcript_path).await.unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.agent, AgentType::Antigravity);
        assert_eq!(parsed.session.title, "Hello World");
        assert_eq!(parsed.session.project_path, "/tmp/project");
        assert_eq!(parsed.messages.len(), 3); // User, Assistant, Tool

        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Hello World");

        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert!(parsed.messages[1].content.contains("Plan: do it."));
        assert!(parsed.messages[1].content.contains("Starting work..."));

        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("list_dir"));
        assert_eq!(
            parsed.messages[2].tool_output.as_deref(),
            Some("[file1, file2]")
        );

        assert_eq!(parsed.session.input_tokens, 22); // 10 (user input) + 9 (tool arguments) + 3 (tool output)
        assert_eq!(parsed.session.output_tokens, 7); // 4 (content) + 3 (thinking)
        assert_eq!(parsed.session.reasoning_tokens, 3); // 3 (thinking)
    }
}
