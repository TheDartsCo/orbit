import {
  AGENT_COLORS,
  AGENT_LABELS,
  AGENT_TEXT_COLORS,
  AGENT_TINTS,
  type AgentType,
} from "../../types";

interface BadgeProps {
  agent: AgentType;
  size?: "sm" | "md";
}

export function Badge({ agent, size = "sm" }: BadgeProps) {
  const color = AGENT_COLORS[agent];
  const label = AGENT_LABELS[agent];
  const textColor = AGENT_TEXT_COLORS[agent];
  const tint = AGENT_TINTS[agent];
  const sizeClasses =
    size === "sm" ? "text-xs px-1.5 py-0.5" : "text-sm px-2.5 py-1";

  return (
    <span
      className={`inline-flex items-center gap-1.5 whitespace-nowrap rounded-md border font-medium ${sizeClasses} ${tint} ${textColor}`}
    >
      <span className={`w-1.5 h-1.5 rounded-full ${color}`} />
      {label}
    </span>
  );
}
