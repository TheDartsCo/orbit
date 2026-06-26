import { useVirtualizer } from "@tanstack/react-virtual";
import { useCallback, useMemo, useRef, type KeyboardEvent, type ReactNode } from "react";
import { useAppStore, type SortColumn, type SortConfig } from "../../store/useAppStore";
import type { Session } from "../../types";
import { SessionItem } from "./SessionItem";

const TABLE_HEADER_HEIGHT = 62;

const EMPTY_COLLAPSED: Set<string> = new Set();

interface TreeSession {
  session: Session;
  depth: number;
  childCount: number;
}

interface SessionListProps {
  columnTemplate: string;
  tableMinWidth: number;
  activeColumns: SortColumn[];
  collapsedParents: Set<string>;
  onToggleCollapse: (sessionId: string) => void;
  header: ReactNode;
}

export function SessionList({
  columnTemplate,
  tableMinWidth,
  activeColumns,
  collapsedParents,
  onToggleCollapse,
  header,
}: SessionListProps) {
  const sessions = useAppStore((s) => s.sessions);
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);
  const selectSession = useAppStore((s) => s.selectSession);
  const initialLoading = useAppStore((s) => s.initialLoading);
  const reindex = useAppStore((s) => s.reindex);
  const indexError = useAppStore((s) => s.indexError);
  const indexStats = useAppStore((s) => s.indexStats);
  const filters = useAppStore((s) => s.filters);
  const searchQuery = filters.query || null;
  const loading = useAppStore((s) => s.loading);
  const sortConfig = useAppStore((s) => s.sortConfig);

  const parentRef = useRef<HTMLDivElement>(null);
  const sortedSessions = useMemo(
    () => sortSessions(sessions, sortConfig),
    [sessions, sortConfig]
  );
  const effectiveCollapsed = searchQuery ? EMPTY_COLLAPSED : collapsedParents;
  const treeSessions = useMemo(
    () => buildSessionTree(sortedSessions, effectiveCollapsed),
    [sortedSessions, effectiveCollapsed]
  );

  const virtualizer = useVirtualizer({
    count: treeSessions.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 33,
    overscan: 18,
    scrollMargin: TABLE_HEADER_HEIGHT,
  });

  const selectSessionAtIndex = useCallback(
    (index: number) => {
      const nextSession = treeSessions[index]?.session;
      if (!nextSession) return;

      void selectSession(nextSession.id);
      virtualizer.scrollToIndex(index, { align: "auto" });
    },
    [selectSession, treeSessions, virtualizer]
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;

      event.preventDefault();
      if (treeSessions.length === 0) return;

      const selectedIndex = treeSessions.findIndex(({ session }) => session.id === selectedSessionId);
      if (event.key === "ArrowDown") {
        selectSessionAtIndex(selectedIndex < 0 ? 0 : Math.min(selectedIndex + 1, treeSessions.length - 1));
        return;
      }

      selectSessionAtIndex(selectedIndex < 0 ? treeSessions.length - 1 : Math.max(selectedIndex - 1, 0));
    },
    [selectSessionAtIndex, selectedSessionId, treeSessions]
  );

  return (
    <div
      ref={parentRef}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      className="min-w-0 flex-1 overflow-auto focus:outline-none"
    >
      <div className="sticky top-0 z-20" style={{ minWidth: `${tableMinWidth}px` }}>
        {header}
      </div>
      {initialLoading ? (
        <div className="flex h-[240px] items-center justify-center p-4" style={{ minWidth: `${tableMinWidth}px` }}>
          <span className="text-sm text-text-muted">Loading sessions...</span>
        </div>
      ) : sessions.length === 0 ? (
        <div className="flex h-[320px] items-center justify-center p-4" style={{ minWidth: `${tableMinWidth}px` }}>
          <div className="flex flex-col items-center gap-3">
            {indexError ? (
              <>
                <span className="text-xs text-danger text-center max-w-[250px]">{indexError}</span>
                <button
                  onClick={reindex}
                  disabled={loading}
                  className="rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-accent-hover disabled:opacity-50"
                >
                  Retry Scan
                </button>
              </>
            ) : loading ? (
              <span className="text-sm text-text-muted">Scanning sessions...</span>
            ) : indexStats && indexStats.sessions_found === 0 ? (
              <>
                <span className="text-sm text-text-muted">No agent sessions found on disk</span>
                <span className="text-xs text-text-muted">Make sure Claude Code, Codex, or OpenCode sessions exist</span>
              </>
            ) : (
              <>
                <span className="text-sm text-text-muted">No sessions found</span>
                <button
                  onClick={reindex}
                  disabled={loading}
                  className="rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-accent-hover disabled:opacity-50"
                >
                  Scan Now
                </button>
              </>
            )}
          </div>
        </div>
      ) : (
      <div
        className="px-2 py-1"
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          minWidth: `${tableMinWidth}px`,
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => {
          const { session, depth, childCount } = treeSessions[virtualItem.index];
          return (
            <div
              key={virtualItem.key}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                minWidth: `${tableMinWidth}px`,
                transform: `translateY(${virtualItem.start - virtualizer.options.scrollMargin}px)`,
              }}
            >
              <SessionItem
                session={session}
                depth={depth}
                childCount={childCount}
                isCollapsed={collapsedParents.has(session.id)}
                onToggleCollapse={() => onToggleCollapse(session.id)}
                isSelected={selectedSessionId === session.id}
                onClick={() => selectSession(session.id)}
                searchQuery={searchQuery}
                columnTemplate={columnTemplate}
                activeColumns={activeColumns}
              />
            </div>
          );
        })}
      </div>
      )}
    </div>
  );
}

