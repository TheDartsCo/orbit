export type AgentType = "claude" | "codex" | "cursor" | "opencode";
export type MessageRole = "user" | "assistant" | "system" | "tool";
export type AttachmentType = "image" | "file" | "diff";

export interface Session {
  id: string;
  agent: AgentType;
  title: string;
  project_path: string;
  created_at: string;
  updated_at: string;
  file_path: string;
  is_active: boolean;
  message_count: number;
}

export interface Message {
  id: string;
  session_id: string;
  role: MessageRole;
  content: string;
  timestamp: string | null;
  sequence: number;
  tool_name: string | null;
  tool_input: string | null;
  tool_output: string | null;
}

export interface Attachment {
  id: string;
  message_id: string;
  attachment_type: AttachmentType;
  path: string;
  mime_type: string | null;
}

export interface SessionFilters {
  agent?: string;
  project_path?: string;
  date_from?: string;
  date_to?: string;
  is_active?: boolean;
  query?: string;
}

export interface IndexStats {
  sessions_found: number;
  sessions_indexed: number;
  sessions_skipped: number;
  sessions_errored: number;
  sessions_removed: number;
}

export const AGENT_COLORS: Record<AgentType, string> = {
  claude: "bg-purple-500",
  codex: "bg-green-500",
  cursor: "bg-blue-500",
  opencode: "bg-orange-500",
};

export const AGENT_LABELS: Record<AgentType, string> = {
  claude: "Claude",
  codex: "Codex",
  cursor: "Cursor",
  opencode: "OpenCode",
};
