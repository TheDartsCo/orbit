use async_trait::async_trait;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::{AgentType, FileTouch, Message, MessageRole, NormalizedSession, Session};

const AIA_HISTORY_DIR: &str = "Library/Application Support/JetBrains";

pub struct JetBrainsAdapter;

#[derive(Default)]
struct JetBrainsSessionFiles {
    agentsession: Option<PathBuf>,
    events: Option<PathBuf>,
    lastid: Option<PathBuf>,
    checkpoints: Option<PathBuf>,
}

impl Default for JetBrainsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl JetBrainsAdapter {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn windows_candidate_data_dirs(paths: &PlatformPaths) -> Vec<PathBuf> {
        [
            paths.data_join("JetBrains"),
            paths.data_local_join("JetBrains"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn has_history_dir(path: &Path) -> bool {
        std::fs::read_dir(path)
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| entry.path().join("aia-task-history").is_dir())
    }

    pub(crate) fn windows_data_dir(paths: &PlatformPaths) -> Option<PathBuf> {
        Self::windows_candidate_data_dirs(paths)
            .into_iter()
            .find(|path| Self::has_history_dir(path))
    }

    fn data_dir() -> Option<PathBuf> {
        if cfg!(target_os = "macos") {
            let home = dirs::home_dir()?;
            let path = home.join(AIA_HISTORY_DIR);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        } else if cfg!(target_os = "linux") {
            // To be implemented.
            None
        } else if cfg!(target_os = "windows") {
            Self::windows_data_dir(&PlatformPaths::system())
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

    fn newest_modified(files: &JetBrainsSessionFiles) -> DateTime<Utc> {
        [
            files.agentsession.as_deref(),
            files.events.as_deref(),
            files.lastid.as_deref(),
            files.checkpoints.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(Self::modified_at)
        .max()
        .unwrap_or_default()
    }

    fn scan_history_dir(history_dir: &Path, locations: &mut Vec<SessionLocation>) {
        let mut grouped: HashMap<String, JetBrainsSessionFiles> = HashMap::new();

        let Ok(entries) = std::fs::read_dir(history_dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };

            let files = grouped.entry(stem.to_string()).or_default();
            match ext {
                "agentsession" => files.agentsession = Some(path),
                "events" => files.events = Some(path),
                "lastid" => files.lastid = Some(path),
                "checkpoints" => files.checkpoints = Some(path),
                _ => {}
            }
        }

        for files in grouped.values() {
            if let Some(events) = &files.events {
                locations.push(SessionLocation {
                    path: events.clone(),
                    last_modified: Self::newest_modified(files),
                });
            }
        }
    }

    fn agentsession_path(events_path: &Path) -> PathBuf {
        events_path.with_extension("agentsession")
    }

    fn parse_agentsession_content(content: &str) -> (Option<String>, Option<String>) {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return (None, None);
        }

        trimmed
            .split_once(':')
            .map(|(agent_id, provider_session_id)| {
                (
                    Some(agent_id.to_string()),
                    Some(provider_session_id.to_string()),
                )
            })
            .unwrap_or_else(|| (Some(trimmed.to_string()), None))
    }

    fn session_id_from_path(path: &Path) -> String {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }

    fn read_agent_id(events_path: &Path) -> Option<String> {
        let content = std::fs::read_to_string(Self::agentsession_path(events_path)).ok()?;
        let (agent_id, _) = Self::parse_agentsession_content(&content);
        agent_id
    }

    fn decode_event(line: &str) -> Option<Value> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(line.trim())
            .ok()?;
        serde_json::from_slice::<Value>(&bytes).ok()
    }

    fn event_type(value: &Value) -> &str {
        value
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    }

    fn nested_event(value: &Value) -> &Value {
        value.get("event").unwrap_or(&Value::Null)
    }

    fn event_kind(value: &Value) -> &str {
        Self::nested_event(value)
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("")
    }

    fn event_text(value: &Value) -> String {
        let event = Self::nested_event(value);
        for key in ["text", "details", "result"] {
            if let Some(text) = event.get(key).and_then(|value| value.as_str()) {
                if !text.trim().is_empty() {
                    return text.to_string();
                }
            }
        }
        String::new()
    }

    fn tool_name(value: &Value) -> String {
        let event = Self::nested_event(value);
        event
            .get("toolType")
            .or_else(|| event.get("text"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| {
                Self::event_kind(value)
                    .rsplit('.')
                    .next()
                    .unwrap_or("tool")
                    .to_string()
            })
    }

    fn tool_output(value: &Value) -> Option<String> {
        let event = Self::nested_event(value);
        event
            .get("details")
            .or_else(|| event.get("result"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }

    fn is_tool_block(kind: &str) -> bool {
        kind.contains("ToolBlockUpdatedEvent") || kind.contains("ViewFilesBlockUpdatedEvent")
    }

    fn path_from_file_url(url: &str) -> Option<String> {
        let raw_path = url.strip_prefix("file://")?;
        let path = Path::new(raw_path);

        for marker in ["app", "src", "tests", "test"] {
            if let Some(root) = Self::ancestor_before_component(path, marker) {
                return Some(root);
            }
        }

        path.parent()
            .map(|parent| parent.to_string_lossy().to_string())
    }

    fn ancestor_before_component(path: &Path, marker: &str) -> Option<String> {
        let mut current = PathBuf::new();
        for component in path.components() {
            let component_text = component.as_os_str().to_string_lossy();
            if component_text == marker {
                let root = current.to_string_lossy().to_string();
                if !root.is_empty() {
                    return Some(root);
                }
            }
            current.push(component.as_os_str());
        }
        None
    }

    fn project_path_from_user_event(value: &Value) -> Option<String> {
        value
            .get("attachments")
            .and_then(|value| value.as_array())
            .and_then(|attachments| {
                attachments.iter().find_map(|attachment| {
                    attachment
                        .get("url")
                        .and_then(|value| value.as_str())
                        .and_then(Self::path_from_file_url)
                })
            })
    }

    fn agent_id_from_event(value: &Value) -> Option<String> {
        value
            .get("agentId")
            .and_then(|agent_id| agent_id.get("id"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    }

    fn push_message(
        messages: &mut Vec<Message>,
        session_id: &str,
        role: MessageRole,
        content: String,
        sequence: &mut u32,
        tool_name: Option<String>,
        tool_output: Option<String>,
    ) {
        if content.trim().is_empty() && tool_output.as_deref().unwrap_or("").trim().is_empty() {
            return;
        }

        messages.push(Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role,
            content,
            timestamp: None,
            sequence: *sequence,
            tool_name,
            tool_input: None,
            tool_output,
        });
        *sequence += 1;
    }
}

#[async_trait]
impl AgentAdapter for JetBrainsAdapter {
    fn id(&self) -> &str {
        "jetbrains"
    }

    fn name(&self) -> &str {
        "JetBrains AI"
    }

    async fn detect(&self) -> bool {
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let Some(data_dir) = Self::data_dir() else {
            return Vec::new();
        };

        let mut locations = Vec::new();
        let Ok(products) = std::fs::read_dir(data_dir) else {
            return locations;
        };

        for product in products.flatten() {
            let history_dir = product.path().join("aia-task-history");
            if history_dir.is_dir() {
                Self::scan_history_dir(&history_dir, &mut locations);
            }
        }

        locations
    }

    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession, String> {
        if path.extension().and_then(|value| value.to_str()) == Some("agentsession") {
            return Err("Metadata-only JetBrains session, skipping".to_string());
        }

        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read: {}", e))?;
        let session_id = Self::session_id_from_path(path);
        let mut title = String::new();
        let mut project_path = String::new();
        let mut messages = Vec::new();
        let mut sequence = 0_u32;
        let mut agent_id = Self::read_agent_id(path);
        let modified_at = Self::modified_at(path);

        for line in content.lines().skip(1) {
            if line.trim().is_empty() {
                continue;
            }

            let Some(event) = Self::decode_event(line) else {
                continue;
            };

            let event_type = Self::event_type(&event);
            if agent_id.is_none() {
                agent_id = Self::agent_id_from_event(&event);
            }

            if event_type.ends_with("ChatSessionUserPromptEvent") {
                let prompt = event
                    .get("prompt")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string();

                if title.is_empty() && !prompt.trim().is_empty() {
                    title = prompt.chars().take(100).collect();
                }
                if project_path.is_empty() {
                    if let Some(path) = Self::project_path_from_user_event(&event) {
                        project_path = path;
                    }
                }

                Self::push_message(
                    &mut messages,
                    &session_id,
                    MessageRole::User,
                    prompt,
                    &mut sequence,
                    None,
                    None,
                );
            } else if event_type.ends_with("ChatSessionMessageBlockEvent") {
                let kind = Self::event_kind(&event);
                if Self::is_tool_block(kind) {
                    Self::push_message(
                        &mut messages,
                        &session_id,
                        MessageRole::Tool,
                        String::new(),
                        &mut sequence,
                        Some(Self::tool_name(&event)),
                        Self::tool_output(&event),
                    );
                } else {
                    Self::push_message(
                        &mut messages,
                        &session_id,
                        MessageRole::Assistant,
                        Self::event_text(&event),
                        &mut sequence,
                        None,
                        None,
                    );
                }
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
            agent: AgentType::JetBrains,
            title,
            project_path,
            created_at: modified_at,
            updated_at: modified_at,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            model: agent_id,
            git_branch: None,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: 0,
            reasoning_tokens: 0,
            file_count: 0,
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches: Vec::<FileTouch>::new(),
        })
    }

    fn supports_resume(&self) -> bool {
        false
    }

    fn resume_command(&self, session_id: &str, _project_path: &str) -> String {
        format!(
            "jetbrains-ai session {}",
            crate::shell_quote::shell_quote(session_id)
        )
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AgentAdapter;
    use crate::models::{AgentType, MessageRole};

    fn encode_event(json: &str) -> String {
        use base64::Engine;

        base64::engine::general_purpose::STANDARD.encode(json)
    }

    #[tokio::test]
    async fn parses_grouped_jetbrains_ai_session_events() {
        let temp = tempfile::tempdir().unwrap();
        let history_dir = temp
            .path()
            .join("Library")
            .join("Application Support")
            .join("JetBrains")
            .join("PhpStorm2026.1")
            .join("aia-task-history");
        std::fs::create_dir_all(&history_dir).unwrap();

        let session_id = "7dd0b58b-e19a-467e-8c4b-591dc35b2f2d";
        let agentsession = history_dir.join(format!("{}.agentsession", session_id));
        let events = history_dir.join(format!("{}.events", session_id));
        let lastid = history_dir.join(format!("{}.lastid", session_id));
        let checkpoints = history_dir.join(format!("{}.checkpoints", session_id));

        std::fs::write(
            &agentsession,
            "acp.registry.junie:108400aa-5329-47a8-bee1-4526086335d0",
        )
        .unwrap();
        std::fs::write(&lastid, "4").unwrap();
        std::fs::write(&checkpoints, "").unwrap();
        std::fs::write(
            &events,
            format!(
                "AUI_EVENTS_V1\n{}\n{}\n{}\n{}\n",
                encode_event(
                    r#"{"type":"com.intellij.ml.llm.chat.shared.ChatSessionUserPromptEvent","id":{"id":1},"prompt":"Why is the admin menu missing?","attachments":[{"url":"file:///tmp/project/app/Admin.php"}],"agentId":{"id":"acp.registry.junie"}}"#
                ),
                encode_event(
                    r#"{"type":"com.intellij.ml.llm.chat.shared.ChatSessionMessageBlockEvent","id":{"id":2},"agentId":{"id":"acp.registry.junie"},"event":{"kind":"com.intellij.ml.llm.aui.events.api.AgentThoughtBlockUpdatedEvent","stepId":"step-1","text":"I will inspect the navigation setup."}}"#
                ),
                encode_event(
                    r#"{"type":"com.intellij.ml.llm.chat.shared.ChatSessionMessageBlockEvent","id":{"id":3},"agentId":{"id":"acp.registry.junie"},"event":{"kind":"com.intellij.ml.llm.aui.events.api.ToolBlockUpdatedEvent","stepId":"tool-1","text":"Search","status":"COMPLETED","details":"Found AdminPanelProvider.php","toolType":"Search"}}"#
                ),
                encode_event(
                    r#"{"type":"com.intellij.ml.llm.chat.shared.ChatSessionMessageBlockEvent","id":{"id":4},"agentId":{"id":"acp.registry.junie"},"event":{"kind":"com.intellij.ml.llm.aui.events.api.MarkdownBlockUpdatedEvent","stepId":"step-2","text":"The resource is absent from the custom navigation list."}}"#
                ),
            ),
        )
        .unwrap();

        let parsed = JetBrainsAdapter::new()
            .parse_session(&events)
            .await
            .unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.agent, AgentType::JetBrains);
        assert_eq!(parsed.session.title, "Why is the admin menu missing?");
        assert_eq!(parsed.session.model.as_deref(), Some("acp.registry.junie"));
        assert_eq!(parsed.session.project_path, "/tmp/project");
        assert_eq!(parsed.messages.len(), 4);
        assert_eq!(parsed.messages[0].role, MessageRole::User);
        assert_eq!(parsed.messages[0].content, "Why is the admin menu missing?");
        assert_eq!(parsed.messages[1].role, MessageRole::Assistant);
        assert_eq!(parsed.messages[2].role, MessageRole::Tool);
        assert_eq!(parsed.messages[2].tool_name.as_deref(), Some("Search"));
        assert_eq!(
            parsed.messages[2].tool_output.as_deref(),
            Some("Found AdminPanelProvider.php")
        );
        assert_eq!(parsed.messages[3].role, MessageRole::Assistant);
    }

    #[test]
    fn scan_history_dir_includes_only_event_backed_groups() {
        let temp = tempfile::tempdir().unwrap();
        let history_dir = temp.path().join("aia-task-history");
        std::fs::create_dir_all(&history_dir).unwrap();

        for ext in ["events", "agentsession", "lastid", "checkpoints"] {
            std::fs::write(
                history_dir.join(format!("11111111-1111-1111-1111-111111111111.{}", ext)),
                "",
            )
            .unwrap();
        }
        std::fs::write(
            history_dir.join("22222222-2222-2222-2222-222222222222.events"),
            "",
        )
        .unwrap();

        let mut locations = Vec::new();
        JetBrainsAdapter::scan_history_dir(&history_dir, &mut locations);
        locations.sort_by(|a, b| a.path.cmp(&b.path));

        let names: Vec<_> = locations
            .iter()
            .filter_map(|location| location.path.file_name().and_then(|name| name.to_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                "11111111-1111-1111-1111-111111111111.events",
                "22222222-2222-2222-2222-222222222222.events",
            ]
        );
    }

    #[tokio::test]
    async fn parses_agent_id_from_events_when_agentsession_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "08650765-e699-466d-8533-a5bb86527e33";
        let events = temp.path().join(format!("{}.events", session_id));
        std::fs::write(
            &events,
            format!(
                "AUI_EVENTS_V1\n{}\n",
                encode_event(
                    r#"{"type":"com.intellij.ml.llm.chat.shared.ChatSessionUserPromptEvent","id":{"id":1},"prompt":"Apply the TODO","agentId":{"id":"junie"}}"#
                ),
            ),
        )
        .unwrap();

        let parsed = JetBrainsAdapter::new()
            .parse_session(&events)
            .await
            .unwrap();

        assert_eq!(parsed.session.id, session_id);
        assert_eq!(parsed.session.model.as_deref(), Some("junie"));
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].content, "Apply the TODO");
    }

    #[tokio::test]
    async fn rejects_agentsession_only_metadata_session() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "d16e6387-d0e4-4218-b2c6-4a3bf99b52b1";
        let path = temp.path().join(format!("{}.agentsession", session_id));
        std::fs::write(&path, "acp.registry.junie:session-260605-142954-2shc").unwrap();

        let err = JetBrainsAdapter::new()
            .parse_session(&path)
            .await
            .unwrap_err();

        assert!(err.contains("skipping"));
    }

    #[test]
    fn jetbrains_sessions_do_not_support_resume() {
        assert!(!JetBrainsAdapter::new().supports_resume());
    }
}
