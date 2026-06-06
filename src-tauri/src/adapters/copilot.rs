use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

pub struct CopilotAdapter;

impl Default for CopilotAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CopilotAdapter {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn windows_data_dir(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".copilot/session-state")
    }

    fn data_dir() -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            let home = dirs::home_dir()?;
            let path = home.join(".copilot").join("session-state");
            if path.exists() {
                Some(path)
            } else {
                None
            }
        } else if cfg!(target_os = "linux") {
            // To be implemented.
            None
        } else if cfg!(target_os = "windows") {
            Self::windows_data_dir(&PlatformPaths::system()).filter(|path| path.is_dir())
        } else {
            None
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

    fn scan_session_state_dir(dir: &Path, locations: &mut Vec<SessionLocation>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let events = path.join("events.jsonl");
                    if events.is_file() {
                        locations.push(SessionLocation {
                            last_modified: Self::modified_at(&events),
                            path: events,
                        });
                    }
                } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    locations.push(SessionLocation {
                        last_modified: Self::modified_at(&path),
                        path,
                    });
                }
            }
        }
    }

    fn parse_timestamp(event: &Value) -> Option<DateTime<Utc>> {
        event
            .get("timestamp")
            .and_then(|value| value.as_str())
            .or_else(|| {
                event
                    .get("data")
                    .and_then(|data| data.get("startTime"))
                    .and_then(|value| value.as_str())
            })
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|dt| dt.with_timezone(&Utc))
    }

    fn fallback_session_id(path: &Path) -> String {
        if path.file_name().and_then(|name| name.to_str()) == Some("events.jsonl") {
            if let Some(parent) = path.parent().and_then(|p| p.file_name()) {
                return parent.to_string_lossy().to_string();
            }
        }

        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    fn text_from_value(value: &Value) -> String {
        if let Some(text) = value.as_str() {
            return text.to_string();
        }

        if let Some(array) = value.as_array() {
            let parts: Vec<String> = array
                .iter()
                .map(Self::text_from_value)
                .filter(|part| !part.trim().is_empty())
                .collect();
            return parts.join("\n");
        }

        if let Some(text) = value.get("text").and_then(|value| value.as_str()) {
            return text.to_string();
        }

        if let Some(content) = value.get("content") {
            return Self::text_from_value(content);
        }

        String::new()
    }

    fn data_text(data: &Value) -> String {
        data.get("content")
            .map(Self::text_from_value)
            .filter(|content| !content.trim().is_empty())
            .or_else(|| {
                data.get("transformedContent")
                    .map(Self::text_from_value)
                    .filter(|content| !content.trim().is_empty())
            })
            .or_else(|| {
                data.get("message")
                    .map(Self::text_from_value)
                    .filter(|content| !content.trim().is_empty())
            })
            .unwrap_or_default()
    }

    fn compact_json(value: &Value) -> Option<String> {
        if value.is_null() {
            return None;
        }
        serde_json::to_string(value).ok()
    }

    fn result_output(data: &Value) -> Option<String> {
        let result = data.get("result").unwrap_or(data);
        let text = Self::text_from_value(result);
        if text.trim().is_empty() {
            Self::compact_json(result)
        } else {
            Some(text)
        }
    }

    fn tool_name_from(data: &Value) -> String {
        data.get("toolName")
            .or_else(|| data.get("name"))
            .and_then(|value| value.as_str())
            .unwrap_or("tool")
            .to_string()
    }

    fn tool_id_from(data: &Value) -> Option<String> {
        data.get("toolCallId")
            .or_else(|| data.get("id"))
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    }

    fn file_operation_for(tool_name: &str) -> String {
        match tool_name.to_ascii_lowercase().as_str() {
            "read_file" | "read" | "cat" => "read".to_string(),
            "edit_file" | "apply_patch" | "write_file" | "write" => "edit".to_string(),
            "bash" | "shell" | "terminal" => "unknown".to_string(),
            _ => "unknown".to_string(),
        }
    }

    fn extract_file_path(arguments: &Value) -> Option<String> {
        for key in ["file_path", "path"] {
            if let Some(path) = arguments.get(key).and_then(|value| value.as_str()) {
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
        None
    }

    fn push_tool_request(
        data: &Value,
        timestamp: Option<DateTime<Utc>>,
        session_id: &str,
        sequence: &mut u32,
        messages: &mut Vec<Message>,
        pending_tools: &mut HashMap<String, (String, Option<String>)>,
        seen_tool_requests: &mut HashSet<String>,
        file_touches: &mut Vec<FileTouch>,
    ) {
        let tool_name = Self::tool_name_from(data);
        let arguments = data.get("arguments").unwrap_or(&Value::Null);
        let tool_input = Self::compact_json(arguments);
        let tool_id = Self::tool_id_from(data);

        if let Some(id) = &tool_id {
            pending_tools.insert(id.clone(), (tool_name.clone(), tool_input.clone()));
            if !seen_tool_requests.insert(id.clone()) {
                return;
            }
        }

        if let Some(path) = Self::extract_file_path(arguments) {
            file_touches.push(FileTouch {
                path,
                operation: Self::file_operation_for(&tool_name),
                sequence: *sequence,
            });
        }

        messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: MessageRole::Tool,
            content: String::new(),
            timestamp,
            sequence: *sequence,
            tool_name: Some(tool_name),
            tool_input,
            tool_output: None,
        });
        *sequence += 1;
    }
}

