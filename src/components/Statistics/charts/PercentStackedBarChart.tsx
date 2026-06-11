import type { ProjectStatisticsRow } from "../../../types";
import { MixStackedBarChart } from "./MixStackedBarChart";

interface PercentStackedBarChartProps {
  rows: ProjectStatisticsRow[];
}

export function PercentStackedBarChart({ rows }: PercentStackedBarChartProps) {
  return (
    <MixStackedBarChart
      rows={rows.map((project) => ({
        key: project.project,
        label: project.project,
        mix: project.agent_mix,
      }))}
    />
  );
}
