import type { ProjectStatisticsRow } from "../../../types";
import { AGENT_LABELS, type AgentType } from "../../../types";
import { formatPercentage } from "../statisticsFormat";
import { agentColor } from "../statisticsColors";

interface PercentStackedBarChartProps {
  rows: ProjectStatisticsRow[];
}

export function PercentStackedBarChart({ rows }: PercentStackedBarChartProps) {
  const visibleRows = rows.slice(0, 8);
  if (visibleRows.length === 0) {
    return (
      <div className="flex h-52 items-center justify-center text-sm text-text-muted">
        No data for this period
      </div>
    );
  }

  return (
    <div className="space-y-5 py-2">
      {visibleRows.map((project) => (
        <div key={project.project} className="grid grid-cols-[110px_1fr] items-center gap-4">
          <div className="truncate text-right text-xs font-medium text-text-secondary">
            {project.project}
          </div>
          <div className="flex h-8 overflow-hidden rounded-md bg-bg-primary">
            {project.agent_mix.map((share) => (
              <div
                key={share.agent}
                className="flex min-w-0 items-center justify-center text-[10px] font-semibold text-bg-primary"
                style={{
                  width: `${share.percentage}%`,
                  backgroundColor: agentColor(share.agent),
                }}
                title={`${AGENT_LABELS[share.agent as AgentType] ?? share.agent}: ${formatPercentage(share.percentage)}`}
              >
                {share.percentage >= 16 ? formatPercentage(share.percentage) : ""}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
