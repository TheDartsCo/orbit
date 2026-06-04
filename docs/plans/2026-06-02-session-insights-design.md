# Session Insights — Design

Extract four pieces of session-level metadata and surface them in the sidebar so users can answer "what did I work on, on which model, at what cost, in which branch?" without opening the transcript.

## Scope

| Insight | Source | Universal? |
|---------|--------|------------|
| Files touched | tool calls across all 5 adapters | Yes |
| Model | adapter-specific field | Mostly (Cursor sometimes missing) |
| Token usage | Claude `usage`, Codex `token_count` event, OpenCode `part.tokens` | Partial — Warp + Cursor not exposed |
| Git branch | Codex `gitInfo`, Claude `gitBranch` | Partial |

**Out of scope:** dollar cost calculation, subagent tree UI, file-based filter UI.

## Per-adapter extraction

### Claude
- Files: `tool_use.input` — map tool name → op (`Read` → read, `Edit`/`MultiEdit` → edit, `Write`/`NotebookEdit` → write). Skip `Bash` (string command).
- Model: `message.model` on assistant entries — first non-empty wins for the session.
- Tokens: `message.usage` — sum `input_tokens`/`output_tokens`/`cache_read_input_tokens`/`cache_creation_input_tokens` across all assistant messages.
- Branch: `gitBranch` on session header.

### Codex
- Files: `function_call.arguments` for tool calls. `apply_patch` extracts paths via regex `*** (Update|Add) File: <path>`.
- Model: `response_item.payload.model`.
- Tokens: `event_msg` with `payload.type == "token_count"` — sum `input_tokens`/`output_tokens`/`cached_input_tokens`/`reasoning_tokens`.
- Branch: `session_meta.payload.gitInfo.branch`.

### Cursor
- Files: `tool_use.input.path` — map tool name → op.
- Model: best-effort, often missing → null.
- Tokens: not exposed.
- Branch: not exposed.

### OpenCode
- Three parsing paths (storage JSON, SQLite, legacy JSONL). All need updating.
- Files: `part.state.input` for tool parts.
- Model: `part.data.modelID` (legacy) or session metadata (db).
- Tokens: `part.data.tokens` (legacy) or column in db.
- Branch: not in session.

### Warp
- Files: `ReadFiles.file_paths`, `ApplyFileDiff.file_path`.
- Model: `TaskEntry.model_id` — first non-empty wins.
- Tokens: not in protobuf.
- Branch: not in protobuf.

## Schema

```sql
ALTER TABLE sessions ADD COLUMN model TEXT;
ALTER TABLE sessions ADD COLUMN git_branch TEXT;
ALTER TABLE sessions ADD COLUMN input_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN output_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN cached_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN reasoning_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sessions ADD COLUMN file_count INTEGER NOT NULL DEFAULT 0;

CREATE TABLE session_files (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    operation TEXT NOT NULL,
    touch_count INTEGER NOT NULL DEFAULT 1,
    first_touched_sequence INTEGER,
    PRIMARY KEY (session_id, file_path, operation)
);

CREATE INDEX idx_session_files_path ON session_files(file_path);
```

Migration uses `add_column_if_missing` helper following the `parent_session_id` pattern at `schema.rs:47-55`.

## Aggregator shape

```rust
pub struct SessionMetadata {
    pub model: Option<String>,
    pub git_branch: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub reasoning_tokens: u64,
    pub files: Vec<FileTouch>,
}

pub struct FileTouch {
    pub path: String,
    pub operation: String,
}
```

Each parser returns `SessionMetadata` alongside `NormalizedSession`. The indexer dedupes by `(path, operation)` and upserts.

## Frontend

### `SessionItem.tsx` chips

```
[claude] Fix auth bug
🤖 Claude Sonnet 4.5    🌿 feat/auth    📄 8 files    12.4k → 3.1k tokens
```

- Model chip: abbreviated, colored via `AGENT_TEXT_COLORS`.
- Branch chip: only if non-null. Click → filter.
- File chip: only if `file_count > 0`. Click → popover with full list.
- Token chip: only if `input_tokens + output_tokens > 0`. Formatted with `Intl.NumberFormat`.

### `FilterBar.tsx`

New dimension: `gitBranch`. Backend `get_sessions` adds `AND git_branch = ?` when set.

### `useAppStore.ts`

Add `gitBranch: string | null` to filters. Pass through to `get_sessions` invoke.

## Verification

- `cargo test` from `src-tauri/` — all existing + new extraction tests
- `tsc --noEmit` — type check
- `npm run build` — full TS + Vite build
- No frontend tests configured

## Migration safety

Idempotent column adds via `add_column_if_missing`. New table uses `IF NOT EXISTS`. Re-runs are no-ops.
