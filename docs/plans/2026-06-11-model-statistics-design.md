# Model Statistics Design

## Goal

Add Model stats as a third full statistics dashboard alongside Agent stats and
Project stats.

## Navigation

- Add `Model stats` to the existing statistics tab switcher.
- Reuse the shared `7d`, `30d`, `90d`, and `All time` controls.
- Preserve the selected period when switching among Agent, Model, and Project
  stats.

## Metric Rules

- Period filtering continues to use `sessions.created_at`.
- Tokens continue to mean `input_tokens + output_tokens`.
- Model names are shown exactly as stored.
- Missing or blank model values are grouped under `Unknown model`.
- Last-used values use `updated_at`.

## Backend

Add `model` to `StatisticsMode` and add a `Model` variant to the tagged
`StatisticsDashboard` response.

The Model response contains:

- Overall `StatisticsSummary`.
- Sessions-over-time buckets grouped by model.
- Model leaderboard rows with sessions, messages, tokens, agent count, top
  agent, last used, and agent mix.

The leaderboard retains every model. Charts keep the top eight models and group
the remainder as `Other`.

## Dashboard

### Summary Cards

- Sessions.
- Messages.
- Total tokens.
- Active models.

### Charts

- Stacked sessions over time by model.
- Horizontal tokens by model.
- 100% agent split per model.

### Leaderboard

Columns:

- Model.
- Sessions.
- Messages.
- Tokens.
- Agents.
- Top agent.
- Last used.

## States And Accessibility

Reuse the existing loading, error, empty, responsive, table, legend, and focus
patterns from the Agent and Project dashboards.

## Testing

Backend tests cover:

- Missing models becoming `Unknown model`.
- Exact model names remaining separate.
- Token, session, message, agent-count, top-agent, and last-used aggregation.
- Timeline grouping by model.
- Top-eight plus `Other` chart behavior.

Frontend verification covers:

- Model tab selection.
- Shared period controls.
- All charts and leaderboard rendering.
- Narrow and wide layouts.

