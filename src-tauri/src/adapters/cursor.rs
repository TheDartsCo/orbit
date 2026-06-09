use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{AgentAdapter, PlatformPaths, SessionLocation};
use crate::models::*;

pub struct CursorAdapter;

impl CursorAdapter {
    pub fn new() -> Self {
        Self
    }

    fn data_dir_path_from_home(home: &Path) -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            Some(home.join(".cursor"))
        } else {
            None
        }
    }

    pub(crate) fn windows_data_dir(paths: &PlatformPaths) -> Option<PathBuf> {
        paths.home_join(".cursor")
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
}

fn find_session_jsonl(root: &Path, locations: &mut Vec<SessionLocation>) {
    let projects_dir = root.join("projects");
    let Ok(projects) = std::fs::read_dir(&projects_dir) else {
        return;
    };

    for project_entry in projects.flatten() {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        let project_name = project_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if project_name.starts_with('.') {
            continue;
        }

        let transcripts_dir = project_path.join("agent-transcripts");
        let Ok(transcripts) = std::fs::read_dir(&transcripts_dir) else {
            continue;
        };

        for session_entry in transcripts.flatten() {
            let session_dir = session_entry.path();
            if !session_dir.is_dir() {
                continue;
            }
            let session_name = session_dir
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if session_name.starts_with('.') {
                continue;
            }

            let Ok(session_files) = std::fs::read_dir(&session_dir) else {
                continue;
            };

            for session_file in session_files.flatten() {
                let jsonl_path = session_file.path();
                if !jsonl_path.is_file()
                    || jsonl_path.extension().and_then(|e| e.to_str()) != Some("jsonl")
                {
                    continue;
                }

                let modified = std::fs::metadata(&jsonl_path)
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
                    path: jsonl_path,
                    last_modified: modified,
                });
            }
        }
    }
}

fn extract_text_blocks(blocks: Option<&serde_json::Value>) -> String {
    let Some(arr) = blocks.and_then(|v| v.as_array()) else {
        return String::new();
    };
    let mut parts = Vec::new();
    for block in arr {
        if block.get("type").and_then(|t| t.as_str()) == Some("text") {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    parts.push(text.to_string());
                }
            }
        }
    }
    parts.join("\n")
}

const CURSOR_META_BLOCKS: &[&str] = &[
    "image_files",
    "timestamp",
    "attached_files",
    "uploaded_documents",
    "open_subagent_context",
];

fn extract_user_query_block(text: &str) -> Option<String> {
    let open = text.find("<user_query>")?;
    let after_open = open + "<user_query>".len();
    let close = text[after_open..].find("</user_query>")?;
    let inner = &text[after_open..after_open + close];
    let trimmed = inner.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn strip_block(text: &str, tag: &str) -> String {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        out.push_str(&rest[..start]);
        let after_open = start + open.len();
        match rest[after_open..].find(&close) {
            Some(end) => {
                rest = &rest[after_open + end + close.len()..];
            }
            None => {
                rest = &rest[after_open..];
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

fn strip_wrapper_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let close_idx = match rest[start..].find('>') {
            Some(i) => start + i + 1,
            None => {
                out.push_str(&rest[start..]);
                return out;
            }
        };
        rest = &rest[close_idx..];
    }
    out.push_str(rest);
    out
}

fn clean_user_text(raw: &str) -> String {
    let mut owned: String;
    let mut text: &str = raw.trim_start();
    if let Some(after) = text.strip_prefix("[Image]") {
        text = after.trim_start();
    }
    for tag in CURSOR_META_BLOCKS {
        owned = strip_block(text, tag);
        text = owned.as_str();
    }
    owned = strip_wrapper_tags(text);
    text = owned.as_str();
    text.trim().to_string()
}

fn cursor_file_operation_for(tool_name: &str) -> &'static str {
    match tool_name {
        "read_file" | "Read" | "read" => "read",
        "edit_file" | "Edit" | "MultiEdit" | "edit" => "edit",
        "write_file" | "Write" | "write" | "create_file" | "Create" => "write",
        "delete_file" | "delete" => "delete",
        _ => "unknown",
    }
}

fn cursor_extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    if let Some(s) = input.get("file_path").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = input.get("path").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = input.get("filePath").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(arr) = input.get("file_paths").and_then(|v| v.as_array()) {
        if let Some(s) = arr.first().and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }
    let _ = tool_name;
    None
}

