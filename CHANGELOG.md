# Changelog

## v0.4.0

### New Agent Adapter

- **Antigravity** — parses JSONL transcripts from `~/.gemini/antigravity/brain/`; extracts user requests from `<USER_REQUEST>` blocks, planner responses with thinking, tool calls with file operations, and token usage estimates

## v0.1.0 — Initial Release

First public release of Orbit, a native desktop app for browsing AI coding agent session history.

### Agent Adapters

- **Claude Code** — scans `~/.claude/projects/` JSONL files; filters out Claude-Mem plugin/subagent sessions automatically
- **Codex** — scans `~/.codex/` JSONL sessions
- **Cursor** — parses Anthropic-style JSONL transcripts from `~/.cursor/projects/`; infers project paths from encoded directory names
- **OpenCode** — supports three storage formats: legacy JSONL (`sessions/`), current storage layout (`storage/session/`), and database-backed sessions via `opencode.db`
- **Warp** — reads protobuf-encoded agent tasks from Warp's local SQLite database
- **GitHub Copilot CLI** — parses conversation history from Copilot's local storage
- **Qoder** — scans Qoder session files with full transcript parsing

### Session Indexing

- Automatic detection of installed agents on macOS
- SQLite database with WAL mode for the local index
- FTS5 full-text search index on message content, tool inputs, and tool outputs
- Change detection via size + mtime hashing to skip unchanged session files
- Parser versioning to force re-parse when adapter logic changes
- Stale session cleanup after each full scan
- Per-provider sync stats tracking

### Session Browsing

- Searchable, filterable session list with virtual scrolling for large histories
- Filter by agent type, project path, model, git branch, and active status
- Multi-agent filter chips for quick toggling
- Search highlights in session titles and transcript content
- Session metadata display: message count, file count, token usage, model, git branch

### Transcript Viewer

- Chat-style transcript with user, assistant, and tool messages
- Markdown rendering with syntax highlighting (via `react-markdown` + `rehype-highlight`)
- Collapsible tool call blocks showing tool name, inputs, and outputs
- Message role filter (All / User / Assistant / Tool)
- In-transcript search with match navigation
- Virtualized rendering for long transcripts

### Resume Sessions

- Copy resume command to clipboard
- Launch resume directly in your preferred terminal (Terminal.app, iTerm2, Warp, Ghostty)
- Configurable preferred terminal in settings

### UI

- Dark theme with custom design tokens
- Resizable sidebar
- Active session indicators with 5-second polling
- Sync status modal showing per-provider indexing stats and last sync time
- Custom app icon

### Technical Stack

- **Frontend**: React 19, TypeScript, Tailwind CSS v4, Vite, Zustand, TanStack Virtual
- **Backend**: Rust, Tauri v2, SQLite (rusqlite), FTS5, protobuf (prost)
- **Platform**: macOS-first (Linux and Windows builds not tested)

### Known Limitations

- macOS is the only tested platform
- App bundles are not signed or notarized
- Live file watching is defined but not wired into the app loop
- Session formats may change when agent vendors update their tools
