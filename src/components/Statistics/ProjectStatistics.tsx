import type { StatisticsDashboard, AgentType } from "../../types";
import { AGENT_LABELS } from "../../types";
import { agentColor, seriesColor } from "./statisticsColors";
import {
  formatCompactNumber,
  formatPercentage,
  formatRelativeTime,
} from "./statisticsFormat";
import { HorizontalBarChart } from "./charts/HorizontalBarChart";
import { PercentStackedBarChart } from "./charts/PercentStackedBarChart";
import { StackedBarChart } from "./charts/StackedBarChart";
import { ChartPanel, DataTable, StatCard } from "./StatisticsParts";

type ProjectDashboard = Extract<StatisticsDashboard, { mode: "project" }>;

export function ProjectStatistics({ dashboard }: { dashboard: ProjectDashboard }) {
  const { summary, timeline, projects, cards } = dashboard;
  const projectColors = new Map(
    projects.map((project, index) => [project.project, seriesColor(project.project, index)])
  );

  return (
    <div className="mx-auto max-w-[1600px] space-y-6 p-6">
      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard label="Projects" value={formatCompactNumber(summary.project_count)} detail="normalized project names" />
        <StatCard label="Sessions" value={formatCompactNumber(summary.sessions)} detail={`${formatCompactNumber(summary.average_messages_per_session)} avg messages`} />
        <StatCard label="Messages" value={formatCompactNumber(summary.messages)} detail="across all projects" />
        <StatCard label="Total tokens" value={formatCompactNumber(summary.total_tokens)} detail={`${formatCompactNumber(summary.average_tokens_per_session)} avg / session`} />
      </div>

      <div className="grid gap-4 md:grid-cols-2">
        {cards.map((project, index) => (
          <article key={project.project} className="rounded-xl border border-border bg-bg-secondary p-5">
            <div className="flex items-start justify-between gap-4">
              <div className="flex min-w-0 items-center gap-2">
                <span className="h-2.5 w-2.5 shrink-0 rounded-full" style={{ backgroundColor: seriesColor(project.project, index) }} />
                <h2 className="truncate text-base font-bold">{project.project}</h2>
              </div>
              <span className="shrink-0 text-xs text-text-muted">{formatRelativeTime(project.last_active)}</span>
            </div>
            <div className="mt-5 flex items-baseline gap-6">
              <div><span className="text-2xl font-bold">{formatCompactNumber(project.sessions)}</span> <span className="text-sm text-text-muted">sessions</span></div>
              <div><span className="text-2xl font-bold">{formatCompactNumber(project.tokens)}</span> <span className="text-sm text-text-muted">tokens</span></div>
            </div>
            <div className="mt-5 flex h-2.5 overflow-hidden rounded-full bg-bg-primary">
              {project.agent_mix.map((share) => (
                <span
                  key={share.agent}
                  style={{ width: `${share.percentage}%`, backgroundColor: agentColor(share.agent) }}
                  title={`${agentLabel(share.agent)} ${formatPercentage(share.percentage)}`}
                />
              ))}
            </div>
            <div className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-text-muted">
              {project.agent_mix.slice(0, 4).map((share) => (
                <span key={share.agent}>{agentLabel(share.agent)} {formatPercentage(share.percentage)}</span>
              ))}
            </div>
          </article>
        ))}
      </div>

      <ChartPanel title="Tokens by project over time">
        <StackedBarChart
          title="Tokens by project over time"
          buckets={timeline}
          colorForKey={(key, index) => projectColors.get(key) ?? seriesColor(key, index)}
        />
      </ChartPanel>

      <div className="grid gap-6 xl:grid-cols-2">
        <ChartPanel title="Sessions by project">
          <HorizontalBarChart
            title="Sessions by project"
            rows={projects.map((project) => ({
              key: project.project,
              label: project.project,
              value: project.sessions,
            }))}
            colorForRow={(key, index) => projectColors.get(key) ?? seriesColor(key, index)}
          />
        </ChartPanel>
        <ChartPanel title="Agent mix per project">
          <PercentStackedBarChart rows={projects} />
        </ChartPanel>
      </div>

      <DataTable
        title="Project comparison"
        headers={["Project", "Sessions", "Tokens", "Agents", "Top agent", "Last active"]}
      >
        {projects.map((project, index) => (
          <tr key={project.project} className="border-b border-border/70 last:border-0">
            <td className="px-5 py-3.5 font-semibold text-text-primary">
              <span className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: projectColors.get(project.project) ?? seriesColor(project.project, index) }} />
                {project.project}
              </span>
            </td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(project.sessions)}</td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(project.tokens)}</td>
            <td className="px-5 py-3.5 text-right tabular-nums">{formatCompactNumber(project.agent_count)}</td>
            <td className="px-5 py-3.5 text-right">{agentLabel(project.top_agent)}</td>
            <td className="px-5 py-3.5 text-right text-text-muted">{formatRelativeTime(project.last_active)}</td>
          </tr>
        ))}
      </DataTable>
    </div>
  );
}

function agentLabel(agent: string): string {
  return AGENT_LABELS[agent as AgentType] ?? agent;
}
