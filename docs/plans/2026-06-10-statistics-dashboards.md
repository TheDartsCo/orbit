# Statistics Dashboards Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add full-window Agent and Project statistics dashboards with shared
date filters, Orbit-themed SVG charts, and backend-owned aggregation.

**Architecture:** Add typed statistics models and a Rust aggregation module that
loads filtered session summaries from SQLite and returns dashboard-ready data
through one Tauri command. Add a top-level frontend view mode and a dedicated
Statistics feature composed from reusable formatting, chart, table, and state
components.

**Tech Stack:** Rust, rusqlite, chrono, Tauri v2, React 19, TypeScript,
Tailwind CSS v4, inline SVG

---

### Task 1: Define Statistics Contracts

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src/types/index.ts`

**Step 1: Add the Rust request enums**

Add:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StatisticsMode {
    Agent,
    Project,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum StatisticsPeriod {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "90d")]
    NinetyDays,
    #[serde(rename = "all")]
    All,
}
```

**Step 2: Add shared response records**

Define serializable records for:

- `StatisticsSummary`
- `StatisticsSeriesValue`
- `StatisticsTimeBucket`
- `AgentStatisticsRow`
- `ModelStatisticsRow`
- `ProjectStatisticsRow`
- `ProjectAgentShare`
- `ProjectStatisticsCard`

Every count and token field uses `u64`; timestamps use `DateTime<Utc>`.

