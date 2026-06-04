use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentType {
    Claude,
    Codex,
    Copilot,
    Cursor,
    OpenCode,
    Warp,
    Qoder,
}

impl AgentType {
    pub fn as_str(&self) -> &str {
        match self {
            AgentType::Claude => "claude",
            AgentType::Codex => "codex",
            AgentType::Copilot => "copilot",
            AgentType::Cursor => "cursor",
            AgentType::OpenCode => "opencode",
            AgentType::Warp => "warp",
            AgentType::Qoder => "qoder",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(AgentType::Claude),
            "codex" => Some(AgentType::Codex),
            "copilot" => Some(AgentType::Copilot),
            "cursor" => Some(AgentType::Cursor),
            "opencode" => Some(AgentType::OpenCode),
            "warp" => Some(AgentType::Warp),
            "qoder" => Some(AgentType::Qoder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl MessageRole {
    pub fn as_str(&self) -> &str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
            MessageRole::Tool => "tool",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "user" => Some(MessageRole::User),
            "assistant" => Some(MessageRole::Assistant),
            "system" => Some(MessageRole::System),
            "tool" => Some(MessageRole::Tool),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    Image,
    File,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub agent: AgentType,
    pub title: String,
    pub project_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub file_path: String,
    pub is_active: bool,
    pub message_count: u32,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub reasoning_tokens: u64,
    pub file_count: u32,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            id: String::new(),
            parent_session_id: None,
            agent: AgentType::Claude,
            title: String::new(),
            project_path: String::new(),
            created_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_else(Utc::now),
            updated_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap_or_else(Utc::now),
            file_path: String::new(),
            is_active: false,
            message_count: 0,
            model: None,
            git_branch: None,
            input_tokens: 0,
            output_tokens: 0,
            cached_tokens: 0,
            reasoning_tokens: 0,
            file_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub sequence: u32,
    pub tool_name: Option<String>,
    pub tool_input: Option<String>,
    pub tool_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub message_id: String,
    pub attachment_type: AttachmentType,
    pub path: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTouch {
    pub path: String,
    pub operation: String,
    pub sequence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSession {
    pub session: Session,
    pub messages: Vec<Message>,
    pub attachments: Vec<Attachment>,
    pub file_touches: Vec<FileTouch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFilters {
    pub agent: Option<String>,
    pub agents: Option<Vec<String>>,
    pub title: Option<String>,
    pub project_path: Option<String>,
    pub model: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub is_active: Option<bool>,
    pub query: Option<String>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
}
