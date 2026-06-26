use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

/// Adapter for ZCode (https://zcode.z.ai).
///
/// ZCode persists raw model I/O as JSONL at `~/.zcode/cli/rollout/model-io-<sessionId>.jsonl`.
/// Each line is one complete API call (`type: "model_io"`) using the Vercel AI SDK message
/// shape: `.request.messages` is a sibling of `.request.body` and uses a first-class `tool`
/// role plus a sibling `toolCalls[]` array on assistant turns (NOT Anthropic `tool_use`/
/// `tool_result` content blocks). Each line is a full conversation snapshot that grows until
/// the conversation exceeds 64 messages, after which only the tail is sent (`messagesKind:
/// "tail"`). We parse the freshest `main_turn` snapshot for the transcript.
pub struct ZCodeAdapter;

impl ZCodeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn data_dir_path_from_home(home: &Path) -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(home.join(".zcode").join("cli").join("rollout"))
        } else {
            None
        }
    }

    pub(crate) fn windows_data_dir(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".zcode").map(|p| p.join("cli").join("rollout"))
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

    /// `<ZCODE_STORAGE_DIR>/cli/rollout`, or the `~/.zcode` default. Mirrors how the ZCode
    /// CLI itself resolves its storage root.
    fn storage_root() -> Option<PathBuf> {
        if let Ok(custom) = std::env::var("ZCODE_STORAGE_DIR") {
            let trimmed = custom.trim().to_string();
            if !trimmed.is_empty() {
                return Some(PathBuf::from(trimmed));
            }
        }
        dirs::home_dir().map(|h| h.join(".zcode"))
    }
}

