import { useEffect } from "react";
import { X } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { TERMINAL_LABELS } from "../../types";

export function SettingsModal() {
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const closeSettings = useAppStore((s) => s.closeSettings);
  const availableTerminals = useAppStore((s) => s.availableTerminals);
  const preferredTerminal = useAppStore((s) => s.preferredTerminal);
  const setPreferredTerminal = useAppStore((s) => s.setPreferredTerminal);
  const loadSettings = useAppStore((s) => s.loadSettings);

  useEffect(() => {
    if (settingsOpen) loadSettings();
  }, [settingsOpen, loadSettings]);

  if (!settingsOpen) return null;

  const available = availableTerminals.filter((t) => t.available);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60"
        onClick={closeSettings}
      />
      <div className="relative z-10 w-full max-w-md rounded-xl border border-border bg-bg-secondary p-6 shadow-2xl">
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold text-text-primary">Settings</h2>
          <button
            onClick={closeSettings}
            className="rounded-md p-1 text-text-muted transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-text-secondary">
              Terminal Emulator
            </label>
            <p className="mb-2 text-xs text-text-muted">
              Choose which terminal to use when resuming sessions.
            </p>
            <select
              value={preferredTerminal ?? "terminal"}
              onChange={(e) => setPreferredTerminal(e.target.value)}
              className="h-9 w-full rounded-lg border border-border bg-bg-primary px-3 text-sm font-medium text-text-primary outline-none transition-colors focus:border-accent"
            >
              {available.map((t) => (
                <option key={t.id} value={t.id}>
                  {TERMINAL_LABELS[t.id] ?? t.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="mt-6 flex justify-end">
          <button
            onClick={closeSettings}
            className="flex items-center gap-1.5 rounded-md bg-accent px-4 py-2 text-xs font-semibold text-white transition-colors hover:bg-accent-hover"
          >
            Save & Close
          </button>
        </div>
      </div>
    </div>
  );
}
