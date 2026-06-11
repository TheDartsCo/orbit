# Model Statistics Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a third Model stats dashboard with backend-owned model
aggregation and reusable Orbit statistics components.

**Architecture:** Extend the existing statistics request and response contracts
with a model mode. Aggregate leaderboard and timeline data in Rust, then render
the new mode with the existing cards, stacked bars, horizontal bars, mix bars,
and table components.

**Tech Stack:** Rust, Tauri v2, React 19, TypeScript, Tailwind CSS v4, inline SVG

---

### Task 1: Extend Statistics Contracts

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src/types/index.ts`

1. Add `Model` / `"model"` to `StatisticsMode`.
2. Expand `ModelStatisticsRow` with sessions, messages, agent count, top agent,
   last used, and agent mix.
3. Add a tagged `Model` dashboard response containing summary, timeline, and
   models.
4. Mirror the response in TypeScript.
5. Run `npm run build`.

### Task 2: Aggregate Model Statistics

**Files:**
- Modify: `src-tauri/src/statistics.rs`

1. Write failing tests for unknown models, exact-name separation, totals, top
   agent, and timeline grouping.
2. Run the focused tests and verify failure.
3. Implement model aggregation and chart-category limiting.
4. Run `cargo test statistics::tests`.

### Task 3: Add Model Dashboard UI

**Files:**
- Create: `src/components/Statistics/ModelStatistics.tsx`
- Modify: `src/components/Statistics/StatisticsDashboard.tsx`
- Modify: `src/components/Statistics/charts/PercentStackedBarChart.tsx`

1. Add the Model stats tab.
2. Render summary cards.
3. Render sessions-over-time by model.
4. Render tokens-by-model.
5. Generalize the percent-stacked chart for model agent mixes.
6. Render the model leaderboard.
7. Run `npm run build`.

### Task 4: Verify

1. Run `cargo test`.
2. Run the existing TypeScript helper tests.
3. Run `npm run build`.
4. Run `git diff --check`.
5. Inspect Agent, Model, and Project tabs at wide and narrow widths.