/// Extract visible text from a Vercel-AI-SDK message `content` value: either a plain string
/// or an array of `{type:"text", text}` / `{type:"reasoning", text}` blocks. Reasoning is
/// skipped (internal thinking) — only `text` blocks contribute.
fn extract_text(content: &serde_json::Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for item in arr {
            let item_type = item.get("type").and_then(|t| t.as_str());
            if item_type == Some("text") {
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

/// True if the message content is an injected harness `<system-reminder>…</system-reminder>`
/// block (skill lists, env context, todo nudges) rather than a real human prompt.
fn is_system_reminder(content: &str) -> bool {
    content.trim_start().starts_with("<system-reminder>")
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

/// Find `Primary working directory: <path>` inside a system-prompt text. Returns the trimmed
/// path (which may contain spaces). Scans the text rather than pulling a structured field —
/// ZCode embeds the cwd only inside prompt text.
fn extract_project_path(system_texts: &[&str]) -> String {
    const MARKER: &str = "Primary working directory: ";
    for text in system_texts {
        if let Some(idx) = text.find(MARKER) {
            let rest = &text[idx + MARKER.len()..];
            // The path runs to the end of the line — the system prompt never embeds it
            // mid-sentence, so the first newline terminates it.
            let line_end = rest.find('\n').unwrap_or(rest.len());
            let path = rest[..line_end].trim();
            if !path.is_empty() {
                return path.to_string();
            }
        }
    }
    String::new()
}

/// Parse the JSON `{"title":"..."}` payload that ZCode's `session_title` model call returns
/// in its `response.text`. Falls back to the raw text if it isn't JSON.
fn parse_title_response(text: &str) -> String {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(title) = parsed.get("title").and_then(|t| t.as_str()) {
            if !title.trim().is_empty() {
                return title.trim().to_string();
            }
        }
    }
    text.trim().to_string()
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
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        // Prefer the env-overridable storage root so custom installs are discovered, but fall
        // back to the validated default data dir (which already gates on `.is_dir()`).
        let dir = match Self::storage_root() {
            Some(root) if root.join("cli").join("rollout").is_dir() => {
                root.join("cli").join("rollout")
            }
            _ => match Self::data_dir() {
                Some(d) => d,
                None => return Vec::new(),
            },
        };

        let mut locations = Vec::new();
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => return locations,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if !name.starts_with("model-io-") {
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
        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;

        // First pass: collect JSON lines partitioned by querySource. `main_turn` are real
        // conversation turns; `session_title` is an auxiliary title-generation call.
        let mut main_turns: Vec<&str> = Vec::new();
        let mut title_from_title_call: Option<String> = None;

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let query_source = json.get("querySource").and_then(|q| q.as_str()).unwrap_or("");
            match query_source {
                "main_turn" => main_turns.push(line),
                "session_title" => {
                    if title_from_title_call.is_none() {
                        if let Some(text) = json
                            .get("response")
                            .and_then(|r| r.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            let parsed = parse_title_response(text);
                            if !parsed.is_empty() {
                                title_from_title_call = Some(parsed);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if main_turns.is_empty() {
            return Err("No main_turn entries, skipping".to_string());
        }

        // Aggregate session-level metadata across every main_turn line (tokens, timestamps,
        // model). The transcript is NOT built from a single line: each line carries only a
        // windowed slice `[messageOffset, messageOffset+len)` of the conversation. Once a
        // session exceeds 64 messages ZCode switches from `full` to `tail` snapshots and
        // starts dropping the head (including the first user message). We therefore merge
        // every line's window by global message index to reconstruct the full conversation.
        let mut session_id = String::new();
        let mut model: Option<String> = None;
        let mut created_at = Utc::now();
        let mut updated_at = Utc::now();
        let mut input_tokens: u64 = 0;
        let mut output_tokens: u64 = 0;
        let mut cached_tokens: u64 = 0;
        // Global-index → message object. Later lines overwrite earlier ones for the same index,
        // which is correct: the tail/refresh windows are fresher copies of the same messages.
        let mut merged: BTreeMap<u64, serde_json::Value> = BTreeMap::new();
        let mut last_line: Option<serde_json::Value> = None;
        let mut first_ts = true;

        for line in &main_turns {
            let json: serde_json::Value = serde_json::from_str(line).map_err(|e| e.to_string())?;

            if session_id.is_empty() {
                if let Some(id) = json.get("sessionId").and_then(|s| s.as_str()) {
                    session_id = id.to_string();
                }
            }
            if model.is_none() {
                if let Some(m) = json.get("model").and_then(|m| m.get("modelId")).and_then(|m| m.as_str()) {
                    model = Some(m.to_string());
                }
            }

            if let Some(started) = json
                .get("startedAt")
                .and_then(|s| s.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
            {
                if first_ts {
                    created_at = started;
                    first_ts = false;
                }
            }
            if let Some(completed) = json
                .get("completedAt")
                .and_then(|s| s.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&Utc))
            {
                updated_at = completed;
            }

            if let Some(usage) = json.get("response").and_then(|r| r.get("usage")) {
                if let Some(n) = usage.get("inputTokens").and_then(|v| v.as_u64()) {
                    input_tokens = input_tokens.saturating_add(n);
                }
                if let Some(n) = usage.get("outputTokens").and_then(|v| v.as_u64()) {
                    output_tokens = output_tokens.saturating_add(n);
                }
                if let Some(n) = usage.get("cacheReadTokens").and_then(|v| v.as_u64()) {
                    cached_tokens = cached_tokens.saturating_add(n);
                }
            }

            // Splice this line's windowed slice into the global map. Each message at
            // `messages[i]` corresponds to global index `messageOffset + i`.
            let offset = json
                .get("request")
                .and_then(|r| r.get("messageOffset"))
                .and_then(|o| o.as_u64())
                .unwrap_or(0);
            if let Some(msgs) = json
                .get("request")
                .and_then(|r| r.get("messages"))
                .and_then(|m| m.as_array())
            {
                for (i, msg) in msgs.iter().enumerate() {
                    merged.insert(offset + i as u64, msg.clone());
                }
            }

            last_line = Some(json);
        }

        if session_id.is_empty() {
            session_id = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .trim_start_matches("model-io-")
                .to_string();
        }

        // Collect system-prompt texts for cwd extraction (also reachable via
        // `.request.body.system[]`, but the leading system messages hold the same text).
        let mut system_texts: Vec<String> = Vec::new();

        let mut messages: Vec<Message> = Vec::new();
        let mut file_touches: Vec<FileTouch> = Vec::new();
        let mut title = title_from_title_call.unwrap_or_default();
        let mut seq: u32 = 0;

        // Emit the reconstructed conversation in global order.
        for msg in merged.values() {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            match role {
                "system" => {
                    let text = extract_text(msg.get("content").unwrap_or(&serde_json::Value::Null));
                    if !text.is_empty() {
                        system_texts.push(text);
                    }
                }
                "user" => {
                    let text = extract_text(msg.get("content").unwrap_or(&serde_json::Value::Null));
                    if text.trim().is_empty() || is_system_reminder(&text) {
                        continue;
                    }
                    if title.is_empty() {
                        title = text.chars().take(100).collect();
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
                    let text = extract_text(msg.get("content").unwrap_or(&serde_json::Value::Null));
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

                    // Assistant tool calls are a sibling `toolCalls[]` array; each becomes
                    // its own Tool message. Their results arrive later as `role:"tool"` msgs.
                    if let Some(tool_calls) = msg.get("toolCalls").and_then(|t| t.as_array()) {
                        for call in tool_calls {
                            let tool_name = call
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let input = call.get("input").cloned().unwrap_or(serde_json::Value::Null);
                            let tool_input = serde_json::to_string(&input).ok().filter(|s| s != "null");

                            if let Some(fp) = extract_file_path(&input) {
                                file_touches.push(FileTouch {
                                    path: fp,
                                    operation: file_operation_for(&tool_name),
                                    sequence: seq,
                                });
                            }

                            if title.is_empty() {
                                title = format!("[Tool: {}]", tool_name);
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
                                tool_output: None,
                            });
                            seq += 1;
                        }
                    }
                }
                "tool" => {
                    // Tool result: scalar string content linked back via toolCallId.
                    let output = msg
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tool_name = msg
                        .get("toolName")
                        .and_then(|n| n.as_str())
                        .map(ToString::to_string);
                    messages.push(Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_id.clone(),
                        role: MessageRole::Tool,
                        content: String::new(),
                        timestamp: None,
                        sequence: seq,
                        tool_name,
                        tool_input: None,
                        tool_output: Some(output),
                    });
                    seq += 1;
                }
                _ => {}
            }
        }

        // The freshest line's `response` is the model's answer for the final turn — it isn't
        // part of `.request.messages` yet (it becomes the next turn's input), so capture it as
        // a trailing assistant message when it's a real text reply (not a tool-call-only turn,
        // whose calls were already replayed from the assistant message in the merged map).
        if let Some(source) = last_line {
            let final_text = source
                .get("response")
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            let final_has_tool_calls = source
                .get("response")
                .and_then(|r| r.get("toolCalls"))
                .and_then(|t| t.as_array())
                .map_or(false, |a| !a.is_empty());
            if !final_text.trim().is_empty() && !final_has_tool_calls {
                messages.push(Message {
                    id: uuid::Uuid::new_v4().to_string(),
                    session_id: session_id.clone(),
                    role: MessageRole::Assistant,
                    content: final_text.to_string(),
                    timestamp: None,
                    sequence: seq,
                    tool_name: None,
                    tool_input: None,
                    tool_output: None,
                });
            }
        }

        if title.is_empty() {
            title = format!(
                "Session {}",
                session_id.chars().take(8).collect::<String>()
            );
        }

        let system_refs: Vec<&str> = system_texts.iter().map(|s| s.as_str()).collect();
        let project_path = extract_project_path(&system_refs);

        let session = Session {
            id: session_id,
            parent_session_id: None,
            agent: AgentType::Zcode,
            title,
            project_path,
            created_at,
            updated_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model,
            git_branch: None,
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


    fn supports_resume(&self) -> bool {
        false
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        let safe = crate::shell_quote::shell_quote(session_id);
        format!("zcode --resume {}", safe)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        // File stem is `model-io-sess_<uuid>`; the DB id is `sess_<uuid>`. They don't match,
        // so active-session reconciliation via file_stem can't work here — opt out (like codex).
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;

    #[test]
    fn data_dir_path_from_home_uses_dot_zcode_rollout_on_unix() {
        let home = std::path::Path::new("/home/orbit-user");

        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert_eq!(
                ZCodeAdapter::data_dir_path_from_home(home),
                Some(home.join(".zcode").join("cli").join("rollout"))
            );
        } else {
            assert!(ZCodeAdapter::data_dir_path_from_home(home).is_none());
        }
    }

    #[test]
    fn resume_command_uses_zcode_resume_flag() {
        let adapter = ZCodeAdapter::new();

        assert_eq!(
            adapter.resume_command("sess_abc-123", ""),
            "zcode --resume 'sess_abc-123'"
        );
    }

    #[test]
    fn does_not_support_resume() {
        let adapter = ZCodeAdapter::new();
        assert!(!adapter.supports_resume());
    }

    #[tokio::test]
    async fn parses_zcode_rollout_with_title_call_and_main_turn() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "sess_b7b1b5dd-45a7-4165-8b1a-3d507abbe295";
        let path = temp.path().join(format!("model-io-{}.jsonl", session_id));

        // session_title line — its response.text carries the generated title.
        let title_line = serde_json::json!({
            "type": "model_io",
            "querySource": "session_title",
            "model": {"modelId": "GLM-5.2", "role": "lite"},
            "sessionId": session_id,
            "startedAt": "2026-06-24T17:34:13.564Z",
            "completedAt": "2026-06-24T17:34:21.517Z",
            "response": {"text": "{\"title\":\"Add ZCode adapter support\"}", "toolCalls": []}
        });

        // main_turn line: system reminder skipped, real user prompt, assistant tool call,
        // its tool result, and a final text reply in response.text. System prompt embeds cwd.
        let main_line = serde_json::json!({
            "type": "model_io",
            "querySource": "main_turn",
            "model": {"modelId": "GLM-5.2", "role": "main"},
            "sessionId": session_id,
            "startedAt": "2026-06-24T17:34:13.566Z",
            "completedAt": "2026-06-24T17:38:28.620Z",
            "request": {
                "messagesKind": "full",
                "messages": [
                    {"role": "system", "content": "Primary working directory: /Users/maf/orbit"},
                    {"role": "user", "content": "<system-reminder>skills list</system-reminder>"},
                    {"role": "user", "content": "Add ZCode adapter support"},
                    {"role": "assistant", "content": [{"type": "text", "text": "Let me look."}], "toolCalls": [{"id": "call_1", "name": "Read", "input": {"file_path": "/src/main.rs"}}]},
                    {"role": "tool", "content": "file contents", "toolCallId": "call_1", "toolName": "Read"}
                ]
            },
            "response": {
                "text": "Done.",
                "toolCalls": [],
                "usage": {"inputTokens": 100, "outputTokens": 50, "cacheReadTokens": 80}
            }
        });

        std::fs::write(&path, format!("{}\n{}\n", title_line, main_line)).unwrap();

        let adapter = ZCodeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.agent, AgentType::Zcode);
        assert_eq!(parsed.session.title, "Add ZCode adapter support");
        assert_eq!(parsed.session.project_path, "/Users/maf/orbit");
        assert_eq!(parsed.session.model.as_deref(), Some("GLM-5.2"));
        assert_eq!(parsed.session.git_branch, None);
        assert_eq!(parsed.session.parent_session_id, None);
        assert!(!adapter.supports_resume());
        assert_eq!(parsed.session.input_tokens, 100);
        assert_eq!(parsed.session.output_tokens, 50);
        assert_eq!(parsed.session.cached_tokens, 80);
        assert_eq!(parsed.session.reasoning_tokens, 0);
        assert_eq!(
            parsed.session.created_at.to_rfc3339(),
            DateTime::parse_from_rfc3339("2026-06-24T17:34:13.566Z")
                .unwrap()
                .with_timezone(&Utc)
                .to_rfc3339()
        );

        // system skipped, first user (reminder) skipped, real user, assistant text, tool call,
        // tool result, final assistant reply.
        assert_eq!(parsed.messages.len(), 5);
        let roles: Vec<&MessageRole> = parsed.messages.iter().map(|m| &m.role).collect();
        assert_eq!(
            roles,
            [
                &MessageRole::User,
                &MessageRole::Assistant,
                &MessageRole::Tool,
                &MessageRole::Tool,
                &MessageRole::Assistant
            ]
        );
        assert_eq!(parsed.messages[0].content, "Add ZCode adapter support");
        assert_eq!(parsed.messages[1].content, "Let me look.");
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("Read"));
        assert!(parsed.messages[2].tool_input.as_deref().unwrap().contains("/src/main.rs"));
        assert_eq!(parsed.messages[3].tool_output.as_deref(), Some("file contents"));
        assert_eq!(parsed.messages[4].content, "Done.");

        let touches: Vec<&str> = parsed.file_touches.iter().map(|t| t.path.as_str()).collect();
        assert!(touches.contains(&"/src/main.rs"));
        let ops: std::collections::HashSet<&str> =
            parsed.file_touches.iter().map(|t| t.operation.as_str()).collect();
        assert!(ops.contains("read"));
    }

    #[tokio::test]
    async fn falls_back_to_first_user_message_when_no_title_call() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "sess_no-title";
        let path = temp.path().join(format!("model-io-{}.jsonl", session_id));

        let main_line = serde_json::json!({
            "type": "model_io",
            "querySource": "main_turn",
            "model": {"modelId": "GLM-5.2", "role": "main"},
            "sessionId": session_id,
            "startedAt": "2026-06-24T17:34:13.566Z",
            "completedAt": "2026-06-24T17:38:28.620Z",
            "request": {
                "messages": [
                    {"role": "user", "content": "Fix the bug in auth"}
                ]
            },
            "response": {"text": "", "toolCalls": []}
        });

        std::fs::write(&path, format!("{}\n", main_line)).unwrap();

        let adapter = ZCodeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        assert_eq!(parsed.session.title, "Fix the bug in auth");
    }

    #[tokio::test]
    async fn merges_tail_windows_so_first_user_message_is_not_lost() {
        // Regression: once a session exceeds 64 messages ZCode switches from `full` to `tail`
        // snapshots and drops the head of the conversation (including the first user message).
        // A "take the last line" parser would lose the first user message. We must merge all
        // windows by global index.
        let temp = tempfile::tempdir().unwrap();
        let session_id = "sess_long";
        let path = temp.path().join(format!("model-io-{}.jsonl", session_id));

        // Line 1: full snapshot covering [0, 4) — contains the original first user message.
        let line1 = serde_json::json!({
            "type": "model_io",
            "querySource": "main_turn",
            "model": {"modelId": "GLM-5.2", "role": "main"},
            "sessionId": session_id,
            "startedAt": "2026-06-24T17:34:13.566Z",
            "completedAt": "2026-06-24T17:35:00.000Z",
            "request": {
                "messagesKind": "full",
                "messageOffset": 0,
                "messageCount": 4,
                "messages": [
                    {"role": "user", "content": "Add ZCode adapter support"},
                    {"role": "assistant", "content": [{"type": "text", "text": "On it."}]},
                    {"role": "assistant", "content": [{"type": "text", "text": ""}], "toolCalls": [{"id": "call_1", "name": "Read", "input": {"file_path": "/a.rs"}}]},
                    {"role": "tool", "content": "contents", "toolCallId": "call_1", "toolName": "Read"}
                ]
            },
            "response": {"text": "", "toolCalls": []}
        });

        // Line 2: tail snapshot covering [1, 4) — messageOffset=1, drops index 0 (the first
        // user message). The response holds the freshest turn's text answer.
        let line2 = serde_json::json!({
            "type": "model_io",
            "querySource": "main_turn",
            "model": {"modelId": "GLM-5.2", "role": "main"},
            "sessionId": session_id,
            "startedAt": "2026-06-24T17:36:00.000Z",
            "completedAt": "2026-06-24T17:38:28.620Z",
            "request": {
                "messagesKind": "tail",
                "messageOffset": 1,
                "messageCount": 4,
                "messages": [
                    {"role": "assistant", "content": [{"type": "text", "text": "On it."}]},
                    {"role": "assistant", "content": [{"type": "text", "text": ""}], "toolCalls": [{"id": "call_1", "name": "Read", "input": {"file_path": "/a.rs"}}]},
                    {"role": "tool", "content": "contents", "toolCallId": "call_1", "toolName": "Read"}
                ]
            },
            "response": {"text": "Done.", "toolCalls": []}
        });

        std::fs::write(&path, format!("{}\n{}\n", line1, line2)).unwrap();

        let adapter = ZCodeAdapter::new();
        let parsed = adapter.parse_session(&path).await.unwrap();

        // The first user message MUST survive the merge.
        assert_eq!(parsed.messages.first().map(|m| m.content.as_str()), Some("Add ZCode adapter support"));
        assert_eq!(parsed.messages[0].role, MessageRole::User);

        // user, assistant text, tool call, tool result, final assistant reply.
        assert_eq!(parsed.messages.len(), 5);
        assert_eq!(parsed.messages[4].content, "Done.");

        // updated_at tracks the last line's completion; created_at tracks the first line's start.
        assert_eq!(
            parsed.session.updated_at.to_rfc3339(),
            DateTime::parse_from_rfc3339("2026-06-24T17:38:28.620Z")
                .unwrap()
                .with_timezone(&Utc)
                .to_rfc3339()
        );
    }
}

