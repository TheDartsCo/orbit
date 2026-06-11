import {
  AGENT_CHART_COLORS,
  type AgentType,
} from "../../types";

const SERIES_COLORS = [
  "#38bdf8",
  "#a78bfa",
  "#2dd4bf",
  "#f472b6",
  "#fb923c",
  "#818cf8",
  "#34d399",
  "#facc15",
];

export function agentColor(agent: string): string {
  return AGENT_CHART_COLORS[agent as AgentType] ?? "#7f8098";
}

export function seriesColor(key: string, index: number): string {
  if (key === "Other") return "#7f8098";
  return SERIES_COLORS[index % SERIES_COLORS.length];
}
