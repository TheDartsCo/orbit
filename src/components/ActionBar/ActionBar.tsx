import { useEffect, useState } from "react";
import { Copy, Play } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { invoke } from "@tauri-apps/api/core";
import { WindowsResumeModal } from "./WindowsResumeModal";

export function ActionBar() {
  const [platform, setPlatform] = useState<string | null>(null);
  const [windowsResume, setWindowsResume] = useState<{
    command: string | null;
    error: string | null;
  } | null>(null);
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);
  const sessions = useAppStore((s) => s.sessions);

  const session = sessions.find((s) => s.id === selectedSessionId);

  useEffect(() => {
    invoke<string>("get_platform").then(setPlatform).catch(console.error);
  }, []);

  useEffect(() => {
    setWindowsResume(null);
  }, [selectedSessionId]);

  if (!session) return null;

  const resumeDisabled = session.agent === "jetbrains" || session.agent === "antigravity";
  const launchDisabled =
    platform === null || (resumeDisabled && platform !== "windows");

  const getResumeMessage = () => {
    if (session.agent === "jetbrains") {
      return "JetBrains AI sessions cannot be resumed from Orbit";
    }
    if (session.agent === "antigravity") {
      return "Antigravity sessions cannot be resumed from Orbit";
    }
    return undefined;
  };

  const handleCopyResume = async () => {
    if (resumeDisabled) return;

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
    if (platform === "windows") {
      if (resumeDisabled) {
        setWindowsResume({ command: null, error: null });
        return;
      }

      try {
        const command = await invoke<string>("get_resume_command", {
          sessionId: session.id,
        });
        setWindowsResume({ command, error: null });
      } catch (e) {
        setWindowsResume({ command: null, error: String(e) });
      }
      return;
    }

    if (launchDisabled) return;

    try {
      await invoke("launch_resume", { sessionId: session.id });
    } catch (e) {
      console.error("Failed to launch resume:", e);
    }
  };

  return (
    <>
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
            disabled={resumeDisabled}
            title={getResumeMessage()}
            className="flex items-center gap-1.5 rounded-md bg-bg-tertiary px-3 py-1.5 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-bg-tertiary disabled:hover:text-text-secondary"
          >
            <Copy className="h-3.5 w-3.5" />
            Copy Resume
          </button>
          <button
            onClick={handleLaunchResume}
            disabled={launchDisabled}
            title={
              resumeDisabled && platform !== "windows"
                ? getResumeMessage()
                : undefined
            }
            className="flex items-center gap-1.5 rounded-md bg-accent px-3 py-1.5 text-xs font-semibold text-white transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-45 disabled:hover:bg-accent"
          >
            <Play className="h-3.5 w-3.5" />
            Resume
          </button>
        </div>
      </div>
      {windowsResume && (
        <WindowsResumeModal
          sessionId={session.id}
          command={windowsResume.command}
          error={windowsResume.error}
          onClose={() => setWindowsResume(null)}
        />
      )}
    </>
  );
}
