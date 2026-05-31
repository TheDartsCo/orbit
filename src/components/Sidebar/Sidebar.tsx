import { RefreshCw } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { SearchBar } from "./SearchBar";
import { FilterBar } from "./FilterBar";
import { SessionList } from "./SessionList";

export function Sidebar() {
  const sessions = useAppStore((s) => s.sessions);
  const reindex = useAppStore((s) => s.reindex);
  const loading = useAppStore((s) => s.loading);

  return (
    <aside className="w-80 bg-bg-secondary border-r border-border flex flex-col h-full shrink-0">
      <div className="p-3 border-b border-border space-y-2">
        <div className="flex items-center justify-between">
          <h1 className="text-sm font-semibold text-text-primary">Sessions</h1>
          <button
            onClick={reindex}
            disabled={loading}
            className="text-text-muted hover:text-text-secondary transition-colors"
          >
            <RefreshCw
              className={`w-4 h-4 ${loading ? "animate-spin" : ""}`}
            />
          </button>
        </div>
        <SearchBar />
        <FilterBar />
        <div className="text-xs text-text-muted">
          {sessions.length} session{sessions.length !== 1 ? "s" : ""}
        </div>
      </div>
      <SessionList />
    </aside>
  );
}