#[async_trait]
impl AgentAdapter for CopilotAdapter {
    fn id(&self) -> &str {
        "copilot"
    }

    fn name(&self) -> &str {
        "Copilot"
    }

    async fn detect(&self) -> bool {
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let Some(data_dir) = Self::data_dir() else {
            return Vec::new();
        };

        let mut locations = Vec::new();
        Self::scan_session_state_dir(&data_dir, &mut locations);
        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;
        let mut session_id = Self::fallback_session_id(path);
        let mut messages = Vec::new();
        let mut file_touches = Vec::new();
        let mut title = String::new();
        let mut project_path = String::new();
        let mut model: Option<String> = None;
        let mut git_branch: Option<String> = None;
        let mut output_tokens = 0_u64;
        let mut sequence = 0_u32;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut saw_timestamp = false;
        let mut pending_tools: HashMap<String, (String, Option<String>)> = HashMap::new();
        let mut seen_tool_requests: HashSet<String> = HashSet::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let event_type = event
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let data = event.get("data").unwrap_or(&Value::Null);
            let timestamp = Self::parse_timestamp(&event);

            if let Some(ts) = timestamp {
                if !saw_timestamp {
                    created_at = ts;
                    saw_timestamp = true;
                }
                updated_at = ts;
            }

            match event_type {
                "session.start" => {
                    if let Some(id) = data.get("sessionId").and_then(|value| value.as_str()) {
                        if !id.is_empty() {
                            session_id = id.to_string();
                        }
                    }
                    if model.is_none() {
                        model = data
                            .get("copilotVersion")
                            .and_then(|value| value.as_str())
                            .map(ToString::to_string);
                    }
                    if let Some(context) = data.get("context") {
                        if let Some(cwd) = context.get("cwd").and_then(|value| value.as_str()) {
                            project_path = cwd.to_string();
                        } else if let Some(git_root) =
                            context.get("gitRoot").and_then(|value| value.as_str())
                        {
                            project_path = git_root.to_string();
                        }
                        git_branch = context
                            .get("branch")
                            .and_then(|value| value.as_str())
                            .map(ToString::to_string);
                    }
                }
                "session.model_change" => {
                    model = data
                        .get("model")
                        .or_else(|| data.get("to"))
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string);
                }
                "user.message" => {
                    let msg_content = Self::data_text(data);
                    if title.is_empty() && !msg_content.trim().is_empty() {
                        title = msg_content.chars().take(100).collect();
                    }
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::User,
                        content: msg_content,
                        timestamp,
                        sequence,
                        tool_name: None,
                        tool_input: None,
                        tool_output: None,
                    });
                    sequence += 1;
                }
                "assistant.message" => {
                    if let Some(tokens) = data.get("outputTokens").and_then(|value| value.as_u64())
                    {
                        output_tokens = output_tokens.saturating_add(tokens);
                    }

                    let msg_content = Self::data_text(data);
                    if !msg_content.trim().is_empty() {
                        messages.push(Message {
                            id: data
                                .get("messageId")
                                .and_then(|value| value.as_str())
                                .map(ToString::to_string)
                                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                            session_id: session_id.clone(),
                            role: MessageRole::Assistant,
                            content: msg_content,
                            timestamp,
                            sequence,
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                        });
                        sequence += 1;
                    }

                    if let Some(tool_requests) =
                        data.get("toolRequests").and_then(|value| value.as_array())
                    {
                        for request in tool_requests {
                            Self::push_tool_request(
                                request,
                                timestamp,
                                &session_id,
                                &mut sequence,
                                &mut messages,
                                &mut pending_tools,
                                &mut seen_tool_requests,
                                &mut file_touches,
                            );
                        }
                    }
                }
                "tool.execution_start" => {
                    Self::push_tool_request(
                        data,
                        timestamp,
                        &session_id,
                        &mut sequence,
                        &mut messages,
                        &mut pending_tools,
                        &mut seen_tool_requests,
                        &mut file_touches,
                    );
                }
                "tool.execution_complete" => {
                    let tool_id = Self::tool_id_from(data);
                    let (tool_name, tool_input) = tool_id
                        .as_ref()
                        .and_then(|id| pending_tools.get(id))
                        .cloned()
                        .unwrap_or_else(|| (Self::tool_name_from(data), None));
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::Tool,
                        content: String::new(),
                        timestamp,
                        sequence,
                        tool_name: Some(tool_name),
                        tool_input,
                        tool_output: Self::result_output(data),
                    });
                    sequence += 1;
                }
                "system.message" => {
                    let msg_content = Self::data_text(data);
                    if !msg_content.trim().is_empty() {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: session_id.clone(),
                            role: MessageRole::System,
                            content: msg_content,
                            timestamp,
                            sequence,
                            tool_name: None,
                            tool_input: None,
                            tool_output: None,
                        });
                        sequence += 1;
                    }
                }
                _ => {}
            }
        }

        if title.is_empty() {
            title = format!(
                "Session {}",
                &session_id.chars().take(8).collect::<String>()
            );
        }

        let session = Session {
            id: session_id,
            parent_session_id: None,
            agent: AgentType::Copilot,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            git_branch,
            input_tokens: 0,
            output_tokens,
            cached_tokens: 0,
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

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        let safe = crate::shell_quote::shell_quote(session_id);
        format!("copilot --resume={}", safe)
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
    async fn parses_current_copilot_events_jsonl_layout() {
        let temp = tempfile::tempdir().unwrap();
        let session_dir = temp.path().join("aaaabbbb-1111-2222-3333-ccccddddeeee");
        std::fs::create_dir_all(&session_dir).unwrap();
        let events = session_dir.join("events.jsonl");
        std::fs::write(
            &events,
            concat!(
                "{\"type\":\"session.start\",\"data\":{\"sessionId\":\"aaaabbbb-1111-2222-3333-ccccddddeeee\",",
                "\"copilotVersion\":\"1.0.11\",\"startTime\":\"2026-03-26T17:50:34.263Z\",",
                "\"context\":{\"cwd\":\"/tmp/project\",\"gitRoot\":\"/tmp/project\",\"branch\":\"main\"}},",
                "\"id\":\"e1\",\"timestamp\":\"2026-03-26T17:50:34.302Z\",\"parentId\":null}\n",
                "{\"type\":\"user.message\",\"data\":{\"content\":\"List the files\",\"interactionId\":\"int-0001\"},",
                "\"id\":\"e2\",\"timestamp\":\"2026-03-26T17:50:35.000Z\",\"parentId\":\"e1\"}\n",
                "{\"type\":\"assistant.message\",\"data\":{\"messageId\":\"msg-0001\",\"content\":\"\",",
                "\"toolRequests\":[{\"toolCallId\":\"call_1\",\"name\":\"bash\",\"arguments\":{\"command\":\"ls\"},\"type\":\"function\"}],",
                "\"interactionId\":\"int-0001\",\"outputTokens\":12},",
                "\"id\":\"e3\",\"timestamp\":\"2026-03-26T17:50:36.000Z\",\"parentId\":\"e2\"}\n",
                "{\"type\":\"tool.execution_complete\",\"data\":{\"toolCallId\":\"call_1\",\"success\":true,",
                "\"result\":{\"content\":\"file1\\nfile2\\n\"}},",
                "\"id\":\"e4\",\"timestamp\":\"2026-03-26T17:50:36.200Z\",\"parentId\":\"e3\"}\n",
                "{\"type\":\"assistant.message\",\"data\":{\"messageId\":\"msg-0002\",",
                "\"content\":\"The directory contains file1 and file2.\",\"toolRequests\":[],\"outputTokens\":14},",
                "\"id\":\"e5\",\"timestamp\":\"2026-03-26T17:50:37.100Z\",\"parentId\":\"e4\"}\n",
            ),
        )
        .unwrap();

        let parsed = CopilotAdapter::new().parse_session(&events).await.unwrap();

        assert_eq!(parsed.session.id, "aaaabbbb-1111-2222-3333-ccccddddeeee");
        assert_eq!(parsed.session.agent, AgentType::Copilot);
        assert_eq!(parsed.session.title, "List the files");
        assert_eq!(parsed.session.project_path, "/tmp/project");
        assert_eq!(parsed.session.model.as_deref(), Some("1.0.11"));
        assert_eq!(parsed.session.git_branch.as_deref(), Some("main"));
        assert_eq!(parsed.session.output_tokens, 26);
        assert_eq!(parsed.messages.len(), 4);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[1].role, MessageRole::Tool);
        assert_eq!(parsed.messages[1].tool_name.as_deref(), Some("bash"));
        assert_eq!(
            parsed.messages[1].tool_input.as_deref(),
            Some("{\"command\":\"ls\"}")
        );
        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(
            parsed.messages[2].tool_output.as_deref(),
            Some("file1\nfile2\n")
        );
        assert_eq!(parsed.messages[3].role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn parses_legacy_flat_copilot_jsonl_layout() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy-session.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"type\":\"session.start\",\"data\":{\"sessionId\":\"legacy-session\",",
                "\"context\":{\"cwd\":\"/tmp/legacy\"}},\"timestamp\":\"2026-01-01T00:00:00Z\"}\n",
                "{\"type\":\"session.model_change\",\"data\":{\"model\":\"gpt-5-copilot\"},",
                "\"timestamp\":\"2026-01-01T00:00:01Z\"}\n",
                "{\"type\":\"user.message\",\"data\":{\"content\":\"Explain this repo\"},",
                "\"timestamp\":\"2026-01-01T00:00:02Z\"}\n",
                "{\"type\":\"assistant.message\",\"data\":{\"content\":\"It is a desktop app.\",\"outputTokens\":5},",
                "\"timestamp\":\"2026-01-01T00:00:03Z\"}\n",
            ),
        )
        .unwrap();

        let parsed = CopilotAdapter::new().parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.id, "legacy-session");
        assert_eq!(parsed.session.project_path, "/tmp/legacy");
        assert_eq!(parsed.session.model.as_deref(), Some("gpt-5-copilot"));
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].content, "Explain this repo");
        assert_eq!(parsed.messages[1].content, "It is a desktop app.");
    }
}