fn encode_for_comparison(path: &str) -> String {
    path.strip_prefix('/')
        .unwrap_or(path)
        .replace('/', "-")
        .replace('.', "-")
        .replace(' ', "-")
}

fn infer_project_from_file_path(file_path: &str, encoded_project: &str) -> Option<String> {
    let path = std::path::Path::new(file_path);
    let mut candidate = path.to_path_buf();
    while candidate.pop() {
        let s = candidate.to_string_lossy();
        if encode_for_comparison(&s) == encoded_project {
            return Some(s.to_string());
        }
    }
    None
}

#[async_trait]
impl AgentAdapter for CursorAdapter {
    fn id(&self) -> &str {
        "cursor"
    }

    fn name(&self) -> &str {
        "Cursor"
    }

    async fn detect(&self) -> bool {
        Self::data_dir().is_some()
    }

    async fn scan(&self) -> Vec<SessionLocation> {
        let Some(data_dir) = Self::data_dir() else {
            return Vec::new();
        };

        let mut locations = Vec::new();
        find_session_jsonl(&data_dir, &mut locations);
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

        let encoded_project = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let mut project_path = String::new();
        let mut messages = Vec::new();
        let mut title = String::new();
        let mut seq: u32 = 0;
        let mut file_touches: Vec<FileTouch> = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let role = json.get("role").and_then(|r| r.as_str()).unwrap_or("");
            let content_blocks = json.get("message").and_then(|m| m.get("content"));

            match role {
                "user" => {
                    let raw_text = extract_text_blocks(content_blocks);
                    let text = clean_user_text(&raw_text);
                    if title.is_empty() {
                        let candidate =
                            extract_user_query_block(&raw_text).unwrap_or_else(|| text.clone());
                        if !candidate.trim().is_empty() {
                            title = candidate.chars().take(100).collect();
                        }
                    }
                    if !text.is_empty() {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: file_name.clone(),
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
                }
                "assistant" => {
                    let text = extract_text_blocks(content_blocks);
                    if !text.is_empty() {
                        messages.push(Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            session_id: file_name.clone(),
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

                    if let Some(arr) = content_blocks.and_then(|v| v.as_array()) {
                        for block in arr {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                                let name = block
                                    .get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let input_value = block
                                    .get("input")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                if let Some(fp) = cursor_extract_file_path(&name, &input_value) {
                                    if project_path.is_empty() {
                                        project_path =
                                            infer_project_from_file_path(&fp, encoded_project)
                                                .unwrap_or_else(|| encoded_project.to_string());
                                    }
                                    let op = cursor_file_operation_for(&name);
                                    file_touches.push(FileTouch {
                                        path: fp,
                                        operation: op.to_string(),
                                        sequence: seq,
                                    });
                                }
                                let input = Some(input_value.to_string());
                                messages.push(Message {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    session_id: file_name.clone(),
                                    role: MessageRole::Tool,
                                    content: String::new(),
                                    timestamp: None,
                                    sequence: seq,
                                    tool_name: Some(name),
                                    tool_input: input,
                                    tool_output: None,
                                });
                                seq += 1;
                            }
                        }
                    }
                }
                _ => continue,
            }
        }

        if title.is_empty() {
            title = format!(
                "Cursor Session ({})",
                &file_name.chars().take(8).collect::<String>()
            );
        }

        let session_mtime = std::fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                DateTime::from_timestamp(
                    t.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64,
                    0,
                )
            })
            .unwrap_or_else(Utc::now);

        if project_path.is_empty() {
            project_path = encoded_project.to_string();
        }

