# AGENTS.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## What is Orbit

Orbit is a **Tauri v2 desktop app** that aggregates and displays coding agent session transcripts from multiple AI tools (Claude Code, Codex, Cursor, OpenCode). It indexes session files from each tool's local storage into a unified SQLite database, then presents them in a searchable, filterable UI with transcript viewing and session resume capabilities.

## Build & Dev Commands

- `npm run tauri dev` — Run the full app in dev mode (starts Vite frontend + Rust backend)
- `npm run dev` — Start only the Vite frontend dev server (port 1420)
- `npm run build` — TypeScript check + Vite production build (frontend only)
- `npm run tauri build` — Full production build (frontend + Rust, produces native binary)
- `cargo build` — Build only the Rust backend (run from `src-tauri/`)
- `cargo test` — Run Rust tests (run from `src-tauri/`)
- `tsc --noEmit` — TypeScript type check without emitting

There is no linter, formatter, or test runner configured for the frontend. There are no frontend tests.

## Architecture

### Two-process Tauri model

The app follows Tauri v2's split architecture:
- **Frontend** (`src/`): React 19 + TypeScript + Tailwind CSS v4 + Vite. Communicates with the backend exclusively via `invoke()` from `@tauri-apps/api/core`.
- **Backend** (`src-tauri/`): Rust. Exposes Tauri commands that the frontend calls. Manages all data access, file system scanning, and session parsing.

### Backend (Rust) — key modules

- **`adapters/`** — One adapter per agent tool (Claude, Codex, Cursor, OpenCode). Each implements the `AgentAdapter` trait which defines: `detect()` (is the tool installed?), `scan()` (find session files on disk), `parse_session()` (parse a session file into normalized `NormalizedSession`), `resume_command()`, and `is_active()`. The `AdapterRegistry` holds all adapters in a HashMap keyed by string ID.
- **`adapters/claude.rs`** — Filters out Claude-Mem plugin observer sessions by checking if the first user message contains "claude-mem" or "Hello memory agent". These return `Err("Subagent/plugin session, skipping")` from `parse_session()`.
- **`indexer/`** — Orchestrates scanning all adapters, parsing sessions, upserting into SQLite, managing stale session cleanup, and rebuilding the FTS5 index. Uses a size+mtime hash to skip unchanged files. **Important**: `mark_stale_sessions()` is called ONCE after ALL adapters finish (not per-adapter), using combined paths from every adapter — otherwise each adapter would delete the previous adapter's sessions.
- **`db/`** — `schema.rs` creates tables (sessions, messages, attachments) and a `messages_fts` FTS5 virtual table. `queries.rs` provides `DbQueries` for all SQL operations. The database lives at `~/<platform data dir>/orbit/orbit.db` with WAL mode enabled.
- **`commands.rs`** — Tauri command handlers (`#[tauri::command]`). These are the IPC boundary: `get_sessions`, `get_session_messages`, `search_sessions`, `get_resume_command`, `launch_resume`, `get_active_sessions`, `reindex_all`.
- **`watcher.rs`** — File system watcher using the `notify` crate (kqueue on macOS). Currently defined but not wired into the main app loop.
- **`models.rs`** — Shared data types: `Session`, `Message`, `Attachment`, `AgentType`, `MessageRole`, `SessionFilters`. These are serialized to/from JSON for the frontend via serde.

### Frontend (React/TS) — key patterns

