import type { Session } from "../../types";
import type { SortColumn } from "../../store/useAppStore";
import { Badge } from "../common/Badge";
import { Highlight } from "../common/Highlight";

interface SessionItemProps {
  session: Session;
  depth: number;
  childCount: number;
  isSelected: boolean;
  onClick: () => void;
  searchQuery: string | null;
  columnTemplate: string;
  activeColumns: SortColumn[];
}

export function SessionItem({
  session,
  depth,
  childCount,
  isSelected,
  onClick,
  searchQuery,
  columnTemplate,
  activeColumns,
}: SessionItemProps) {
  const updatedAt = new Date(session.updated_at);
  const timeStr = formatRelativeTime(updatedAt);
  const projectName = formatProjectName(session.project_path);
  const isChild = depth > 0;

  return (
    <button
      onClick={onClick}
      style={{
        gridTemplateColumns: columnTemplate,
      }}
      className={`grid h-8 w-full items-center gap-2 rounded-md px-3 text-left font-mono text-[13px] transition-colors ${
        isSelected
          ? "bg-bg-active text-text-primary"
          : "text-text-secondary hover:bg-bg-hover hover:text-text-primary"
      }`}
    >
      {activeColumns.map((colId) => (
        <div key={colId}>{renderCell(colId, session, isChild, childCount, searchQuery, depth, timeStr, projectName)}</div>
      ))}
    </button>
  );
}

function renderCell(
  colId: SortColumn,
  session: Session,
  isChild: boolean,
  childCount: number,
  searchQuery: string | null,
  depth: number,
  timeStr: string,
  projectName: string,
): React.ReactNode {
  switch (colId) {
    case "agent":
      return (
        <div className="flex min-w-0 items-center gap-2">
          <Badge agent={session.agent} />
          {isChild && (
            <span className="rounded border border-border px-1 py-0.5 text-[10px] leading-none text-text-muted">
              sub
            </span>
          )}
          {!isChild && childCount > 0 && (
            <span className="rounded border border-accent/30 px-1 py-0.5 text-[10px] leading-none text-accent">
              {childCount}
            </span>
          )}
          {session.is_active && (
            <span className="h-2 w-2 shrink-0 rounded-full bg-success shadow-[0_0_10px_rgba(48,209,88,0.85)]" />
          )}
        </div>
      );
    case "session":
      return (
        <div
          className="flex min-w-0 items-center gap-1 truncate font-semibold tracking-[0]"
          style={{ paddingLeft: `${Math.min(depth, 4) * 18}px` }}
        >
          {isChild ? (
            <span className="shrink-0 text-[14px] text-text-muted">↳</span>
          ) : childCount > 0 ? (
            <span className="shrink-0 text-[12px] text-accent">▾</span>
          ) : null}
          <span className="min-w-0 shrink truncate">
            <Highlight text={session.title} query={searchQuery} />
          </span>
        </div>
      );
    case "date":
      return <div className="truncate text-text-muted">{timeStr}</div>;
    case "project":
      return (
        <div
          className="truncate font-semibold text-text-secondary"
          title={session.project_path || projectName}
        >
          {projectName}
        </div>
      );
    case "model":
      return (
        <div className="truncate text-text-muted" title={session.model ?? ""}>
          {session.model ? shortModelName(session.model) : ""}
        </div>
      );
    case "branch":
      return (
        <div className="truncate text-text-muted" title={session.git_branch ?? ""}>
          {session.git_branch ?? ""}
        </div>
      );
    case "tokens": {
      const total = session.input_tokens + session.output_tokens;
      return (
        <div className="text-right tabular-nums text-text-muted" title={`${session.input_tokens.toLocaleString()} in / ${session.output_tokens.toLocaleString()} out`}>
          {total > 0 ? formatTokens(total) : ""}
        </div>
      );
    }
    case "files":
      return (
        <div className="text-right tabular-nums text-text-secondary">
          {session.file_count > 0 ? session.file_count : ""}
        </div>
      );
    case "messages":
      return (
        <div className="text-right tabular-nums text-text-secondary">
          {session.message_count}
        </div>
      );
  }
}

function formatRelativeTime(date: Date) {
  const diffMs = Date.now() - date.getTime();
  const minutes = Math.max(0, Math.floor(diffMs / 60000));
  if (minutes < 1) return "now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} hr ago`;
  const days = Math.floor(hours / 24);
  if (days < 7) return `${days} day${days === 1 ? "" : "s"} ago`;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function formatProjectName(path: string) {
  if (!path || path === "-") return "-";
  const cleanPath = path.replace(/\\/g, "/");
  const parts = cleanPath.split("/").filter(Boolean);
  if (parts.length > 1) return parts[parts.length - 1] ?? cleanPath;
  const segments = path.split("-").filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function shortModelName(model: string): string {
  return model.replace(/^claude-/i, "").replace(/^gpt-5-/i, "").replace(/^gpt-/i, "").replace(/^codex-/i, "") || model;
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`;
  return String(n);
}