**Step 3: Add a tagged dashboard response**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum StatisticsDashboard {
    Agent {
        summary: StatisticsSummary,
        timeline: Vec<StatisticsTimeBucket>,
        agents: Vec<AgentStatisticsRow>,
        models: Vec<ModelStatisticsRow>,
    },
    Project {
        summary: StatisticsSummary,
        timeline: Vec<StatisticsTimeBucket>,
        projects: Vec<ProjectStatisticsRow>,
        cards: Vec<ProjectStatisticsCard>,
    },
}
```

**Step 4: Mirror the contract in TypeScript**

Add discriminated unions and supporting interfaces to `src/types/index.ts`.
Use `StatisticsMode = "agent" | "project"` and
`StatisticsPeriod = "7d" | "30d" | "90d" | "all"`.

**Step 5: Verify type compilation**

Run: `npm run build`

Expected: PASS with no TypeScript errors.

**Step 6: Commit**

```bash
git add src-tauri/src/models.rs src/types/index.ts
git commit -m "feat: define statistics dashboard contracts"
```

### Task 2: Load Statistics Session Rows

**Files:**
- Modify: `src-tauri/src/db/queries.rs`

**Step 1: Write a failing date-filter query test**

Create sessions immediately before and at the cutoff and assert that only the
session at or after the cutoff is returned.

```rust
#[test]
fn statistics_sessions_filter_by_created_at() {
    // Insert old and in-range sessions.
    // Call get_statistics_sessions(Some(cutoff)).
    // Assert only the in-range row is returned.
}
```

**Step 2: Run the focused test**

Run:

```bash
cd src-tauri && cargo test db::queries::tests::statistics_sessions_filter_by_created_at
```

Expected: FAIL because `get_statistics_sessions` does not exist.

**Step 3: Add an internal statistics row**

Define a query-only record containing:

```rust
pub struct StatisticsSessionRow {
    pub agent: String,
    pub project_path: String,
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

**Step 4: Implement the query**

Add:

```rust
pub fn get_statistics_sessions(
    &self,
    created_at_or_after: Option<DateTime<Utc>>,
) -> Result<Vec<StatisticsSessionRow>>
```

Select only the fields above. Apply `created_at >= ?1` when a cutoff exists and
order by `created_at ASC, agent ASC`.

**Step 5: Run database tests**

Run:

```bash
cd src-tauri && cargo test db::queries::tests
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/db/queries.rs
git commit -m "feat: query sessions for statistics"
```

### Task 3: Implement Project Identity Normalization

**Files:**
- Create: `src-tauri/src/statistics.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Write failing normalization tests**

Cover:

```rust
#[test]
fn normalizes_unix_windows_and_encoded_project_paths() {}

#[test]
fn encoded_path_prefers_longest_known_project_suffix() {}

#[test]
fn empty_project_path_becomes_unknown_project() {}
```

Test examples:

- `/Users/maf/My Files/orbit` -> `orbit`
- `C:\Users\maf\My Files\orbit` -> `orbit`
- `-Users-maf-My-Files-orbit` -> `orbit`
- known basename `api-server` plus encoded path
  `-Users-maf-work-api-server` -> `api-server`
- empty and `-` -> `Unknown project`

**Step 2: Run the focused tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests::normalizes
```

Expected: FAIL because the module and normalizer do not exist.

**Step 3: Implement two-pass normalization**

1. Collect readable basenames from paths containing `/` or `\`.
2. Tokenize names using non-alphanumeric runs as separators.
3. For encoded paths, compare token suffixes against readable basenames.
4. Prefer the candidate with the most tokens.
5. Fall back to the last encoded segment.
6. Key groups by lowercase normalized tokens.
7. Preserve the most recently active readable display label.

Keep this logic private to `statistics.rs`.

**Step 4: Register the module**

Add `pub mod statistics;` to `src-tauri/src/lib.rs`.

**Step 5: Run the module tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/statistics.rs src-tauri/src/lib.rs
git commit -m "feat: normalize project identities for statistics"
```

### Task 4: Aggregate Agent Statistics

**Files:**
- Modify: `src-tauri/src/statistics.rs`

**Step 1: Write failing agent aggregation tests**

Test:

- Sessions, messages, and `input + output` totals.
- Cached and reasoning tokens are ignored.
- Average values use zero-safe integer division.
- Agent rows sort by tokens descending, then label.
- Missing models group as `Other`.
- `last_used` uses maximum `updated_at`.
- Seven-day periods create seven daily buckets including empty days.

**Step 2: Run the focused tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests::agent_
```

Expected: FAIL because agent aggregation is not implemented.

**Step 3: Implement period boundaries**

- `7d`: seven UTC calendar-day buckets ending today.
- `30d`: thirty UTC calendar-day buckets ending today.
- `90d`: thirteen UTC week buckets, Monday-based.
- `all`: UTC month buckets from the earliest session through today.

The SQL cutoff is inclusive. Bucket labels are formatted in the frontend from
the returned bucket start timestamp.

**Step 4: Implement agent aggregation**

Build:

- Overall `StatisticsSummary`.
- Stacked session counts per time bucket and agent.
- Agent rows with sessions, messages, tokens, average messages, last used.
- Model rows with token totals and percentages.

Limit chart categories to the top eight by value and combine remaining model
values under `Other`.

**Step 5: Run module tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests
```

Expected: PASS.

**Step 6: Commit**

```bash
git add src-tauri/src/statistics.rs
git commit -m "feat: aggregate agent statistics"
```

### Task 5: Aggregate Project Statistics

**Files:**
- Modify: `src-tauri/src/statistics.rs`

**Step 1: Write failing project aggregation tests**

Test:

- Normal and encoded paths merge into one project.
- Unknown paths form `Unknown project`.
- Project tokens use input plus output only.
- Project agent counts and top agent are correct.
- Agent shares sum to 100% after rounding correction.
- Project rows sort by tokens descending, then display name.
- Timeline values group by normalized project.
- Categories after the top eight combine as `Other`.

**Step 2: Run the focused tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests::project_
```

Expected: FAIL because project aggregation is incomplete.

**Step 3: Implement project aggregation**

Build:

- Overall summary.
- Project rows.
- Up to four project cards.
- Token timeline by project.
- Session counts per project.
- Per-project agent shares.

Use the most recently active readable project label for display.

**Step 4: Run module tests**

Run:

```bash
cd src-tauri && cargo test statistics::tests
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src-tauri/src/statistics.rs
git commit -m "feat: aggregate project statistics"
```

### Task 6: Expose the Statistics Tauri Command

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add the command**

```rust
#[tauri::command]
pub async fn get_statistics(
    mode: StatisticsMode,
    period: StatisticsPeriod,
    state: State<'_, AppState>,
) -> Result<StatisticsDashboard, String>
```

Compute the cutoff, load rows through `DbQueries`, and call the corresponding
statistics aggregator.

**Step 2: Register the command**

Add `commands::get_statistics` to `tauri::generate_handler!`.

**Step 3: Run backend verification**

Run:

```bash
cd src-tauri && cargo test
```

Expected: PASS.

Run:

```bash
cd src-tauri && cargo fmt --check
```

Expected: PASS.

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: expose statistics command"
```

### Task 7: Add Frontend Statistics State And Navigation

**Files:**
- Modify: `src/store/useAppStore.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/Sidebar/Sidebar.tsx`
- Create: `src/components/Statistics/StatisticsDashboard.tsx`

**Step 1: Add statistics state**

Add:

```ts
type AppView = "sessions" | "statistics";

view: AppView;
statisticsMode: StatisticsMode;
statisticsPeriod: StatisticsPeriod;
statistics: StatisticsDashboard | null;
statisticsLoading: boolean;
statisticsError: string | null;
```

Add actions:

- `openStatistics`
- `closeStatistics`
- `setStatisticsMode`
- `setStatisticsPeriod`
- `loadStatistics`

**Step 2: Implement loading behavior**

Invoke:

```ts
invoke<StatisticsDashboard>("get_statistics", {
  mode: get().statisticsMode,
  period: get().statisticsPeriod,
})
```

Discard stale responses when mode or period changes before a request finishes.
After `reindex`, refresh statistics when the Statistics view is active.

**Step 3: Add the full-window view switch**

In `App.tsx`, render `StatisticsDashboard` instead of the complete
sidebar/resizer/transcript layout when `view === "statistics"`.

Do not clear session selection, filters, sorting, messages, or sidebar width.

**Step 4: Add the entry control**

Add a Statistics button to the sidebar footer beside the existing sync and
settings controls, using a Lucide chart icon and an accessible label.

**Step 5: Add the dashboard shell**

`StatisticsDashboard.tsx` contains:

- Back to Sessions control.
- Title.
- Agent stats / Project stats tabs.
- Shared period segmented control.
- Loading, error with retry, and empty states.

**Step 6: Verify**

Run: `npm run build`

Expected: PASS.

**Step 7: Commit**

```bash
git add src/store/useAppStore.ts src/App.tsx src/components/Sidebar/Sidebar.tsx src/components/Statistics/StatisticsDashboard.tsx
git commit -m "feat: add statistics navigation and state"
```

### Task 8: Build Formatting And SVG Chart Primitives

**Files:**
- Create: `src/components/Statistics/statisticsFormat.ts`
- Create: `src/components/Statistics/statisticsFormat.test.ts`
- Create: `src/components/Statistics/charts/chartGeometry.ts`
- Create: `src/components/Statistics/charts/chartGeometry.test.ts`
- Create: `src/components/Statistics/charts/ChartLegend.tsx`
- Create: `src/components/Statistics/charts/StackedBarChart.tsx`
- Create: `src/components/Statistics/charts/HorizontalBarChart.tsx`
- Create: `src/components/Statistics/charts/PercentStackedBarChart.tsx`
- Create: `src/components/Statistics/charts/DonutChart.tsx`

**Step 1: Write failing helper tests**

Test:

- Compact numbers: `999`, `1.2k`, `3.4M`, `1.5B`.
- Token and percentage formatting.
- Relative timestamps.
- Nice axis maximum calculation.
- Zero-safe stacked segment widths.
- Donut arcs with empty and single-value datasets.

Follow the existing TypeScript test style used by
`src/components/Transcript/messageVisibility.test.ts`.

**Step 2: Run TypeScript compilation**

Run: `npm run build`

Expected: FAIL until helpers are implemented.

**Step 3: Implement formatting and geometry helpers**

Keep pure calculations outside React components. Use SVG `viewBox` coordinates
so charts resize without reading DOM dimensions.

**Step 4: Implement chart components**

Requirements:

- Orbit theme colors via CSS variables.
- Stable agent colors from a new plain-color map in `src/types/index.ts`.
- Neutral `Other` color.
- Keyboard-focusable data marks.
- Native SVG `<title>` values plus a styled hover/focus tooltip.
- Text fallback or empty state for no values.
- Reduced label density for narrow widths.

**Step 5: Verify**

Run: `npm run build`

Expected: PASS.

**Step 6: Commit**

```bash
git add src/components/Statistics src/types/index.ts
git commit -m "feat: add statistics chart primitives"
```

### Task 9: Build The Agent Dashboard

**Files:**
- Create: `src/components/Statistics/AgentStatistics.tsx`
- Create: `src/components/Statistics/StatCard.tsx`
- Create: `src/components/Statistics/StatisticsTable.tsx`
- Modify: `src/components/Statistics/StatisticsDashboard.tsx`

**Step 1: Render summary cards**

Show:

- Sessions.
- Messages and average per session.
- Total tokens and average per session.
- Active agents and project count.

**Step 2: Render charts**

- Sessions over time: stacked by agent.
- Tokens by agent: horizontal bars.
- Token split by model: donut.

**Step 3: Render the leaderboard**

Columns:

- Agent.
- Sessions.
- Messages.
- Tokens.
- Average messages.
- Last used.

Use a responsive overflow container at narrow widths.

**Step 4: Verify**

Run: `npm run build`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/components/Statistics
git commit -m "feat: build agent statistics dashboard"
```

### Task 10: Build The Project Dashboard

**Files:**
- Create: `src/components/Statistics/ProjectStatistics.tsx`
- Create: `src/components/Statistics/ProjectCard.tsx`
- Modify: `src/components/Statistics/StatisticsDashboard.tsx`

**Step 1: Render project cards**

Show up to four cards with:

- Project label.
- Sessions.
- Tokens.
- Last active.
- Agent mix bar and percentages.

**Step 2: Render charts**

- Tokens over time by project.
- Sessions by project.
- Agent mix per project.

**Step 3: Render project comparison**

Columns:

- Project.
- Sessions.
- Tokens.
- Agents.
- Top agent.
- Last active.

**Step 4: Verify**

Run: `npm run build`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/components/Statistics
git commit -m "feat: build project statistics dashboard"
```

### Task 11: Integrate Theme, Accessibility, And Responsive Layout

**Files:**
- Modify: `src/index.css`
- Modify: `src/components/Statistics/**/*.tsx`

**Step 1: Add chart theme tokens**

Add only reusable semantic values that cannot be expressed with existing
tokens, such as chart grid and tooltip surface colors. Do not create a second
parallel theme.

**Step 2: Audit interaction states**

Verify:

- Visible keyboard focus for back, tabs, periods, retry, and chart marks.
- Selected tabs and periods expose `aria-selected` or `aria-pressed`.
- Every chart has an accessible title.
- Tables retain semantic table markup.
- Color is not the only way series are identified; labels remain visible.

**Step 3: Audit responsive behavior**

Verify at:

- 900px wide.
- 1200px wide.
- 1600px wide.

Cards and paired charts collapse to one column when space is insufficient.

**Step 4: Commit**

```bash
git add src/index.css src/components/Statistics
git commit -m "style: polish statistics dashboards"
```

### Task 12: Final Verification

**Files:**
- Review all modified statistics files.

**Step 1: Run backend tests**

Run:

```bash
cd src-tauri && cargo test
```

Expected: PASS.

**Step 2: Run Rust formatting**

Run:

```bash
cd src-tauri && cargo fmt --check
```

Expected: PASS.

**Step 3: Run frontend build**

Run:

```bash
npm run build
```

Expected: PASS.

**Step 4: Run whitespace verification**

Run:

```bash
git diff --check
```

Expected: no output.

**Step 5: Inspect the application**

Run:

```bash
npm run tauri dev
```

Verify:

- Statistics opens as a full-window view.
- Returning preserves session-browser state.
- Both dashboard tabs load.
- All four periods work.
- Empty periods render correctly.
- Reindex refreshes visible statistics.
- Project names merge normal and encoded paths.
- Agent and project colors match Orbit's theme.
- Charts remain usable at narrow and wide window sizes.

**Step 6: Commit any verification fixes**

```bash
git add <fixed-files>
git commit -m "fix: address statistics verification findings"
```