        let session = Session {
            id: file_name,
            parent_session_id: None,
            agent: AgentType::Cursor,
            title,
            project_path,
            created_at: session_mtime,
            updated_at: session_mtime,
            file_path: path.to_string_lossy().to_string(),
            is_active: false,
            message_count: messages.len() as u32,
            ..Default::default()
        };

        Ok(NormalizedSession {
            session,
            messages,
            attachments: Vec::new(),
            file_touches,
        })
    }

    fn resume_command(&self, _session_id: &str, project_path: &str) -> String {
        let safe = crate::shell_quote::shell_quote(project_path);
        format!("cursor {}", safe)
    }

    async fn is_active(&self, _session_path: &Path) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_session_file(dir: &Path, session_uuid: &str, content: &str) -> std::path::PathBuf {
        let session_dir = dir.join(session_uuid);
        fs::create_dir_all(&session_dir).unwrap();
        let path = session_dir.join(format!("{}.jsonl", session_uuid));
        fs::write(&path, content).unwrap();
        path
    }

    fn setup_cursor_layout(root: &Path) -> PathBuf {
        let projects = root.join("projects");
        fs::create_dir_all(&projects).unwrap();
        let project_a = projects.join("Users-maf-My-Files-My-apps-thedarts-co-zido-web");
        let transcripts = project_a.join("agent-transcripts");
        let project_b = projects.join("empty-window");
        let transcripts_b = project_b.join("agent-transcripts");
        fs::create_dir_all(&transcripts).unwrap();
        fs::create_dir_all(&transcripts_b).unwrap();
        transcripts
    }

    #[test]
    fn data_dir_path_from_home_uses_dot_cursor_on_unix() {
        let home = std::path::Path::new("/home/orbit-user");

        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            assert_eq!(
                CursorAdapter::data_dir_path_from_home(home),
                Some(home.join(".cursor"))
            );
        } else {
            assert!(CursorAdapter::data_dir_path_from_home(home).is_none());
        }
    }

    #[tokio::test]
    async fn scan_finds_jsonl_sessions_in_agent_transcripts() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        make_session_file(
            &transcripts,
            "11111111-1111-1111-1111-111111111111",
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}\n",
        );
        make_session_file(
            &transcripts,
            "22222222-2222-2222-2222-222222222222",
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"yo\"}]}}\n",
        );

        let subagent_dir = transcripts
            .join("22222222-2222-2222-2222-222222222222")
            .join("subagents");
        fs::create_dir_all(&subagent_dir).unwrap();
        fs::write(
            subagent_dir.join("sub.jsonl"),
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"sub\"}]}}\n",
        )
        .unwrap();

        let adapter = CursorAdapter::new();
        let locations = scan_with_root(&adapter, tmp.path()).await;

        assert_eq!(
            locations.len(),
            2,
            "expected 2 sessions, got {}",
            locations.len()
        );
        for loc in &locations {
            let path_str = loc.path.to_string_lossy();
            assert!(
                !path_str.contains("subagents"),
                "scan should skip subagents/ but found: {}",
                path_str
            );
        }
    }

    #[tokio::test]
    async fn scan_finds_direct_jsonl_with_different_basename_and_skips_nested_subagents() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());
        let session_dir = transcripts.join("33333333-3333-3333-3333-333333333333");
        fs::create_dir_all(&session_dir).unwrap();

        let direct_path = session_dir.join("conversation.jsonl");
        fs::write(
            &direct_path,
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"direct\"}]}}\n",
        )
        .unwrap();

        let subagent_dir = session_dir.join("subagents");
        fs::create_dir_all(&subagent_dir).unwrap();
        fs::write(
            subagent_dir.join("sub.jsonl"),
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"sub\"}]}}\n",
        )
        .unwrap();

        let adapter = CursorAdapter::new();
        let locations = scan_with_root(&adapter, tmp.path()).await;
        let paths: Vec<&Path> = locations.iter().map(|loc| loc.path.as_path()).collect();

        assert_eq!(paths, vec![direct_path.as_path()]);
    }

    #[tokio::test]
    async fn scan_ignores_non_transcript_files() {
        let tmp = TempDir::new().unwrap();
        let projects = tmp.path().join("projects");
        fs::create_dir_all(&projects).unwrap();
        let project_dir = projects.join("Users-test-project");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("notes.txt"), "noise").unwrap();
        fs::write(project_dir.join("state.db"), "not a session").unwrap();

        let adapter = CursorAdapter::new();
        let locations = scan_with_root(&adapter, tmp.path()).await;
        assert!(
            locations.is_empty(),
            "scan should not pick up non-agent-transcript files, got {}",
            locations.len()
        );
    }

    #[tokio::test]
    async fn parse_session_extracts_user_and_assistant_text() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let jsonl = "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"What is 2+2?\"}]}}\n\
                     {\"role\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"It is 4.\"}]}}\n";

        let path = make_session_file(&transcripts, "33333333-3333-3333-3333-333333333333", jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].role, MessageRole::User);
        assert_eq!(result.messages[0].content, "What is 2+2?");
        assert_eq!(result.messages[1].role, MessageRole::Assistant);
        assert_eq!(result.messages[1].content, "It is 4.");

        assert_eq!(result.session.title, "What is 2+2?");
        assert_eq!(
            result.session.project_path, "Users-maf-My-Files-My-apps-thedarts-co-zido-web",
            "project_path should be the encoded project dir name"
        );
        assert_eq!(result.session.message_count, 2);
    }

    #[tokio::test]
    async fn parse_session_extracts_tool_use_as_tool_message() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let jsonl = "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Read foo\"}]}}\n\
                     {\"role\":\"assistant\",\"message\":{\"content\":[\
                        {\"type\":\"text\",\"text\":\"Reading now.\"},\
                        {\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"path\":\"/tmp/foo\"}}\
                     ]}}\n";

        let path = make_session_file(&transcripts, "44444444-4444-4444-4444-444444444444", jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        assert_eq!(result.messages.len(), 3);
        assert_eq!(result.messages[1].role, MessageRole::Assistant);
        assert_eq!(result.messages[1].content, "Reading now.");
        assert_eq!(result.messages[2].role, MessageRole::Tool);
        assert_eq!(result.messages[2].tool_name.as_deref(), Some("Read"));
        let tool_input = result.messages[2].tool_input.as_deref().unwrap_or("");
        assert!(
            tool_input.contains("/tmp/foo"),
            "tool input should include path, got: {}",
            tool_input
        );
    }

    #[tokio::test]
    async fn parse_session_strips_user_query_wrapper_tags_from_title() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let jsonl = "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"<user_query>\\nWe need to implement onboarding steps.\\n</user_query>\"}]}}\n";

        let path = make_session_file(&transcripts, "55555555-5555-5555-5555-555555555555", jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        assert_eq!(
            result.session.title, "We need to implement onboarding steps.",
            "title should extract content from <user_query> block, not the wrapper tags"
        );
        let user_msg = result
            .messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .unwrap();
        assert!(
            !user_msg.content.contains("<user_query>"),
            "stored user content should not include <user_query> tags, got: {}",
            user_msg.content
        );
        assert!(user_msg
            .content
            .contains("We need to implement onboarding steps."));
    }

    #[tokio::test]
    async fn parse_session_picks_user_query_block_when_image_files_precedes_it() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let raw = "[Image]\\n<image_files>\\nThe following images were provided by the user...\\n</image_files>\\n<user_query>\\nsomething is off with product purchases.\\n</user_query>";
        let jsonl = format!(
            "{{\"role\":\"user\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}\n",
            raw.replace('\n', "\\n")
        );

        let path = make_session_file(&transcripts, "66666666-6666-6666-6666-666666666666", &jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        assert_eq!(
            result.session.title, "something is off with product purchases.",
            "title should jump to <user_query> block even when image_files precedes it"
        );
    }

    #[tokio::test]
    async fn parse_session_uses_file_mtime_for_session_dates() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let jsonl =
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"hi\"}]}}\n";
        let path = make_session_file(&transcripts, "88888888-8888-8888-8888-888888888888", jsonl);

        let now_secs = chrono::Utc::now().timestamp();
        let expected_mtime = std::fs::metadata(&path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        let updated_diff = (result.session.updated_at.timestamp() - expected_mtime).abs();
        let created_diff = (result.session.created_at.timestamp() - expected_mtime).abs();
        assert!(
            updated_diff <= 1,
            "updated_at should match file mtime (diff={}s)",
            updated_diff
        );
        assert!(
            created_diff <= 1,
            "created_at should match file mtime (diff={}s)",
            created_diff
        );

        let now_diff = (result.session.updated_at.timestamp() - now_secs).abs();
        assert!(
            now_diff <= 2,
            "for an old file mtime, session date should NOT be Utc::now() (now_diff={}s)",
            now_diff
        );
    }

    #[tokio::test]
    async fn parse_session_real_laravel_session_uses_file_mtime() {
        let path = std::path::PathBuf::from(
            "/Users/maf/.cursor/projects/Users-maf-My-Files-My-apps-thedarts-co-zido-zido-code-workspace/agent-transcripts/55b47dbd-eb60-4086-9d5c-0706e58bc77e/55b47dbd-eb60-4086-9d5c-0706e58bc77e.jsonl",
        );
        if !path.exists() {
            eprintln!(
                "skipping real-file test, file not present: {}",
                path.display()
            );
            return;
        }
        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        let file_mtime = std::fs::metadata(&path)
            .unwrap()
            .modified()
            .unwrap()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let file_mtime_dt = chrono::DateTime::from_timestamp(file_mtime, 0).unwrap();
        let now = chrono::Utc::now().timestamp();

        eprintln!(
            "DEBUG file_mtime: {}, now: {}, diff_days: {}",
            file_mtime_dt,
            now,
            (now - file_mtime) / 86400
        );

        let mtime_diff = (file_mtime - result.session.updated_at.timestamp()).abs();
        assert!(
            mtime_diff <= 1,
            "updated_at should match file mtime (not Utc::now()), mtime_diff={}s",
            mtime_diff
        );
        let now_diff = now - result.session.updated_at.timestamp();
        assert!(
            now_diff > 86400,
            "updated_at should be much older than now (proves mtime used, not now), now_diff={}s",
            now_diff
        );
    }

    #[tokio::test]
    async fn parse_session_real_empty_window_speakers_session() {
        let path = std::path::PathBuf::from(
            "/Users/maf/.cursor/projects/empty-window/agent-transcripts/7727bfe8-2504-4e16-ba00-1a825ff6dcd6/7727bfe8-2504-4e16-ba00-1a825ff6dcd6.jsonl",
        );
        if !path.exists() {
            eprintln!(
                "skipping real-file test, file not present: {}",
                path.display()
            );
            return;
        }
        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        eprintln!("DEBUG title: {:?}", result.session.title);

        assert!(
            !result.session.title.contains("<user_query>"),
            "title should not contain <user_query> tag, got: {:?}",
            result.session.title
        );
        assert!(
            result.session.title.contains("speakers"),
            "title should contain 'speakers', got: {:?}",
            result.session.title
        );
    }

    #[tokio::test]
    async fn parse_session_real_laravel_session_strips_user_query_from_title() {
        let path = std::path::PathBuf::from(
            "/Users/maf/.cursor/projects/Users-maf-My-Files-My-apps-thedarts-co-zido-zido-code-workspace/agent-transcripts/55b47dbd-eb60-4086-9d5c-0706e58bc77e/55b47dbd-eb60-4086-9d5c-0706e58bc77e.jsonl",
        );
        if !path.exists() {
            eprintln!(
                "skipping real-file test, file not present: {}",
                path.display()
            );
            return;
        }
        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        eprintln!("DEBUG title: {}", result.session.title);
        eprintln!("DEBUG title length: {}", result.session.title.len());

        assert!(
            !result.session.title.contains("<user_query>"),
            "title should not contain wrapper tags, got: {}",
            result.session.title
        );
        assert!(
            !result.session.title.contains("</user_query>"),
            "title should not contain closing tags, got: {}",
            result.session.title
        );
        assert!(
            result.session.title.to_lowercase().contains("laravel"),
            "title should reference the actual user query, got: {}",
            result.session.title
        );
    }

    #[tokio::test]
    async fn parse_session_real_image_session_picks_user_query_title() {
        let path = std::path::PathBuf::from(
            "/Users/maf/.cursor/projects/Users-maf-My-Files-My-apps-thedarts-co-zido-web/agent-transcripts/c337795f-a520-463a-bc6f-37123414a0fa/c337795f-a520-463a-bc6f-37123414a0fa.jsonl",
        );
        if !path.exists() {
            eprintln!(
                "skipping real-file test, file not present: {}",
                path.display()
            );
            return;
        }
        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        assert!(
            !result.session.title.contains("<user_query>"),
            "title should not contain wrapper tags, got: {}",
            result.session.title
        );
        assert!(
            result
                .session
                .title
                .to_lowercase()
                .contains("product purchases"),
            "title should reference the actual user query, got: {}",
            result.session.title
        );
    }

    #[tokio::test]
    async fn parse_session_strips_timestamp_and_image_files_blocks_from_user_content() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let raw = "<timestamp>Wednesday, Apr 29, 2026, 5:18 PM (UTC+4)</timestamp> <user_query>\\nIn a Laravel app...\\n</user_query>";
        let jsonl = format!(
            "{{\"role\":\"user\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{}\"}}]}}}}\n",
            raw.replace('\n', "\\n")
        );

        let path = make_session_file(&transcripts, "77777777-7777-7777-7777-777777777777", &jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        let user_msg = result
            .messages
            .iter()
            .find(|m| m.role == MessageRole::User)
            .unwrap();
        assert!(
            !user_msg.content.contains("<timestamp>"),
            "stored user content should not include timestamp tag, got: {}",
            user_msg.content
        );
        assert!(user_msg.content.contains("In a Laravel app"));
    }

    async fn scan_with_root(_adapter: &CursorAdapter, root: &Path) -> Vec<SessionLocation> {
        let mut locations = Vec::new();
        find_session_jsonl(root, &mut locations);
        locations
    }

    #[tokio::test]
    async fn extract_file_touches_from_tool_use_blocks() {
        let tmp = TempDir::new().unwrap();
        let transcripts = setup_cursor_layout(tmp.path());

        let jsonl = concat!(
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"edit auth\"}]}}\n",
            "{\"role\":\"assistant\",\"message\":{\"content\":[",
            "{\"type\":\"text\",\"text\":\"ok\"},",
            "{\"type\":\"tool_use\",\"name\":\"read_file\",\"input\":{\"file_path\":\"/src/foo.rs\"}},",
            "{\"type\":\"tool_use\",\"name\":\"edit_file\",\"input\":{\"file_path\":\"/src/bar.rs\"}}",
            "]}}\n",
        );

        let path = make_session_file(&transcripts, "88888888-8888-8888-8888-888888888888", jsonl);

        let adapter = CursorAdapter::new();
        let result = adapter.parse_session(&path).await.unwrap();

        let paths: Vec<&str> = result
            .file_touches
            .iter()
            .map(|t| t.path.as_str())
            .collect();
        assert!(
            paths.contains(&"/src/foo.rs"),
            "expected /src/foo.rs in {:?}",
            paths
        );
        assert!(
            paths.contains(&"/src/bar.rs"),
            "expected /src/bar.rs in {:?}",
            paths
        );
        assert_eq!(
            result.file_touches.len(),
            2,
            "expected 2 file_touches, got {:?}",
            result.file_touches
        );
    }
}
