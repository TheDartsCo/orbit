import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { ALL_AGENTS, type Session, type Message, type SessionFilters, type IndexStats, type SyncStatus, type MessageRole, type AgentType, type TerminalInfo } from "../types";
import { createDefaultEnabledRoles } from "./messageRoles";

export type SortColumn = "agent" | "session" | "date" | "project" | "model" | "branch" | "tokens" | "files" | "messages";
export type SortDirection = "asc" | "desc";

export interface SortConfig {
  column: SortColumn;
  direction: SortDirection;
}

interface AppState {
  sessions: Session[];
  selectedSessionId: string | null;
  messages: Message[];
  filters: SessionFilters;
  loading: boolean;
  initialLoading: boolean;
  messagesLoading: boolean;
  activeSessionIds: Set<string>;
  indexError: string | null;
  indexStats: IndexStats | null;
  lastSyncAt: string | null;
  enabledRoles: Set<MessageRole>;
  enabledAgents: Set<AgentType>;
  sortConfig: SortConfig | null;
  availableTerminals: TerminalInfo[];
  preferredTerminal: string | null;
  settingsOpen: boolean;

  loadSessions: () => Promise<void>;
  selectSession: (id: string) => Promise<void>;
  setFilters: (filters: Partial<SessionFilters>) => void;
  search: (query: string) => Promise<void>;
  loadMoreMessages: () => Promise<void>;
  refreshActiveSessions: () => Promise<void>;
  reindex: () => Promise<void>;
  loadSyncStatus: () => Promise<void>;
  toggleRole: (role: MessageRole) => void;
  toggleAgent: (agent: AgentType) => void;
  setEnabledAgents: (agents: AgentType[]) => void;
  setSort: (column: SortColumn) => void;
  setGitBranchFilter: (branch: string | null) => void;
  loadSettings: () => Promise<void>;
  setPreferredTerminal: (id: string) => Promise<void>;
  openSettings: () => void;
  closeSettings: () => void;
}

