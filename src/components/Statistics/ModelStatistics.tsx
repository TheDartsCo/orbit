import type {
  AgentType,
  StatisticsDashboard,
  StatisticsPeriod,
} from "../../types";
import { AGENT_LABELS } from "../../types";
import { agentColor, seriesColor } from "./statisticsColors";
import {
  formatCompactNumber,
  formatRelativeTime,
} from "./statisticsFormat";
import { HorizontalBarChart } from "./charts/HorizontalBarChart";
import { MixStackedBarChart } from "./charts/MixStackedBarChart";
import { StackedBarChart } from "./charts/StackedBarChart";
import { ChartPanel, DataTable, StatCard } from "./StatisticsParts";

type ModelDashboard = Extract<StatisticsDashboard, { mode: "model" }>;

export function ModelStatistics({
  dashboard,
  period,
}: {
  dashboard: ModelDashboard;
  period: StatisticsPeriod;
}) {
  const { summary, timeline, models } = dashboard;
  const modelColors = new Map(
    models.map((model, index) => [model.model, seriesColor(model.model, index)])
  );

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
          label="Active models"
          value={formatCompactNumber(models.length)}
          detail={`used by ${formatCompactNumber(summary.active_agents)} agents`}
        />
      </div>

      <ChartPanel title="Sessions over time">
        <StackedBarChart
          title="Sessions over time by model"
          buckets={timeline}
          period={period}
          colorForKey={(key, index) =>
            modelColors.get(key) ?? seriesColor(key, index)
          }
        />
      </ChartPanel>

      <div className="grid gap-6 xl:grid-cols-2">
        <ChartPanel title="Tokens by model">
          <HorizontalBarChart
            title="Tokens by model"
            rows={models.slice(0, 8).map((model) => ({
              key: model.model,
              label: model.model,
              value: model.tokens,
            }))}
            colorForRow={(key, index) =>
              modelColors.get(key) ?? seriesColor(key, index)
            }
          />
        </ChartPanel>
        <ChartPanel title="Agent split per model">
          <MixStackedBarChart
            rows={models.map((model) => ({
              key: model.model,
              label: model.model,
              mix: model.agent_mix,
            }))}
          />
        </ChartPanel>
      </div>

      <DataTable
        title="Model leaderboard"
        headers={[
          "Model",
          "Sessions",
          "Messages",
          "Tokens",
          "Agents",
          "Top agent",
          "Last used",
        ]}
      >
        {models.map((model, index) => (
          <tr
            key={model.model}
            className="border-b border-border/70 last:border-0"
          >
            <td className="px-5 py-3.5 font-semibold text-text-primary">
              <span className="flex items-center gap-2">
                <span
                  className="h-2.5 w-2.5 rounded-full"
                  style={{
                    backgroundColor:
                      modelColors.get(model.model) ??
                      seriesColor(model.model, index),
                  }}
                />
                {model.model}
              </span>
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">
              {formatCompactNumber(model.sessions)}
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">
              {formatCompactNumber(model.messages)}
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">
              {formatCompactNumber(model.tokens)}
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">
              {formatCompactNumber(model.agent_count)}
            </td>
            <td className="px-5 py-3.5 text-right">
              <span className="inline-flex items-center justify-end gap-2">
                <span
                  className="h-2 w-2 rounded-full"
                  style={{ backgroundColor: agentColor(model.top_agent) }}
                />
                {agentLabel(model.top_agent)}
              </span>
            </td>
            <td className="px-5 py-3.5 text-right text-text-muted">
              {formatRelativeTime(model.last_used)}
            </td>
          </tr>
        ))}
      </DataTable>
    </div>
  );
}

function agentLabel(agent: string): string {
  return AGENT_LABELS[agent as AgentType] ?? agent;
}
