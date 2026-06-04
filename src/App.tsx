import {
  useCallback,
  useEffect,
  useState,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { Sidebar } from "./components/Sidebar/Sidebar";
import { TranscriptView } from "./components/Transcript/TranscriptView";
import { ActionBar } from "./components/ActionBar/ActionBar";
import { SettingsModal } from "./components/Settings/SettingsModal";
import { useAppStore } from "./store/useAppStore";

const SIDEBAR_MIN_WIDTH = 440;
const MAIN_MIN_WIDTH = 420;
const DEFAULT_SIDEBAR_WIDTH = 640;
const RESIZE_KEY_STEP = 40;

function App() {
  const [sidebarWidth, setSidebarWidth] = useState(DEFAULT_SIDEBAR_WIDTH);
  const loadSessions = useAppStore((s) => s.loadSessions);
  const refreshActiveSessions = useAppStore((s) => s.refreshActiveSessions);
  const loadSyncStatus = useAppStore((s) => s.loadSyncStatus);
  const loadSettings = useAppStore((s) => s.loadSettings);
  const selectedSessionId = useAppStore((s) => s.selectedSessionId);

  useEffect(() => {
    loadSessions();
    loadSyncStatus();
    loadSettings();
    const interval = setInterval(refreshActiveSessions, 5000);
    return () => clearInterval(interval);
  }, [loadSessions, refreshActiveSessions, loadSyncStatus, loadSettings]);

  const handleResizeStart = useCallback(
    (event: ReactPointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      const startX = event.clientX;
      const startWidth = sidebarWidth;

      const handleResize = (moveEvent: PointerEvent) => {
        const maxWidth = window.innerWidth - MAIN_MIN_WIDTH;
        const nextWidth = startWidth + moveEvent.clientX - startX;
        setSidebarWidth(
          Math.min(Math.max(nextWidth, SIDEBAR_MIN_WIDTH), maxWidth)
        );
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
    [sidebarWidth]
  );

  const handleResizeKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;

      event.preventDefault();
      const direction = event.key === "ArrowLeft" ? -1 : 1;
      const maxWidth = window.innerWidth - MAIN_MIN_WIDTH;
      setSidebarWidth((width) =>
        Math.min(
          Math.max(width + direction * RESIZE_KEY_STEP, SIDEBAR_MIN_WIDTH),
          maxWidth
        )
      );
    },
    []
  );

  return (
    <div className="flex h-screen overflow-hidden bg-bg-primary text-text-primary">
      <Sidebar width={sidebarWidth} />
      <SettingsModal />
      <div
        role="separator"
        aria-label="Resize session browser"
        aria-orientation="vertical"
        tabIndex={0}
        onPointerDown={handleResizeStart}
        onKeyDown={handleResizeKeyDown}
        className="group relative z-10 w-1 shrink-0 cursor-col-resize bg-border transition-colors hover:bg-accent focus:bg-accent focus:outline-none"
      >
        <div className="absolute left-1/2 top-1/2 h-14 w-1.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-border-light opacity-0 transition-opacity group-hover:opacity-100 group-focus:opacity-100" />
      </div>
      <main className="flex min-w-[420px] flex-1 flex-col bg-bg-primary">
        {selectedSessionId ? (
          <>
            <TranscriptView />
            <ActionBar />
          </>
        ) : (
          <div className="flex flex-1 items-center justify-center">
            <div className="text-center">
              <h2 className="text-lg font-semibold text-text-secondary">
                Orbit
              </h2>
              <p className="mt-1 text-sm text-text-muted">
                Select a session to view its transcript
              </p>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
