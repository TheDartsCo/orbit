import { Copy, Play } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { invoke } from "@tauri-apps/api/core";

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
    <div className="flex h-11 items-center justify-between border-t border-border bg-bg-secondary px-3">
      <div className="flex min-w-0 items-center gap-2 text-xs font-medium text-text-secondary">
        <span className="truncate">
          Created {new Date(session.created_at).toLocaleString()}
        </span>
        {session.is_active && (
          <>
            <span className="text-text-muted">|</span>
            <span className="flex items-center gap-1 text-success">
              <span className="h-1.5 w-1.5 rounded-full bg-success" />
              Active
            </span>
          </>
        )}
      </div>
      <div className="flex items-center gap-2">
        <button
          onClick={handleCopyResume}
          className="flex items-center gap-1.5 rounded-md bg-bg-tertiary px-3 py-1.5 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary"
        >
          <Copy className="h-3.5 w-3.5" />
          Copy Resume
        </button>
        <button
          onClick={handleLaunchResume}
          className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-accent-hover"
        >
          <Play className="h-3.5 w-3.5" />
          Resume
        </button>
      </div>
    </div>
  );
}
