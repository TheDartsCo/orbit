import { useMemo } from "react";
import { useAppStore } from "../../store/useAppStore";
import {
  ALL_AGENTS,
  AGENT_LABELS,
  AGENT_TEXT_COLORS,
  AGENT_TINTS,
} from "../../types";

export function FilterBar() {
  const enabledAgents = useAppStore((s) => s.enabledAgents);
  const toggleAgent = useAppStore((s) => s.toggleAgent);
  const sessions = useAppStore((s) => s.sessions);
  const gitBranchFilter = useAppStore((s) => s.filters.git_branch);
  const setGitBranchFilter = useAppStore((s) => s.setGitBranchFilter);

  const branches = useMemo(() => {
    const set = new Set<string>();
    for (const s of sessions) {
      if (s.git_branch) set.add(s.git_branch);
    }
    return Array.from(set).sort();
  }, [sessions]);

  return (
    <div className="flex min-w-0 items-center gap-2 overflow-x-auto pb-1">
      {ALL_AGENTS.map((agent) => {
        const active = enabledAgents.has(agent);
        return (
          <button
            key={agent}
            onClick={() => toggleAgent(agent)}
            className={`h-8 shrink-0 rounded-full border px-3 text-xs font-semibold transition-colors ${
              active
                ? `${AGENT_TINTS[agent]} ${AGENT_TEXT_COLORS[agent]}`
                : "border-border bg-bg-secondary text-text-secondary hover:bg-bg-hover opacity-50"
            }`}
          >
            {AGENT_LABELS[agent]}
          </button>
        );
      })}
      {branches.length > 0 && (
        <select
          value={gitBranchFilter ?? ""}
          onChange={(e) => setGitBranchFilter(e.target.value || null)}
          className="h-8 shrink-0 rounded-full border border-border bg-bg-secondary px-3 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover"
          aria-label="Filter by git branch"
        >
          <option value="">All branches</option>
          {branches.map((b) => (
            <option key={b} value={b}>
              {b}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}
