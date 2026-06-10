import type { StatisticsDashboard, AgentType } from "../../types";
import { AGENT_LABELS } from "../../types";
import { agentColor, seriesColor } from "./statisticsColors";
import { formatCompactNumber, formatRelativeTime } from "./statisticsFormat";
import { DonutChart } from "./charts/DonutChart";
import { HorizontalBarChart } from "./charts/HorizontalBarChart";
import { StackedBarChart } from "./charts/StackedBarChart";
import { ChartPanel, DataTable, StatCard } from "./StatisticsParts";

type AgentDashboard = Extract<StatisticsDashboard, { mode: "agent" }>;

export function AgentStatistics({ dashboard }: { dashboard: AgentDashboard }) {
  const { summary, timeline, agents, models } = dashboard;

  return (
    <div className="mx-auto max-w-[1600px] space-y-6 p-6">
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          label="Sessions"
          value={formatCompactNumber(summary.sessions)}
          detail={`${formatCompactNumber(summary.project_count)} projects`}
        />
        <StatCard
          label="Messages"
          value={formatCompactNumber(summary.messages)}
          detail={`${formatCompactNumber(summary.average_messages_per_session)} avg / session`}
        />
        <StatCard
          label="Total tokens"
          value={formatCompactNumber(summary.total_tokens)}
          detail={`${formatCompactNumber(summary.average_tokens_per_session)} avg / session`}
        />
        <StatCard
          label="Active agents"
          value={formatCompactNumber(summary.active_agents)}
          detail={`across ${formatCompactNumber(summary.project_count)} projects`}
        />
      </div>

      <ChartPanel title="Sessions over time">
        <StackedBarChart
          title="Sessions over time by agent"
          buckets={timeline}
          colorForKey={(key) => agentColor(key)}
        />
      </ChartPanel>

      <div className="grid gap-6 xl:grid-cols-2">
        <ChartPanel title="Tokens by agent">
          <HorizontalBarChart
            title="Tokens by agent"
            rows={agents.map((agent) => ({
              key: agent.agent,
              label: agentLabel(agent.agent),
              value: agent.tokens,
            }))}
            colorForRow={(key) => agentColor(key)}
          />
        </ChartPanel>
        <ChartPanel title="Token split by model">
          <DonutChart
            title="Token split by model"
            rows={models.map((model) => ({
              key: model.model,
              label: model.model,
              value: model.tokens,
              percentage: model.percentage,
            }))}
            colorForRow={seriesColor}
          />
        </ChartPanel>
      </div>

      <DataTable
        title="Agent leaderboard"
        headers={["Agent", "Sessions", "Messages", "Tokens", "Avg msgs", "Last used"]}
      >
        {agents.map((agent) => (
          <tr key={agent.agent} className="border-b border-border/70 last:border-0">
            <td className="px-5 py-3.5 font-semibold text-text-primary">
              <span className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: agentColor(agent.agent) }} />
                {agentLabel(agent.agent)}
              </span>
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(agent.sessions)}</td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(agent.messages)}</td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(agent.tokens)}</td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(agent.average_messages)}</td>
            <td className="px-5 py-3.5 text-right text-text-muted">{formatRelativeTime(agent.last_used)}</td>
          </tr>
        ))}
      </DataTable>
    </div>
  );
}

function agentLabel(agent: string): string {
  return AGENT_LABELS[agent as AgentType] ?? agent;
}
