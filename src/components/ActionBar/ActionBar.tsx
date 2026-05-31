import { Copy, Play } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { invoke } from "@tauri-apps/api/core";
import { Badge } from "../common/Badge";

export function ActionBar() {
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);
  const sessions = useAppStore((s) => s.sessions);

  const session = sessions.find((s) => s.id === selectedSessionId);

  if (!session) return null;

  const handleCopyResume = async () => {
    try {
      const cmd = await invoke<string>("get_resume_command", {
        sessionId: session.id,
      });
      await navigator.clipboard.writeText(cmd);
    } catch (e) {
      console.error("Failed to get resume command:", e);
    }
  };

  const handleLaunchResume = async () => {
    try {
      await invoke("launch_resume", { sessionId: session.id });
    } catch (e) {
      console.error("Failed to launch resume:", e);
    }
  };

  return (
    <div className="h-12 border-t border-border bg-bg-secondary flex items-center justify-between px-4">
      <div className="flex items-center gap-3">
        <Badge agent={session.agent} size="md" />
        <span className="text-xs text-text-muted">
          {session.message_count} messages
        </span>
        <span className="text-xs text-text-muted">
          {new Date(session.created_at).toLocaleString()}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleCopyResume}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-bg-tertiary text-text-secondary text-xs hover:bg-bg-hover transition-colors"
        >
          <Copy className="w-3.5 h-3.5" />
          Copy Resume
        </button>
        <button
          onClick={handleLaunchResume}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-accent text-white text-xs hover:bg-accent-hover transition-colors"
        >
          <Play className="w-3.5 h-3.5" />
          Resume
        </button>
      </div>
    </div>
  );
}