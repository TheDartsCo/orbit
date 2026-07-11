export type AgentType =
  | "claude"
  | "codex"
  | "copilot"
  | "cursor"
  | "jetbrains"
  | "kilo"
  | "opencode"
  | "warp"
  | "qoder"
  | "antigravity"
  | "zcode";

export const ALL_AGENTS: AgentType[] = ["antigravity", "claude", "codex", "copilot", "cursor", "jetbrains", "kilo", "opencode", "qoder", "warp", "zcode"];

export type MessageRole = "user" | "assistant" | "system" | "tool";
export type AttachmentType = "image" | "file" | "diff";

export interface Session {
  id: string;
  parent_session_id: string | null;
  agent: AgentType;
  title: string;
  project_path: string;
  created_at: string;
  updated_at: string;
  file_path: string;
  is_active: boolean;
  message_count: number;
  model: string | null;
  git_branch: string | null;
  input_tokens: number;
  output_tokens: number;
  cached_tokens: number;
  reasoning_tokens: number;
  file_count: number;
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
  agents?: string[];
  title?: string;
  project_path?: string;
  model?: string;
  date_from?: string;
  date_to?: string;
  is_active?: boolean;
  query?: string;
  git_branch?: string;
}

export interface ProviderSyncStats {
  found: number;
  indexed: number;
  skipped: number;
  errored: number;
}

export interface IndexStats {
  sessions_found: number;
  sessions_indexed: number;
  sessions_skipped: number;
  sessions_errored: number;
  sessions_removed: number;
  provider_stats: Record<string, ProviderSyncStats>;
  last_sync_at: string | null;
}

export interface SyncStatus {
  last_sync_at: string | null;
  provider_stats: Record<string, ProviderSyncStats>;
}

export const AGENT_COLORS: Record<AgentType, string> = {
  claude: "bg-purple-500",
  codex: "bg-green-500",
  copilot: "bg-emerald-500",
  cursor: "bg-blue-500",
  jetbrains: "bg-pink-500",
  kilo: "bg-sky-500",
  opencode: "bg-orange-500",
  warp: "bg-teal-500",
  qoder: "bg-indigo-500",
  antigravity: "bg-violet-500",
  zcode: "bg-amber-500",
};

export const AGENT_CHART_COLORS: Record<AgentType, string> = {
  claude: "#fb923c",
  codex: "#60a5fa",
  copilot: "#34d399",
  cursor: "#22d3ee",
  jetbrains: "#f472b6",
  kilo: "#38bdf8",
  opencode: "#e879f9",
  warp: "#2dd4bf",
  qoder: "#818cf8",
  antigravity: "#a78bfa",
  zcode: "#fbbf24",
};

export const AGENT_TEXT_COLORS: Record<AgentType, string> = {
  claude: "text-orange-300",
  codex: "text-blue-400",
  copilot: "text-emerald-300",
  cursor: "text-cyan-300",
  jetbrains: "text-pink-300",
  kilo: "text-sky-300",
  opencode: "text-fuchsia-300",
  warp: "text-teal-300",
  qoder: "text-indigo-300",
  antigravity: "text-violet-300",
  zcode: "text-amber-300",
};

export const AGENT_TINTS: Record<AgentType, string> = {
  claude: "bg-orange-400/10 border-orange-400/20",
  codex: "bg-blue-400/10 border-blue-400/20",
  copilot: "bg-emerald-400/10 border-emerald-400/20",
  cursor: "bg-cyan-400/10 border-cyan-400/20",
  jetbrains: "bg-pink-400/10 border-pink-400/20",
  kilo: "bg-sky-400/10 border-sky-400/20",
  opencode: "bg-fuchsia-400/10 border-fuchsia-400/20",
  warp: "bg-teal-400/10 border-teal-400/20",
  qoder: "bg-indigo-400/10 border-indigo-400/20",
  antigravity: "bg-violet-400/10 border-violet-400/20",
  zcode: "bg-amber-400/10 border-amber-400/20",
};

export const AGENT_LABELS: Record<AgentType, string> = {
  claude: "Claude",
  codex: "Codex",
  copilot: "Copilot",
  cursor: "Cursor",
  jetbrains: "JetBrains AI",
  kilo: "Kilo Code",
  opencode: "OpenCode",
  warp: "Warp",
  qoder: "Qoder",
  antigravity: "Antigravity",
  zcode: "ZCode",
};

export interface TerminalInfo {
  id: string;
  name: string;
  available: boolean;
}

export type StatisticsMode = "agent" | "model" | "project";
export type StatisticsPeriod = "7d" | "30d" | "90d" | "all";

export interface StatisticsSummary {
  sessions: number;
  messages: number;
  total_tokens: number;
  active_agents: number;
  project_count: number;
  average_messages_per_session: number;
  average_tokens_per_session: number;
}

export interface StatisticsSeriesValue {
  key: string;
  label: string;
  value: number;
}

export interface StatisticsTimeBucket {
  start: string;
  values: StatisticsSeriesValue[];
}

export interface AgentStatisticsRow {
  agent: string;
  sessions: number;
  messages: number;
  tokens: number;
  average_messages: number;
  last_used: string;
}

export interface ModelStatisticsRow {
  model: string;
  sessions: number;
  messages: number;
  tokens: number;
  percentage: number;
  agent_count: number;
  top_agent: string;
  last_used: string;
  agent_mix: ProjectAgentShare[];
}

export interface ProjectAgentShare {
  agent: string;
  sessions: number;
  tokens: number;
  percentage: number;
}

export interface ProjectStatisticsRow {
  project: string;
  sessions: number;
  messages: number;
  tokens: number;
  agent_count: number;
  top_agent: string;
  last_active: string;
  agent_mix: ProjectAgentShare[];
}

export interface ProjectStatisticsCard {
  project: string;
  sessions: number;
  tokens: number;
  last_active: string;
  agent_mix: ProjectAgentShare[];
}

export type StatisticsDashboard =
  | {
      mode: "agent";
      summary: StatisticsSummary;
      timeline: StatisticsTimeBucket[];
      agents: AgentStatisticsRow[];
      models: ModelStatisticsRow[];
    }
  | {
      mode: "model";
      summary: StatisticsSummary;
      timeline: StatisticsTimeBucket[];
      models: ModelStatisticsRow[];
    }
  | {
      mode: "project";
      summary: StatisticsSummary;
      timeline: StatisticsTimeBucket[];
      projects: ProjectStatisticsRow[];
      cards: ProjectStatisticsCard[];
    };

export const TERMINAL_LABELS: Record<string, string> = {
  terminal: "Terminal",
  iterm: "iTerm2",
  warp: "Warp",
  ghostty: "Ghostty",
};
