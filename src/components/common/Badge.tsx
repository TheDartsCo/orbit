import { AGENT_COLORS, AGENT_LABELS, type AgentType } from "../../types";

interface BadgeProps {
  agent: AgentType;
  size?: "sm" | "md";
}

export function Badge({ agent, size = "sm" }: BadgeProps) {
  const color = AGENT_COLORS[agent];
  const label = AGENT_LABELS[agent];
  const sizeClasses =
    size === "sm" ? "text-xs px-1.5 py-0.5" : "text-sm px-2 py-1";

  return (
    <span
      className={`inline-flex items-center gap-1 ${sizeClasses} rounded-md bg-bg-tertiary text-text-secondary`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${color}`} />
      {label}
    </span>
  );
}
