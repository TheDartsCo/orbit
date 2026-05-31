# Orbit — Design Specification

## 1. Overview

**Orbit** is a local-first, cross-platform desktop application for browsing, searching, and managing AI coding agent sessions. It provides a unified view across multiple agents (Claude Code, Codex, Cursor, OpenCode) with a polished, Linear-like UI.

**Target user:** Individual developers using multiple AI coding agents on their local machine.

**Key principles:** Local-only (no cloud dependency), privacy-first, low resource usage, fast.

## 2. Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri v2 |
| Backend | Rust |
| Frontend | React + TypeScript |
| Styling | Tailwind CSS |
| Database | SQLite (bundled via rusqlite) |
| Full-text search | SQLite FTS5 |
| File watching | notify crate |
| Async runtime | tokio |
| State management | Zustand |
| Virtual lists | @tanstack/react-virtual |
| Markdown | react-markdown + rehype-highlight |
| Icons | lucide-react |

## 3. System Architecture

```
Tauri v2 Window
├── React/TypeScript UI (frontend)
│   ├── Sidebar (sessions, filters, search)
│   └── MainContent (transcript viewer, action bar)
│       ↕ Tauri IPC (invoke commands)
└── Rust Backend
    ├── AdapterRegistry → ClaudeAdapter, CodexAdapter, CursorAdapter, OpenCodeAdapter
    ├── Indexer (scan → parse → SQLite + FTS5)
    ├── Watcher (notify → incremental re-index)
    ├── Resume (clipboard copy + terminal launch)
    └── SQLite (app DB + FTS5 index)
```

All data access goes through Tauri IPC. The frontend never touches files or databases directly.

## 4. Data Model

### 4.1 Normalized Entities

```rust
struct Session {
    id: String,
    agent: AgentType,     // claude, codex, cursor, opencode
    title: String,
    project_path: String,
    created_at: DateTime,
    updated_at: DateTime,
    file_path: String,
    is_active: bool,
    message_count: u32,
}

struct Message {
    id: String,
    session_id: String,
    role: MessageRole,     // User, Assistant, System, Tool
    content: String,
    timestamp: DateTime,
    sequence: u32,
    tool_name: Option<String>,
    tool_input: Option<String>,
    tool_output: Option<String>,
}

struct Attachment {
    id: String,
    message_id: String,
    attachment_type: AttachmentType, // Image, File, Diff
    path: String,
    mime_type: Option<String>,
}
```

### 4.2 SQLite Schema

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    title TEXT,
    project_path TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    file_path TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    message_count INTEGER NOT NULL DEFAULT 0,
    source_hash TEXT
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    content TEXT,
    timestamp TEXT,
    sequence INTEGER NOT NULL,
    tool_name TEXT,
    tool_input TEXT,
    tool_output TEXT
);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    attachment_type TEXT NOT NULL,
    path TEXT NOT NULL,
    mime_type TEXT
);

CREATE INDEX idx_sessions_agent ON sessions(agent);
CREATE INDEX idx_sessions_project ON sessions(project_path);
CREATE INDEX idx_sessions_updated ON sessions(updated_at DESC);
CREATE INDEX idx_messages_session ON messages(session_id, sequence);

CREATE VIRTUAL TABLE messages_fts USING fts5(
    content, tool_input, tool_output,
    content=messages, content_rowid=rowid
);
```

`source_hash` on sessions enables incremental re-indexing — only re-parse changed files. FTS5 uses the content table pattern for efficient sync. All timestamps as ISO 8601 text. Cascade deletes keep things clean.

## 5. Adapter System

### 5.1 AgentAdapter Trait

```rust
#[async_trait]
trait AgentAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    async fn detect(&self) -> Result<bool>;
    async fn scan(&self) -> Result<Vec<SessionLocation>>;
    async fn parse_session(&self, path: &Path) -> Result<NormalizedSession>;
    fn resume_command(&self, session_id: &str, project_path: &str) -> String;
    async fn is_active(&self, session_path: &Path) -> Result<bool>;
}

struct SessionLocation {
    path: PathBuf,
    last_modified: DateTime,
}