- **State management**: Single Zustand store (`src/store/useAppStore.ts`) holds all app state and async actions. Components select slices from the store. Key state: `sessions`, `messages`, `filters`, `messageRoleFilter`, `indexError`, `indexStats`, `loading`, `activeSessionIds`.
- **IPC**: All backend calls go through `invoke()` directly in the store or components. The `useInvoke` hook exists but the store uses `invoke` from `@tauri-apps/api/core` directly.
- **Virtualization**: Both `SessionList` and `TranscriptView` use `@tanstack/react-virtual` for efficient rendering of long lists.
- **Search highlighting**: Shared `Highlight` component (`src/components/common/Highlight.tsx`) wraps matching text in `<mark>` tags. Used by `SessionItem` (title), `MarkdownRenderer` (message content via react-markdown `text` component), and `ToolCall` (input/output).
- **Message role filters**: TranscriptView has All/user/assistant/tool filter buttons. `messageRoleFilter` in the store filters the virtual list client-side. Resets on session change.
- **Types**: `src/types/index.ts` mirrors the Rust models. `AgentType` and `MessageRole` are string literal unions. `AGENT_COLORS`, `AGENT_TEXT_COLORS`, `AGENT_TINTS`, and `AGENT_LABELS` maps live here.
- **Styling**: Tailwind v4 with custom theme tokens defined in `src/index.css` via `@theme` (dark theme: `bg-primary`, `bg-secondary`, `text-primary`, `accent`, etc.).
- **Layout**: `App.tsx` → resizable Sidebar (left) + main area (TranscriptView + ActionBar). Sidebar contains SearchBar, FilterBar, and SessionList.

### Data flow

1. On startup, `loadSessions()` calls `get_sessions` Tauri command → Rust queries SQLite → returns `Session[]`.
2. Active session polling runs every 5 seconds via `refreshActiveSessions` → `get_active_sessions`.
3. Selecting a session calls `get_session_messages` → loads `Message[]` into the store. `messageRoleFilter` resets to null.
4. Reindex button triggers `reindex_all` → Rust scans all adapters, parses new/changed session files, upserts to DB, rebuilds FTS. Status bar shows "Indexed X of Y sessions". Errors shown in the empty state.
5. Search uses LIKE queries in `get_sessions` (not FTS5 MATCH) — searches across `title` and message `content`/`tool_input`/`tool_output`. Keywords are highlighted in the session list and transcript via the `Highlight` component.

### Adding a new agent adapter

1. Create `src-tauri/src/adapters/<name>.rs` implementing the `AgentAdapter` trait.
2. Add `pub mod <name>;` to `src-tauri/src/adapters/mod.rs`.
3. Register it in `AdapterRegistry::new()`.
4. Add the variant to `AgentType` enum in `models.rs` (with `as_str`/`from_str` match arms).
5. Add the corresponding type and labels in `src/types/index.ts` (`AgentType` union, `AGENT_COLORS`, `AGENT_LABELS`).

### Session file formats by adapter

- **Claude**: JSONL files in `~/.claude/projects/<project>/`, each line has a `type` field (`summary`, `user`/`human`, `assistant`, `tool_result`). Subagent files live in `<project>/<uuid>/subagents/` but are excluded by scan (only top-level `.jsonl` files are scanned). Claude-Mem plugin sessions are filtered by content detection in `parse_session()`.
- **Codex**: JSONL files in `~/.codex/`, uses `role` field (`user`, `assistant`, `system`, `tool`).
- **Cursor**: Scans `~/.cursor/projects/<encoded-project>/agent-transcripts/<session-uuid>/<session-uuid>.jsonl` files. Parses Anthropic-style JSONL (`role` + `message.content[]` with `text`/`tool_use` blocks). Skips `subagents/` subdirs. Parser version is 2 (bumped when the stub was replaced).
- **OpenCode**: JSONL in config or data dir under `opencode/sessions/`. Format similar to Codex.

### Database

SQLite with WAL mode. Three tables: `sessions`, `messages`, `attachments`. FTS5 virtual table `messages_fts` indexes `content`, `tool_input`, `tool_output` from messages. The `source_hash` column on sessions stores a `"size:mtime"` string for change detection. Search queries use LIKE (not FTS5 MATCH) for robustness with arbitrary user input.

### Known issues

- 4 minor Rust warnings (unused imports/variables) — no errors
- Cursor adapter used to be a stub (returns empty messages, only finds `ai-tracking.db`); now parses real JSONL session files
- File watcher (`watcher.rs`) is defined but not wired into the app loop
