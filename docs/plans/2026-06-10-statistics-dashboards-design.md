# Statistics Dashboards Design

## Goal

Add a full-window Statistics experience that lets users switch between Agent
stats and Project stats while preserving Orbit's existing session browser
state and visual theme.

## Navigation

- Add a top-level app mode: `Sessions` or `Statistics`.
- Open Statistics from a button in the session browser chrome.
- Statistics replaces the entire session browser and transcript layout.
- Returning to Sessions preserves the selected session, filters, sorting, and
  sidebar width.
- The Statistics screen has `Agent stats` and `Project stats` tabs.
- Both tabs share `7d`, `30d`, `90d`, and `All time` period controls.

## Metric Rules

- Period filtering uses `sessions.created_at`.
- Total tokens means `input_tokens + output_tokens`.
- Cached and reasoning tokens are excluded.
- Message totals use the persisted `message_count`.
- Last-used and last-active values use `updated_at`.
- Sessions without a project path are grouped under `Unknown project`.

## Project Identity

Project statistics merge sessions by a normalized project name instead of the
full path. This avoids splitting one project when an adapter stores a normal
path and another adapter stores an encoded path.

Normalization:

1. Convert Windows separators to `/`.
2. Extract the final path component when separators are present.
3. For encoded paths, extract the final non-empty segment.
4. Compare names case-insensitively.
5. Treat `.`, `/`, `\`, spaces, `_`, and `-` as equivalent separators.
6. Collapse repeated separators.
7. Use a readable project name from the most recently active matching session.
8. Use `Unknown project` when no usable name remains.

Examples that group together:

- `/Users/maf/My Files/orbit`
- `C:\Users\maf\My Files\orbit`
- `-Users-maf-My-Files-orbit`
- `orbit`

## Backend Architecture

Add a `get_statistics` Tauri command that accepts:

- `mode`: `agent` or `project`
- `period`: `7d`, `30d`, `90d`, or `all`

The Rust backend reads all matching session summary rows from SQLite and builds
dashboard-ready aggregates. Keeping aggregation in Rust provides one
authoritative implementation for date filtering, project normalization, token
totals, category limits, and `Other` grouping.

The response contains:

- Overall summary totals.
- Time-series buckets.
- Ranked agent, model, and project aggregates.
- Agent mix per project.
- Comparison-table rows.

No schema migration is required because sessions already persist agent, project
path, model, timestamps, message count, and token totals.

## Agent Dashboard

### Summary Cards

- Sessions.
- Messages, with average messages per session.
- Total tokens, with average tokens per session.
- Active agents, with project count.

### Charts

- Stacked sessions over time by agent.
- Horizontal tokens by agent.
- Donut chart for token split by model.

### Table

Agent leaderboard columns:

- Agent.
- Sessions.
- Messages.
- Tokens.
- Average messages.
- Last used.

## Project Dashboard

### Project Cards

Show the highest-activity projects with:

- Project name.
- Sessions.
- Tokens.
- Last active.
- Stacked agent mix and percentage labels.

### Charts

- Stacked tokens over time by project.
- Horizontal sessions by project.
- 100% stacked agent mix per project.

### Table

Project comparison columns:

- Project.
- Sessions.
- Tokens.
- Agents.
- Top agent.
- Last active.

## Chart Rendering

Use reusable React SVG components instead of introducing a chart dependency.
The required chart set is limited and stable:

- Stacked vertical bar chart.
- Horizontal bar chart.
- 100% stacked horizontal bar chart.
- Donut chart.

Charts must:

- Be responsive.
- Use Orbit theme tokens for surfaces, borders, labels, and grid lines.
- Use stable agent colors derived from the existing agent palette.
- Provide legends and hover/focus tooltips.
- Include accessible names and textual values.
- Handle zero values and empty datasets without invalid SVG dimensions.

Large category sets are limited to the highest-ranked categories. Remaining
values are combined into `Other` using a neutral theme color.

## UI States

- Loading: dashboard skeletons while the command runs.
- Error: inline error panel with a retry action.
- Empty: explanatory message when the selected period has no sessions.
- Refresh: reload after a successful reindex and whenever mode or period
  changes.

## Testing

Rust tests cover:

- `created_at` period boundaries.
- Input-plus-output token totals.
- Project-name normalization.
- Unknown projects.
- Agent, model, and project aggregation.
- Time-bucket generation.
- Stable ranking and `Other` aggregation.

Frontend tests cover:

- Number and relative-time formatting.
- Chart-domain and stacked-segment helpers.
- Empty and zero-value chart data.

Integration verification:

- `cargo test` from `src-tauri/`.
- `npm run build`.
- `git diff --check`.
- Visual inspection in the Tauri app at narrow and wide window sizes.

