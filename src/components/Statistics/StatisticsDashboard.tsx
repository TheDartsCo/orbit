import { ArrowLeft, BarChart3, FolderKanban, RefreshCw } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import type { StatisticsMode, StatisticsPeriod } from "../../types";
import { AgentStatistics } from "./AgentStatistics";
import { ModelStatistics } from "./ModelStatistics";
import { ProjectStatistics } from "./ProjectStatistics";

const PERIODS: Array<{ value: StatisticsPeriod; label: string }> = [
  { value: "7d", label: "7d" },
  { value: "30d", label: "30d" },
  { value: "90d", label: "90d" },
  { value: "all", label: "All time" },
];

export function StatisticsDashboard() {
  const mode = useAppStore((state) => state.statisticsMode);
  const period = useAppStore((state) => state.statisticsPeriod);
  const statistics = useAppStore((state) => state.statistics);
  const loading = useAppStore((state) => state.statisticsLoading);
  const error = useAppStore((state) => state.statisticsError);
  const closeStatistics = useAppStore((state) => state.closeStatistics);
  const setMode = useAppStore((state) => state.setStatisticsMode);
  const setPeriod = useAppStore((state) => state.setStatisticsPeriod);
  const loadStatistics = useAppStore((state) => state.loadStatistics);

  return (
    <main className="flex h-full w-full flex-col overflow-hidden bg-bg-primary">
      <header className="flex min-h-16 flex-wrap items-center gap-3 border-b border-border bg-bg-secondary px-5 py-3">
        <button
          type="button"
          onClick={closeStatistics}
          className="flex h-9 items-center gap-2 rounded-lg border border-border px-3 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary focus:outline-none focus:ring-2 focus:ring-accent/60"
        >
          <ArrowLeft className="h-4 w-4" />
          Sessions
        </button>
        <div className="mr-auto flex items-center gap-3">
          <BarChart3 className="h-5 w-5 text-accent" />
          <div>
            <h1 className="text-lg font-bold">Statistics</h1>
            <p className="text-xs text-text-muted">
              Usage across indexed coding-agent sessions
            </p>
          </div>
        </div>
        <ModeSwitch mode={mode} onChange={setMode} />
        <div className="flex rounded-lg border border-border bg-bg-primary p-1">
          {PERIODS.map((item) => (
            <button
              key={item.value}
              type="button"
              onClick={() => setPeriod(item.value)}
              aria-pressed={period === item.value}
              className={`rounded-md px-3 py-1.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-accent/60 ${
                period === item.value
                  ? "bg-bg-active text-text-primary"
                  : "text-text-muted hover:bg-bg-hover hover:text-text-secondary"
              }`}
            >
              {item.label}
            </button>
          ))}
        </div>
      </header>

      <div className="flex-1 overflow-y-auto">
        {loading && !statistics ? (
          <StatisticsSkeleton />
        ) : error ? (
          <div className="flex min-h-[60vh] items-center justify-center p-8">
            <div className="max-w-md rounded-xl border border-danger/30 bg-danger/10 p-6 text-center">
              <h2 className="font-semibold text-text-primary">Statistics could not be loaded</h2>
              <p className="mt-2 text-sm text-text-secondary">{error}</p>
              <button
                type="button"
                onClick={() => void loadStatistics()}
                className="mt-5 inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-xs font-bold text-bg-primary hover:bg-accent-hover"
              >
                <RefreshCw className="h-4 w-4" />
                Retry
              </button>
            </div>
          </div>
        ) : statistics?.summary.sessions === 0 ? (
          <div className="flex min-h-[60vh] items-center justify-center p-8 text-center">
            <div>
              <FolderKanban className="mx-auto h-10 w-10 text-text-muted" />
              <h2 className="mt-4 text-base font-semibold">No sessions in this period</h2>
              <p className="mt-1 text-sm text-text-muted">
                Choose a longer range or reindex your session history.
              </p>
            </div>
          </div>
        ) : statistics?.mode === "agent" ? (
          <AgentStatistics dashboard={statistics} period={period} />
        ) : statistics?.mode === "model" ? (
          <ModelStatistics dashboard={statistics} period={period} />
        ) : statistics?.mode === "project" ? (
          <ProjectStatistics dashboard={statistics} period={period} />
        ) : null}
      </div>
    </main>
  );
}

function ModeSwitch({
  mode,
  onChange,
}: {
  mode: StatisticsMode;
  onChange: (mode: StatisticsMode) => void;
}) {
  return (
    <div className="flex rounded-lg border border-border bg-bg-primary p-1" role="tablist">
      {(["agent", "model", "project"] as const).map((value) => (
        <button
          key={value}
          type="button"
          role="tab"
          aria-selected={mode === value}
          onClick={() => onChange(value)}
          className={`rounded-md px-3 py-1.5 text-xs font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-accent/60 ${
            mode === value
              ? "bg-accent/15 text-accent"
              : "text-text-muted hover:bg-bg-hover hover:text-text-secondary"
          }`}
        >
          {value === "agent"
            ? "Agent stats"
            : value === "model"
              ? "Model stats"
              : "Project stats"}
        </button>
      ))}
    </div>
  );
}

function StatisticsSkeleton() {
  return (
    <div className="mx-auto max-w-[1600px] space-y-6 p-6">
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        {[0, 1, 2, 3].map((item) => (
          <div key={item} className="h-32 animate-pulse rounded-xl bg-bg-tertiary" />
        ))}
      </div>
      <div className="h-96 animate-pulse rounded-xl bg-bg-tertiary" />
      <div className="grid gap-6 lg:grid-cols-2">
        <div className="h-80 animate-pulse rounded-xl bg-bg-tertiary" />
        <div className="h-80 animate-pulse rounded-xl bg-bg-tertiary" />
      </div>
    </div>
  );
}