function sortSessions(sessions: Session[], sortConfig: SortConfig | null): Session[] {
  if (!sortConfig) return sessions;

  const { column, direction } = sortConfig;
  const mul = direction === "asc" ? 1 : -1;

  const sorted = [...sessions].sort((a, b) => {
    let cmp = 0;
    switch (column) {
      case "agent":
        cmp = a.agent.localeCompare(b.agent);
        break;
      case "session":
        cmp = a.title.localeCompare(b.title);
        break;
      case "date":
        cmp = a.updated_at.localeCompare(b.updated_at);
        break;
      case "project":
        cmp = (a.project_path ?? "").localeCompare(b.project_path ?? "");
        break;
      case "model":
        cmp = (a.model ?? "").localeCompare(b.model ?? "");
        break;
      case "branch":
        cmp = (a.git_branch ?? "").localeCompare(b.git_branch ?? "");
        break;
      case "tokens":
        cmp = (a.input_tokens + a.output_tokens) - (b.input_tokens + b.output_tokens);
        break;
      case "files":
        cmp = a.file_count - b.file_count;
        break;
      case "messages":
        cmp = a.message_count - b.message_count;
        break;
    }
    return cmp * mul;
  });

  return sorted;
}

function buildSessionTree(sessions: Session[], collapsedParents: Set<string>): TreeSession[] {
  const byId = new Map(sessions.map((session) => [session.id, session]));
  const childrenByParent = new Map<string, Session[]>();
  const roots: Session[] = [];

  for (const session of sessions) {
    const parentId = session.parent_session_id;
    if (parentId && byId.has(parentId)) {
      const children = childrenByParent.get(parentId) ?? [];
      children.push(session);
      childrenByParent.set(parentId, children);
    } else {
      roots.push(session);
    }
  }

  const flattened: TreeSession[] = [];
  const append = (session: Session, depth: number) => {
    const children = childrenByParent.get(session.id) ?? [];
    flattened.push({ session, depth, childCount: children.length });
    if (collapsedParents.has(session.id)) return;
    for (const child of children) {
      append(child, depth + 1);
    }
  };

  for (const root of roots) {
    append(root, 0);
  }

  return flattened;
}
