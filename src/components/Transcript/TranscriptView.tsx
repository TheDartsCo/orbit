import { useRef, useEffect } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useAppStore } from "../../store/useAppStore";
import { MessageBubble } from "./MessageBubble";
import { SearchNav } from "./SearchNav";
import { Badge } from "../common/Badge";
import { isDisplayableMessage } from "./messageVisibility";
import type { MessageRole } from "../../types";

const ROLES: { role: MessageRole; label: string; activeClass: string }[] = [
  { role: "user", label: "user", activeClass: "border-blue-400 bg-blue-400/10 text-blue-400" },
  { role: "assistant", label: "assistant", activeClass: "border-fuchsia-400 bg-fuchsia-400/10 text-fuchsia-400" },
  { role: "tool", label: "tool", activeClass: "border-green-400 bg-green-400/10 text-green-400" },
  { role: "system", label: "system", activeClass: "border-amber-400 bg-amber-400/10 text-amber-400" },
];

export function TranscriptView() {
  const messages = useAppStore((s) => s.messages);
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);
  const sessions = useAppStore((s) => s.sessions);
  const loading = useAppStore((s) => s.messagesLoading);
  const enabledRoles = useAppStore((s) => s.enabledRoles);
  const toggleRole = useAppStore((s) => s.toggleRole);
  const filters = useAppStore((s) => s.filters);
  const searchQuery = filters.query || null;
  const session = sessions.find((s) => s.id === selectedSessionId);

  const parentRef = useRef<HTMLDivElement>(null);

  const visibleMessages = messages.filter(isDisplayableMessage);

  const filteredMessages = visibleMessages.filter((m) => enabledRoles.has(m.role as MessageRole));

  const virtualizer = useVirtualizer({
    count: filteredMessages.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 120,
    overscan: 5,
  });

  useEffect(() => {
    if (messages.length === 0 || !parentRef.current) return;

    if (searchQuery && filteredMessages.length > 0) {
      const firstMatchIdx = filteredMessages.findIndex((m) => {
        const q = searchQuery.toLowerCase();
        return (
          m.content?.toLowerCase().includes(q) ||
          m.tool_input?.toLowerCase().includes(q) ||
          m.tool_output?.toLowerCase().includes(q)
        );
      });
      if (firstMatchIdx >= 0) {
        requestAnimationFrame(() => {
          virtualizer.scrollToIndex(firstMatchIdx, { align: "start" });
        });
        return;
      }
    }

    parentRef.current.scrollTop = parentRef.current.scrollHeight;
  }, [selectedSessionId, searchQuery]);

  if (loading) {
    return (
      <div className="flex flex-1 items-center justify-center">
        <span className="text-sm text-text-muted">Loading transcript...</span>
      </div>
    );
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col bg-bg-primary">
      <header className="border-b border-border bg-bg-secondary">
        <div className="flex min-h-[58px] items-start gap-3 px-4 py-3">
          {session && <Badge agent={session.agent} size="md" />}
          <div className="min-w-0 flex-1">
            <h2 className="whitespace-normal break-words text-sm font-semibold leading-snug text-text-primary">
              {session?.title ?? "Transcript"}
            </h2>
            <p className="truncate text-xs text-text-muted">
              {session?.project_path ?? "No project path"}
            </p>
          </div>
        </div>
        <div className="flex h-10 items-center gap-3 border-t border-border px-4 text-xs font-semibold text-text-secondary">
          {ROLES.map(({ role, label, activeClass }) => {
            const isActive = enabledRoles.has(role);
            return (
              <button
                key={role}
                onClick={() => toggleRole(role)}
                className={`cursor-pointer rounded-full border px-3 py-1 transition-colors ${
                  isActive ? activeClass : "border-border text-text-muted hover:bg-bg-hover"
                }`}
              >
                {roleCount(visibleMessages, role)} {label}
              </button>
            );
          })}
          <span className="text-text-muted">{visibleMessages.length} total</span>
        </div>
      </header>
      <div className="relative flex min-h-0 flex-1 flex-col">
        <SearchNav />
        <div ref={parentRef} data-transcript-container className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <div
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            width: "100%",
            position: "relative",
          }}
        >
          {virtualizer.getVirtualItems().map((virtualItem) => {
            const message = filteredMessages[virtualItem.index];
            return (
              <div
                key={virtualItem.key}
                data-index={virtualItem.index}
                ref={virtualizer.measureElement}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${virtualItem.start}px)`,
                }}
              >
                <div className="pb-2">
                  <MessageBubble message={message} searchQuery={searchQuery} />
                </div>
              </div>
            );
          })}
        </div>
        </div>
      </div>
    </section>
  );
}

function roleCount(messages: { role: string }[], role: string) {
  return messages.filter((message) => message.role === role).length;
}
