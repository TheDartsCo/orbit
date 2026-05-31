import { useAppStore } from "../../store/useAppStore";
import { AGENT_LABELS, type AgentType } from "../../types";

const AGENTS: AgentType[] = ["claude", "codex", "cursor", "opencode"];

export function FilterBar() {
  const filters = useAppStore((s) => s.filters);
  const setFilters = useAppStore((s) => s.setFilters);

  return (
    <div className="flex flex-wrap gap-1.5">
      <button
        onClick={() => setFilters({ agent: undefined as unknown as string })}
        className={`px-2 py-1 rounded-md text-xs transition-colors ${
          !filters.agent
            ? "bg-accent text-white"
            : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover"
        }`}
      >
        All
      </button>
      {AGENTS.map((agent) => (
        <button
          key={agent}
          onClick={() =>
            setFilters({
              agent:
                filters.agent === agent
                  ? (undefined as unknown as string)
                  : agent,
            })
          }
          className={`px-2 py-1 rounded-md text-xs transition-colors ${
            filters.agent === agent
              ? "bg-accent text-white"
              : "bg-bg-tertiary text-text-secondary hover:bg-bg-hover"
          }`}
        >
          {AGENT_LABELS[agent]}
        </button>
      ))}
    </div>
  );
}
