import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  Check,
  ChevronDown,
  Columns3,
  Filter,
  RefreshCw,
  Settings,
  X,
} from "lucide-react";
import { useAppStore, type SortColumn } from "../../store/useAppStore";
import { SearchBar } from "./SearchBar";
import { SessionList } from "./SessionList";
import { SyncStatsModal } from "./SyncStatsModal";
import {
  ALL_AGENTS,
  AGENT_LABELS,
  AGENT_TEXT_COLORS,
  AGENT_TINTS,
  type AgentType,
} from "../../types";

type ColumnId = SortColumn;

interface ColumnConfig {
  id: ColumnId;
  label: string;
  min: number;
  max: number;
  align?: "center" | "right";
  resizable?: boolean;
  defaultWidth: number;
  toggleable?: boolean;
}

const COLUMNS: ColumnConfig[] = [
  { id: "agent", label: "Agent", min: 92, max: 220, resizable: true, defaultWidth: 120 },
  { id: "session", label: "Session", min: 140, max: 720, resizable: true, defaultWidth: 210 },
  { id: "date", label: "Date", min: 76, max: 180, resizable: true, defaultWidth: 92 },
  { id: "project", label: "Project", min: 92, max: 420, resizable: true, defaultWidth: 120 },
  { id: "model", label: "Model", min: 60, max: 180, resizable: true, defaultWidth: 104 },
  { id: "branch", label: "Branch", min: 60, max: 200, resizable: true, defaultWidth: 108 },
  { id: "tokens", label: "Tokens", min: 48, max: 120, align: "right", resizable: true, defaultWidth: 70 },
  { id: "files", label: "Files", min: 36, max: 80, align: "right", resizable: true, defaultWidth: 50 },
  { id: "messages", label: "Msgs", min: 48, max: 110, align: "right", defaultWidth: 54 },
];

const DEFAULT_VISIBLE = new Set<ColumnId>([
  "agent", "session", "date", "project", "tokens", "messages",
]);

const FILTERABLE_COLUMNS = new Set<ColumnId>(["agent", "session", "project", "model", "branch"]);

