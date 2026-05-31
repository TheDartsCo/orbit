import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";
import { useAppStore } from "../../store/useAppStore";
import { SessionItem } from "./SessionItem";

export function SessionList() {
  const sessions = useAppStore((s) => s.sessions);
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);
  const selectSession = useAppStore((s) => s.selectSession);
  const initialLoading = useAppStore((s) => s.initialLoading);
  const reindex = useAppStore((s) => s.reindex);

  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: sessions.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 80,
    overscan: 10,
  });

  if (initialLoading) {
    return (
      <div className="flex-1 flex items-center justify-center p-4">
        <span className="text-text-muted text-sm">Scanning sessions...</span>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-4">
        <div className="flex flex-col items-center gap-3">
          <span className="text-text-muted text-sm">No sessions found</span>
          <button
            onClick={reindex}
            className="px-3 py-1.5 rounded-md bg-accent text-white text-xs hover:bg-accent-hover transition-colors"
          >
            Scan Now
          </button>
        </div>
      </div>
    );
  }

  return (
    <div ref={parentRef} className="flex-1 overflow-y-auto">
      <div
        style={{
          height: `${virtualizer.getTotalSize()}px`,
          width: "100%",
          position: "relative",
        }}
      >
        {virtualizer.getVirtualItems().map((virtualItem) => {
          const session = sessions[virtualItem.index];
          return (
            <div
              key={virtualItem.key}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              <SessionItem
                session={session}
                isSelected={selectedSessionId === session.id}
                onClick={() => selectSession(session.id)}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