export const useAppStore = create<AppState>((set, get) => ({
  sessions: [],
  selectedSessionId: null,
  messages: [],
  filters: {},
  loading: false,
  initialLoading: true,
  messagesLoading: false,
  activeSessionIds: new Set<string>(),
  indexError: null,
  indexStats: null,
  lastSyncAt: null,
  enabledRoles: createDefaultEnabledRoles(),
  enabledAgents: new Set<AgentType>(ALL_AGENTS),
  sortConfig: null,
  availableTerminals: [],
  preferredTerminal: null,
  settingsOpen: false,

  loadSessions: async () => {
    set({ loading: true });
    try {
      const { enabledAgents } = get();
      const agentsFilter = enabledAgents.size === ALL_AGENTS.length
        ? undefined
        : Array.from(enabledAgents);
      const filters = { ...get().filters, agents: agentsFilter };
      const sessions = await invoke<Session[]>("get_sessions", {
        filters,
        offset: 0,
        limit: 200,
      });
      set({ sessions, loading: false, initialLoading: false });
    } catch (e) {
      console.error("Failed to load sessions:", e);
      set({ sessions: [], loading: false, initialLoading: false });
    }
  },

  selectSession: async (id: string) => {
    set({ selectedSessionId: id, messages: [], loading: true, messagesLoading: true, enabledRoles: createDefaultEnabledRoles() });
    try {
      const messages = await invoke<Message[]>("get_session_messages", {
        sessionId: id,
        offset: 0,
        limit: 500,
      });
      set({ messages, loading: false, messagesLoading: false });
    } catch (e) {
      console.error("Failed to load messages:", e);
      set({ loading: false, messagesLoading: false });
    }
  },

  setFilters: (filters: Partial<SessionFilters>) => {
    const newFilters = { ...get().filters, ...filters };
    if (!filters.query && filters.query !== undefined) {
      delete newFilters.query;
    }
    if (!filters.agent && filters.agent !== undefined) {
      delete newFilters.agent;
    }
    if (!filters.title && filters.title !== undefined) {
      delete newFilters.title;
    }
    if (!filters.project_path && filters.project_path !== undefined) {
      delete newFilters.project_path;
    }
    if (!filters.model && filters.model !== undefined) {
      delete newFilters.model;
    }
    set({ filters: newFilters });
    get().loadSessions();
  },

  search: async (query: string) => {
    if (!query.trim()) {
      get().setFilters({ query: undefined as unknown as string });
      return;
    }
    get().setFilters({ query });
  },

  loadMoreMessages: async () => {
    const { messages, selectedSessionId } = get();
    if (!selectedSessionId) return;
    try {
      const more = await invoke<Message[]>("get_session_messages", {
        sessionId: selectedSessionId,
        offset: messages.length,
        limit: 500,
      });
      if (more.length > 0) {
        set({ messages: [...messages, ...more] });
      }
    } catch (e) {
      console.error("Failed to load more messages:", e);
    }
  },

  refreshActiveSessions: async () => {
    try {
      const activeIds = await invoke<string[]>("get_active_sessions");
      set({ activeSessionIds: new Set(activeIds) });
    } catch (e) {
      console.error("Failed to refresh active sessions:", e);
    }
  },

  reindex: async () => {
    set({ loading: true, indexError: null });
    try {
      const stats = await invoke<IndexStats>("reindex_all");
      console.log("Reindex complete:", stats);
      set({ indexStats: stats, lastSyncAt: stats.last_sync_at });
      await get().loadSessions();
    } catch (e) {
      console.error("Failed to reindex:", e);
      set({ indexError: String(e), loading: false });
    }
  },

  loadSyncStatus: async () => {
    try {
      const status = await invoke<SyncStatus>("get_sync_status");
      set({ lastSyncAt: status.last_sync_at });
    } catch (e) {
      console.error("Failed to load sync status:", e);
    }
  },

  toggleRole: (role: MessageRole) => {
    set((state) => {
      const next = new Set(state.enabledRoles);
      if (next.has(role)) {
        next.delete(role);
      } else {
        next.add(role);
      }
      return { enabledRoles: next };
    });
  },

  toggleAgent: (agent: AgentType) => {
    set((state) => {
      const next = new Set(state.enabledAgents);
      if (next.has(agent)) {
        next.delete(agent);
      } else {
        next.add(agent);
      }
      return { enabledAgents: next };
    });
    get().loadSessions();
  },

  setEnabledAgents: (agents: AgentType[]) => {
    set({ enabledAgents: new Set(agents) });
    get().loadSessions();
  },

  setSort: (column: SortColumn) => {
    set((state) => {
      if (state.sortConfig?.column === column) {
        if (state.sortConfig.direction === "asc") {
          return { sortConfig: { column, direction: "desc" } };
        }
        return { sortConfig: null };
      }
      return { sortConfig: { column, direction: "asc" } };
    });
  },

  setGitBranchFilter: (branch: string | null) => {
    const filters = { ...get().filters };
    if (branch === null) {
      delete filters.git_branch;
    } else {
      filters.git_branch = branch;
    }
    set({ filters });
    get().loadSessions();
  },

  loadSettings: async () => {
    try {
      const [terminals, settings] = await Promise.all([
        invoke<TerminalInfo[]>("detect_terminals"),
        invoke<{ preferred_terminal: string | null }>("get_app_settings"),
      ]);
      set({
        availableTerminals: terminals,
        preferredTerminal: settings.preferred_terminal ?? null,
      });
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  },

  setPreferredTerminal: async (id: string) => {
    try {
      await invoke("save_app_setting", { key: "preferred_terminal", value: id });
      set({ preferredTerminal: id });
    } catch (e) {
      console.error("Failed to save terminal preference:", e);
    }
  },

  openSettings: () => set({ settingsOpen: true }),
  closeSettings: () => set({ settingsOpen: false }),
}));
