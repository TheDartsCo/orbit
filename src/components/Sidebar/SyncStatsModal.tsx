import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { SyncStatus, AgentType } from "../../types";
import { AGENT_LABELS, AGENT_COLORS } from "../../types";

interface SyncStatsModalProps {
  open: boolean;
  onClose: () => void;
}

export function SyncStatsModal({ open, onClose }: SyncStatsModalProps) {
  const [status, setStatus] = useState<SyncStatus | null>(null);

  useEffect(() => {
    if (open) {
      invoke<SyncStatus>("get_sync_status").then(setStatus).catch(console.error);
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  const providers = status?.provider_stats ?? {};
  const agentKeys = Object.keys(providers) as AgentType[];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />
      <div className="relative z-10 w-full max-w-lg rounded-lg border border-border bg-bg-secondary shadow-2xl">
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h2 className="text-sm font-bold text-text-primary">Sync Statistics</h2>
          <button
            onClick={onClose}
            className="rounded-md p-1 text-text-muted transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="px-5 py-4">
          {status?.last_sync_at ? (
            <p className="mb-4 text-xs text-text-muted">
              Last synced {new Date(status.last_sync_at).toLocaleString()}
            </p>
          ) : (
            <p className="mb-4 text-xs text-text-muted">Never synced</p>
          )}

          {agentKeys.length === 0 ? (
            <p className="py-8 text-center text-sm text-text-muted">
              No sync data available yet.
            </p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs font-semibold text-text-secondary">
                  <th className="pb-2 pr-3">Provider</th>
                  <th className="pb-2 px-3 text-right">Indexed</th>
                  <th className="pb-2 px-3 text-right">Skipped</th>
                  <th className="pb-2 px-3 text-right">Errored</th>
                </tr>
              </thead>
              <tbody>
                {agentKeys.map((agent) => {
                  const s = providers[agent as string];
                  return (
                    <tr
                      key={agent}
                      className="border-b border-border/50 last:border-0"
                    >
                      <td className="py-2.5 pr-3">
                        <span className="flex items-center gap-2">
                          <span
                            className={`inline-block h-2 w-2 rounded-full ${AGENT_COLORS[agent] ?? "bg-gray-500"}`}
                          />
                          <span className="font-medium text-text-primary">
                            {AGENT_LABELS[agent] ?? agent}
                          </span>
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right text-green-400">
                        {s.indexed}
                      </td>
                      <td className="py-2.5 px-3 text-right text-yellow-400">
                        {s.skipped}
                      </td>
                      <td className="py-2.5 px-3 text-right text-red-400">
                        {s.errored}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        <div className="flex justify-end border-t border-border px-5 py-3">
          <button
            onClick={onClose}
            className="rounded-md bg-bg-tertiary px-4 py-1.5 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
