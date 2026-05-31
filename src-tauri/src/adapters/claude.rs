use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, SessionLocation};
use crate::models::*;

pub struct ClaudeAdapter;

impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn session_dirs(&self) -> Vec<PathBuf> {
        let home = dirs::home_dir().unwrap_or_default();
        let claude_dir = home.join(".claude").join("projects");
        if claude_dir.exists() {
            let mut dirs = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&claude_dir) {
                for entry in entries.flatten() {
                    let sessions_dir = entry.path().join("sessions");
                    if sessions_dir.is_dir() {
                        dirs.push(sessions_dir);
                    }
                }
            }
            dirs
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
        let home = dirs::home_dir().unwrap_or_default();
        home.join(".claude").exists()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let mut locations = Vec::new();
        for dir in self.session_dirs() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        let modified = std::fs::metadata(&path)
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| DateTime::from_timestamp(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64, 0))
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

        let project_path = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut messages = Vec::new();
        let mut title = String::from("Untitled Session");
        let mut seq: u32 = 0;
        let mut first_user_msg = true;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");

            match msg_type {
                "summary" => {
                    if let Some(sum) = json.get("summary").and_then(|s| s.as_str()) {
                        title = sum.to_string();
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
                    let content_text = json
                        .get("message")
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
                                        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                            block.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
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

                    let tool_calls: Vec<ToolCallInfo> = json
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                        .map(|blocks| {
                            blocks
                                .iter()
                                .filter_map(|block| {
                                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                        Some(ToolCallInfo {
                                            name: block.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
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

                    for tc in tool_calls {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: file_name.clone(),
                            role: MessageRole::Tool,
                            content: String::new(),
                            timestamp,
                            sequence: seq,
                            tool_name: Some(tc.name),
                            tool_input: tc.input,
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
            id: file_name,
            agent: AgentType::Claude,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
        })
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        format!("claude --resume {}", session_id)
    }

    async fn is_active(&self, session_path: &Path) -> bool {
        let home = dirs::home_dir().unwrap_or_default();
        let lock_path = home.join(".claude").join(format!(
            "{}.lock",
            session_path.file_stem().unwrap_or_default().to_string_lossy()
        ));
        lock_path.exists()
    }
}

struct ToolCallInfo {
    name: String,
    input: Option<String>,
}
