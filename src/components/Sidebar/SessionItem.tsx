import type { Session } from "../../types";
import { Badge } from "../common/Badge";

interface SessionItemProps {
  session: Session;
  isSelected: boolean;
  onClick: () => void;
}

export function SessionItem({ session, isSelected, onClick }: SessionItemProps) {
  const timeStr = new Date(session.updated_at).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <button
      onClick={onClick}
      className={`w-full text-left px-3 py-2.5 border-b border-border transition-colors ${
        isSelected
          ? "bg-bg-active border-l-2 border-l-accent"
          : "hover:bg-bg-hover"
      }`}
    >
      <div className="flex items-center gap-2 mb-1">
        <Badge agent={session.agent} />
        {session.is_active && (
          <span className="w-2 h-2 rounded-full bg-success animate-pulse" />
        )}
      </div>
      <div className="text-sm text-text-primary truncate">{session.title}</div>
      <div className="flex items-center gap-2 mt-1 text-xs text-text-muted">
        <span>{timeStr}</span>
        <span className="truncate">{session.project_path}</span>
      </div>
    </button>
  );
}
