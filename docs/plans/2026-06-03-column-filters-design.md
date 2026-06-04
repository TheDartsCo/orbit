# Column Filters Design

## Goal

Replace the long top-bar agent chips with column-attached filters and move the branch selector into the Branch column so filtering matches the table structure.

## Design

The top bar keeps global search and app actions only. Column-level filters live in the table header:

- Agent: compact popover checklist with all providers, counts, and all/none controls.
- Branch: select directly below the Branch label with `All branches` as the default.
- Session, Project, Model: inline text filters below their labels.
- Other columns remain sortable without filters for this pass.

Filters update the existing Zustand store and reload sessions through the existing `get_sessions` command. Sorting remains on header label clicks, while filter controls stop event propagation so typing/selecting does not change sort.

## Implementation Plan

1. Extend frontend filter types and store helpers for `title`, `model`, and branch/agent updates.
2. Extend Rust `SessionFilters` and `DbQueries::get_sessions` to support title and model LIKE filters.
3. Replace `FilterBar` usage in the sidebar header with a table header that renders per-column controls.
4. Reuse existing agent color/label constants for the Agent popover.
5. Verify with `npm run build` and fix any TypeScript issues.