struct NormalizedSession {
    session: Session,
    messages: Vec<Message>,
    attachments: Vec<Attachment>,
}
```

### 5.2 MVP Adapters

| Adapter | Storage Location | Format | Active Check |
|---------|-----------------|--------|-------------|
| Claude Code | `~/.claude/projects/*/sessions/*.jsonl` | JSONL | Process/file lock |
| Codex | `~/.codex/sessions/` (verify during impl) | TBD | Process check |
| Cursor | `~/.cursor/` SQLite DBs | SQLite query | Process check |
| OpenCode | OpenCode data dir | JSONL | Process check |

### 5.3 AdapterRegistry

Static registration at compile time. `detect_available()` called at startup to discover installed agents. Only detected agents appear in UI filters. New agents require a new adapter file + one `register()` call.

## 6. Indexer & File Watching

### 6.1 Indexing Flow

1. On startup: `detect_available()` → get installed agents
2. For each adapter: `scan()` → get session locations
3. For each location: check `source_hash` (mtime + size)
4. If changed/new: `parse_session()` → upsert session + messages + FTS5
5. If unchanged: skip
6. Mark sessions not found in scan as deleted

### 6.2 File Watching

- Watch each adapter's session directory root via `notify`
- Events: Create (index), Modify (re-index), Remove (mark deleted)
- 500ms debounce window for rapid writes
- Active session polling every 5s
- Exclude hidden dirs, `.git`, `node_modules`, temp files

### 6.3 Performance Targets

| Operation | Target |
|-----------|--------|
| Initial scan (1000 sessions) | < 3s |
| Incremental re-index (1 session) | < 100ms |
| Search query | < 50ms |
| Session list load | < 100ms |
| Transcript render (500 messages) | < 200ms |

## 7. Frontend

### 7.1 Layout

Sidebar + Detail view (email-client pattern):
- **Sidebar:** Search input, filter bar (agent, date, project), scrollable session list with agent badges and active indicators
- **Detail:** Chat-style transcript with user/assistant messages, collapsible tool calls, markdown + code highlighting, image previews
- **Action bar:** Copy resume command, launch resume in terminal, session metadata

### 7.2 State Management

Zustand store with actions for: loadSessions, selectSession, search, setFilters, resumeSession.

### 7.3 Key UI Patterns

- Virtualized lists for session sidebar and transcript (@tanstack/react-virtual)
- Markdown rendering via react-markdown + rehype-highlight
- Lazy-loaded messages in pages of 100
- Color-coded agent badges (Claude=purple, Codex=green, Cursor=blue, OpenCode=orange)
- Green dot for active sessions, refreshed every 5s

### 7.4 Tauri IPC Commands

`get_sessions`, `get_session_messages`, `search_sessions`, `get_resume_command`, `launch_resume`, `get_active_sessions`, `reindex_all`

## 8. Resume Workflow

1. User clicks "Resume" on a session
2. Adapter generates the resume command string
3. "Copy" button copies to clipboard
4. "Launch" button opens system terminal (macOS: Terminal.app / iTerm2, Linux: xterm/gnome-terminal, Windows: Windows Terminal) with the command pre-filled

## 9. Project Structure

```
orbit/
├── src-tauri/src/
│   ├── main.rs, lib.rs
│   ├── commands.rs
│   ├── db/ (mod, schema, queries)
│   ├── adapters/ (mod, claude, codex, cursor, opencode)
│   ├── indexer/ (mod, fts)
│   ├── watcher.rs
│   ├── models.rs
│   └── resume.rs
├── src/ (React)
│   ├── components/ (Sidebar/, Transcript/, ActionBar/, common/)
│   ├── store/ (useAppStore.ts)
│   ├── hooks/ (useInvoke, useSearch)
│   ├── types/
│   └── styles/
├── Cargo.toml, package.json, vite.config.ts, tauri.conf.json
```

## 10. MVP Scope

**In scope:**
- Tauri v2 + Rust + React + TypeScript + SQLite + FTS5
- 4 adapters: Claude Code, Codex, Cursor, OpenCode
- Session browser with sidebar + detail layout
- Chat-style transcript viewer
- Full-text search with filters
- Active session indicator
- Copy + launch resume commands
- File watching with incremental indexing

**Out of scope (future phases):**
- Semantic search / local embeddings
- Live transcript streaming
- Full cockpit dashboard
- Analytics
- Image browser
- Additional agents
- CLI version
- Auto-updates
- Crash reporting