function formatTimeAgo(iso: string): string {
  const now = Date.now();
  const then = new Date(iso).getTime();
  const diff = Math.max(0, now - then);
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

interface SidebarProps {
  width: number;
}

export function Sidebar({ width }: SidebarProps) {
  const [columnWidths, setColumnWidths] = useState<Record<ColumnId, number>>(
    () => Object.fromEntries(COLUMNS.map((c) => [c.id, c.defaultWidth])) as Record<ColumnId, number>
  );
  const [visibleColumns, setVisibleColumns] = useState<Set<ColumnId>>(() => new Set(DEFAULT_VISIBLE));
  const [columnsMenuOpen, setColumnsMenuOpen] = useState(false);
  const [agentMenuOpen, setAgentMenuOpen] = useState(false);
  const columnsMenuRef = useRef<HTMLDivElement>(null);
  const agentMenuRef = useRef<HTMLDivElement>(null);
  const [syncModalOpen, setSyncModalOpen] = useState(false);
  const sessions = useAppStore((s) => s.sessions);
  const filters = useAppStore((s) => s.filters);
  const setFilters = useAppStore((s) => s.setFilters);
  const enabledAgents = useAppStore((s) => s.enabledAgents);
  const toggleAgent = useAppStore((s) => s.toggleAgent);
  const setEnabledAgents = useAppStore((s) => s.setEnabledAgents);
  const setGitBranchFilter = useAppStore((s) => s.setGitBranchFilter);
  const reindex = useAppStore((s) => s.reindex);
  const loading = useAppStore((s) => s.loading);
  const indexStats = useAppStore((s) => s.indexStats);
  const lastSyncAt = useAppStore((s) => s.lastSyncAt);
  const sortConfig = useAppStore((s) => s.sortConfig);
  const setSort = useAppStore((s) => s.setSort);
  const openSettings = useAppStore((s) => s.openSettings);

  useEffect(() => {
    if (!columnsMenuOpen && !agentMenuOpen) return;
    const handler = (e: MouseEvent) => {
      if (columnsMenuRef.current && !columnsMenuRef.current.contains(e.target as Node)) {
        setColumnsMenuOpen(false);
      }
      if (agentMenuRef.current && !agentMenuRef.current.contains(e.target as Node)) {
        setAgentMenuOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [columnsMenuOpen, agentMenuOpen]);

  const toggleColumn = useCallback((id: ColumnId) => {
    setVisibleColumns((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const activeColumns = useMemo(() => COLUMNS.filter((c) => visibleColumns.has(c.id)), [visibleColumns]);

  const branches = useMemo(() => {
    const set = new Set<string>();
    for (const session of sessions) {
      if (session.git_branch) set.add(session.git_branch);
    }
    if (filters.git_branch) set.add(filters.git_branch);
    return Array.from(set).sort();
  }, [filters.git_branch, sessions]);

  const agentCounts = useMemo(() => {
    const counts = new Map<AgentType, number>();
    for (const agent of ALL_AGENTS) counts.set(agent, 0);
    for (const session of sessions) {
      counts.set(session.agent, (counts.get(session.agent) ?? 0) + 1);
    }
    return counts;
  }, [sessions]);

  const activeAgentCount = enabledAgents.size;
  const hasActiveColumnFilter = useCallback(
    (columnId: ColumnId) =>
      (columnId === "agent" && activeAgentCount !== ALL_AGENTS.length) ||
      (columnId === "session" && Boolean(filters.title)) ||
      (columnId === "project" && Boolean(filters.project_path)) ||
      (columnId === "model" && Boolean(filters.model)) ||
      (columnId === "branch" && Boolean(filters.git_branch)),
    [
      activeAgentCount,
      filters.git_branch,
      filters.model,
      filters.project_path,
      filters.title,
    ]
  );

  const columnTemplate = useMemo(
    () =>
      activeColumns
        .map((column, index) =>
          index === activeColumns.length - 1
            ? `minmax(${columnWidths[column.id]}px, 1fr)`
            : `${columnWidths[column.id]}px`
        )
        .join(" "),
    [columnWidths, activeColumns]
  );

  const tableMinWidth = useMemo(
    () =>
      activeColumns.reduce((total, column) => total + columnWidths[column.id], 0) +
      (activeColumns.length - 1) * 8,
    [columnWidths, activeColumns]
  );

  const startColumnResize = useCallback(
    (column: ColumnConfig, event: ReactPointerEvent<HTMLDivElement>) => {
      if (!column.resizable) return;

      event.preventDefault();
      event.stopPropagation();

      const startX = event.clientX;
      const startWidth = columnWidths[column.id];

      const handleResize = (moveEvent: PointerEvent) => {
        const nextWidth = startWidth + moveEvent.clientX - startX;
        setColumnWidths((widths) => ({
          ...widths,
          [column.id]: Math.min(
            Math.max(nextWidth, column.min),
            column.max
          ),
        }));
      };

      const stopResize = () => {
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        window.removeEventListener("pointermove", handleResize);
        window.removeEventListener("pointerup", stopResize);
      };

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      window.addEventListener("pointermove", handleResize);
      window.addEventListener("pointerup", stopResize);
    },
    [columnWidths]
  );

  const setTextFilter = useCallback(
    (key: "title" | "project_path" | "model", value: string) => {
      setFilters({ [key]: value.trim() || undefined });
    },
    [setFilters]
  );

  const renderColumnFilter = useCallback(
    (column: ColumnConfig) => {
      switch (column.id) {
        case "agent":
          return (
            <div ref={agentMenuRef} className="relative mt-1">
              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  setAgentMenuOpen((open) => !open);
                }}
                className={`flex h-6 w-full min-w-0 items-center justify-between gap-1 rounded-md border px-2 text-[11px] font-medium shadow-inner transition-colors ${
                  activeAgentCount === ALL_AGENTS.length
                    ? "border-border/70 bg-bg-primary/45 text-text-muted hover:border-border-light hover:bg-bg-hover/60"
                    : "border-accent/40 bg-accent/10 text-accent"
                }`}
                aria-label="Filter agents"
              >
                <span className="truncate">
                  {activeAgentCount === ALL_AGENTS.length
                    ? "All agents"
                    : `${activeAgentCount} agent${activeAgentCount === 1 ? "" : "s"}`}
                </span>
                <ChevronDown className="h-3 w-3 shrink-0 opacity-70" />
              </button>
              {agentMenuOpen && (
                <div
                  onClick={(event) => event.stopPropagation()}
                  className="absolute left-0 top-full z-50 mt-1 w-44 rounded-lg border border-border bg-bg-secondary p-1.5 shadow-lg"
                >
                  <div className="mb-1 flex items-center gap-1 border-b border-border pb-1">
                    <button
                      type="button"
                      onClick={() => setEnabledAgents(ALL_AGENTS)}
                      className="flex-1 rounded px-2 py-1 text-[11px] font-semibold text-text-secondary hover:bg-bg-hover"
                    >
                      All
                    </button>
                    <button
                      type="button"
                      onClick={() => setEnabledAgents([])}
                      className="flex-1 rounded px-2 py-1 text-[11px] font-semibold text-text-secondary hover:bg-bg-hover"
                    >
                      None
                    </button>
                  </div>
                  {ALL_AGENTS.map((agent) => {
                    const active = enabledAgents.has(agent);
                    return (
                      <button
                        key={agent}
                        type="button"
                        onClick={() => toggleAgent(agent)}
                        className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs text-text-secondary hover:bg-bg-hover"
                      >
                        <span className="flex h-4 w-4 shrink-0 items-center justify-center rounded border border-border">
                          {active && <Check className="h-3 w-3" />}
                        </span>
                        <span className={`min-w-0 flex-1 truncate ${active ? AGENT_TEXT_COLORS[agent] : ""}`}>
                          {AGENT_LABELS[agent]}
                        </span>
                        <span className={`rounded border px-1.5 py-0.5 text-[10px] ${active ? AGENT_TINTS[agent] : "border-border text-text-muted"}`}>
                          {agentCounts.get(agent) ?? 0}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        case "session":
          return (
            <ColumnTextFilter
              value={filters.title ?? ""}
              placeholder="Find session"
              onChange={(value) => setTextFilter("title", value)}
            />
          );
        case "project":
          return (
            <ColumnTextFilter
              value={filters.project_path ?? ""}
              placeholder="Find project"
              onChange={(value) => setTextFilter("project_path", value)}
            />
          );
        case "model":
          return (
            <ColumnTextFilter
              value={filters.model ?? ""}
              placeholder="Find model"
              onChange={(value) => setTextFilter("model", value)}
            />
          );
        case "branch":
          return (
            <select
              value={filters.git_branch ?? ""}
              onClick={(event) => event.stopPropagation()}
              onChange={(event) => setGitBranchFilter(event.target.value || null)}
              className={`mt-1 h-6 w-full min-w-0 rounded-md border bg-bg-primary/45 px-2 text-[11px] font-medium shadow-inner outline-none transition-colors focus:border-accent ${
                filters.git_branch
                  ? "border-accent/40 text-accent"
                  : "border-border/70 text-text-muted hover:border-border-light hover:bg-bg-hover/60"
              }`}
              aria-label="Filter by git branch"
            >
              <option value="">All branches</option>
              {branches.map((branch) => (
                <option key={branch} value={branch}>
                  {branch}
                </option>
              ))}
            </select>
          );
        default:
          return null;
      }
    },
    [
      activeAgentCount,
      agentCounts,
      agentMenuOpen,
      branches,
      enabledAgents,
      filters.git_branch,
      filters.model,
      filters.project_path,
      filters.title,
      setEnabledAgents,
      setGitBranchFilter,
      setTextFilter,
      toggleAgent,
    ]
  );

  return (
    <aside
      className="flex h-full shrink-0 flex-col bg-bg-primary"
      style={{ width }}
    >
      <div className="border-b border-border bg-bg-secondary">
        <div className="flex h-[58px] items-center gap-3 px-4">
          <SearchBar />
          <div className="relative ml-auto flex shrink-0 items-center text-text-secondary">
            <button
              onClick={() => setColumnsMenuOpen((v) => !v)}
              className={`flex h-9 items-center gap-2 rounded-lg border px-3 text-xs font-semibold transition-colors ${
                columnsMenuOpen
                  ? "border-accent/40 bg-accent/10 text-accent"
                  : "border-border bg-bg-secondary hover:bg-bg-hover hover:text-text-primary"
              }`}
              aria-label="View options"
              aria-expanded={columnsMenuOpen}
            >
              <Columns3 className="h-4 w-4" />
              <span>View</span>
              <ChevronDown className="h-3.5 w-3.5 opacity-70" />
            </button>
            {columnsMenuOpen && (
              <div
                ref={columnsMenuRef}
                className="absolute right-0 top-full z-50 mt-1 w-48 rounded-lg border border-border bg-bg-secondary p-1.5 shadow-lg"
              >
                <div className="border-b border-border px-2 pb-1.5 pt-1 text-[11px] font-semibold uppercase tracking-[0.04em] text-text-muted">
                  Columns
                </div>
                {COLUMNS.filter((c) => c.toggleable !== false).map((column) => (
                  <button
                    key={column.id}
                    onClick={() => toggleColumn(column.id)}
                    className="mt-1 flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-xs text-text-secondary hover:bg-bg-hover"
                  >
                    <span className="flex h-4 w-4 shrink-0 items-center justify-center">
                      {visibleColumns.has(column.id) && <Check className="h-3 w-3" />}
                    </span>
                    {column.label}
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
      <SessionList
        columnTemplate={columnTemplate}
        tableMinWidth={tableMinWidth}
        activeColumns={activeColumns.map((c) => c.id)}
        header={
          <div className="flex h-[62px] shrink-0 items-center border-b border-border bg-bg-secondary px-4">
            <div
              className="grid w-full shrink-0 items-start gap-3 text-xs font-semibold text-text-secondary"
              style={{ gridTemplateColumns: columnTemplate, minWidth: tableMinWidth }}
            >
              {activeColumns.map((column) => (
                <div
                  key={column.id}
                  className={`group/column relative min-w-0 select-none ${
                    column.align === "center"
                      ? "text-center text-base leading-none"
                      : column.align === "right"
                        ? "text-right"
                        : ""
                  }`}
                >
                  <button
                    type="button"
                    onClick={() => setSort(column.id)}
                    className={`flex h-5 max-w-full items-center gap-1 truncate text-[12px] font-semibold leading-none tracking-[0] text-text-secondary hover:text-text-primary ${
                      column.align === "right"
                        ? "ml-auto justify-end text-right"
                        : column.align === "center"
                          ? "mx-auto justify-center"
                          : ""
                    }`}
                  >
                    <span className="truncate">{column.label}</span>
                    {!FILTERABLE_COLUMNS.has(column.id) && (
                      <Filter className="h-3 w-3 shrink-0 opacity-0" />
                    )}
                    {hasActiveColumnFilter(column.id) ? (
                      <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />
                    ) : null}
                    <SortIndicator
                      direction={
                        sortConfig?.column === column.id
                          ? sortConfig.direction
                          : null
                      }
                    />
                  </button>
                  {renderColumnFilter(column)}
                  {column.resizable && (
                    <div
                      role="separator"
                      aria-label={`Resize ${column.label} column`}
                      aria-orientation="vertical"
                      onPointerDown={(event) => startColumnResize(column, event)}
                      className="group absolute -right-2 top-1/2 flex h-9 w-4 -translate-y-1/2 cursor-col-resize items-center justify-center"
                    >
                      <span className="h-7 w-px rounded-full bg-border-light/0 transition-colors group-hover/column:bg-border-light group-hover:bg-accent group-active:bg-accent" />
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        }
      />
      <div className="flex h-7 w-full items-center border-t border-border bg-bg-secondary text-xs font-medium text-text-secondary">
        <button
          onClick={() => setSyncModalOpen(true)}
          className="flex h-full min-w-0 flex-1 items-center justify-between px-3 text-left transition-colors hover:bg-bg-hover"
        >
          <span className="truncate">
            {loading
              ? "Scanning..."
              : lastSyncAt
                ? `Last sync ${formatTimeAgo(lastSyncAt)}`
                : indexStats
                  ? `Indexed ${indexStats.sessions_indexed} of ${indexStats.sessions_found} sessions`
                  : "Ready"}
          </span>
          <span className="ml-3 shrink-0">
            {sessions.length} Session{sessions.length !== 1 ? "s" : ""}
          </span>
        </button>
        <button
          onClick={openSettings}
          className="flex h-full w-8 shrink-0 items-center justify-center border-l border-border text-text-muted transition-colors hover:bg-bg-hover hover:text-text-secondary"
          aria-label="Settings"
        >
          <Settings className="h-4 w-4" />
        </button>
        <button
          onClick={reindex}
          disabled={loading}
          className="flex h-full w-8 shrink-0 items-center justify-center border-l border-border text-text-muted transition-colors hover:bg-bg-hover hover:text-text-secondary disabled:cursor-wait"
          aria-label="Refresh sessions"
        >
          <RefreshCw
            className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
          />
        </button>
      </div>
      <SyncStatsModal open={syncModalOpen} onClose={() => setSyncModalOpen(false)} />
    </aside>
  );
}

interface ColumnTextFilterProps {
  value: string;
  placeholder: string;
  onChange: (value: string) => void;
}

interface SortIndicatorProps {
  direction: "asc" | "desc" | null;
}

function SortIndicator({ direction }: SortIndicatorProps) {
  return (
    <span
      className="flex shrink-0 flex-col items-center justify-center gap-0.5 leading-none"
      aria-hidden="true"
    >
      <span
        className={`text-[7px] leading-[6px] ${
          direction === "asc" ? "text-accent" : "text-text-muted/45"
        }`}
      >
        ▲
      </span>
      <span
        className={`text-[7px] leading-[6px] ${
          direction === "desc" ? "text-accent" : "text-text-muted/45"
        }`}
      >
        ▼
      </span>
    </span>
  );
}

function ColumnTextFilter({ value, placeholder, onChange }: ColumnTextFilterProps) {
  return (
    <div
      className="relative mt-1"
      onClick={(event) => event.stopPropagation()}
    >
      <Filter className="pointer-events-none absolute left-1.5 top-1/2 h-3 w-3 -translate-y-1/2 text-text-muted/80" />
      <input
        type="text"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        className={`h-6 w-full rounded-md border bg-bg-primary/45 pl-5 text-[11px] font-medium shadow-inner outline-none transition-colors placeholder:text-text-muted/90 focus:border-accent ${
          value
            ? "border-accent/40 pr-6 text-accent"
            : "border-border/70 pr-1.5 text-text-secondary hover:border-border-light hover:bg-bg-hover/60"
        }`}
      />
      {value && (
        <button
          type="button"
          onClick={() => onChange("")}
          className="absolute right-1 top-1/2 -translate-y-1/2 rounded p-0.5 text-text-muted hover:bg-bg-hover hover:text-text-primary"
          aria-label={`Clear ${placeholder.toLowerCase()} filter`}
        >
          <X className="h-3 w-3" />
        </button>
      )}
    </div>
  );
}
