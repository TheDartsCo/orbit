# Warp Agent Adapter Design

## Overview

Add a new agent adapter to Orbit that reads Warp terminal's AI agent sessions from its local SQLite database. Unlike other adapters that parse JSONL/JSON files, Warp stores conversations as protobuf-encoded blobs in SQLite, requiring a different parsing strategy.

## Data Source

- **Location**: `~/Library/Group Containers/2BBY89MBSN.dev.warp/Library/Application Support/dev.warp.Warp-Stable/warp.sqlite`
- **Format**: SQLite database with protobuf BLOBs
- **Key tables**:
  - `agent_conversations` — one row per conversation, JSON metadata (usage stats, model info, tool counts)
  - `agent_tasks` — one+ rows per conversation, protobuf-encoded `task` BLOB with full message history

## Session Granularity

One Orbit session = one `agent_task` row. A Warp conversation can have multiple tasks, but each task is a self-contained exchange with its own message sequence.

## Protobuf Schema

The `task` BLOB is a protobuf message with repeated fields representing a sequence of turns:

```
AgentTask {
  field 1:  task_id (string, UUID)
  field 2:  UserMessage (repeated) — user prompt with context
  field 3:  AssistantText (repeated) — assistant text response
  field 4:  ToolCall (repeated) — tool invocation
  field 5:  ToolResult (repeated) — tool execution result
  field 6:  ThinkingMessage (repeated) — short reasoning (base64)
  field 11: conversation_id (string, UUID)
  field 13: model_id (string)
  field 14: Timestamp { seconds, nanos }
  field 15: DetailedThinking (repeated) — chain-of-thought reasoning
}
```

### Sub-messages

**UserMessage (field 2)**:
```
{
  field 1: text (string)
  field 2: context {
    field 1: { path (working dir), home_dir, is_tty }
    field 8[]: { project_name, project_path }
    field 11: { git_branch }
  }
  field 4: session_id
  field 5: type (int32)
}
```

**AssistantText (field 3)**:
```
{ field 1: text (string) }
```

**ToolCall (field 4)**:
```
{
  field 1: tool_call_id (string)
  field 2: RunCommand { command, is_background, exit_code }
  field 5: ReadFiles { file_paths[] }
  field 6: ApplyFileDiff { description, file_diffs[] }
  field 9: GrepSearch { patterns[], path }
}
```

**ToolResult (field 5)**:
```
{
  field 1: tool_call_id (string)
  field 2: CommandResult { output, exit_code }
  field 5: ReadFilesResult { files[] { path, content, line_numbers } }
  field 6: ApplyFileDiffResult { diffs[] }
  field 9: GrepResult { matches[] }
}
```

## Message Mapping

| Protobuf field | Orbit MessageRole | Notes |
|---|---|---|
| UserMessage (2) | `User` | Extract text from field 1 |
| AssistantText (3) | `Assistant` | Extract text from field 1 |
| ToolCall (4) | `Tool` | `tool_name` from sub-message type, `tool_input` from command/query |
| ToolResult (5) | (backfill) | Match by `tool_call_id`, set `tool_output` on previous Tool message |
| ThinkingMessage (6, 15) | Skip | Internal reasoning, not displayed |

## Parsing Strategy

Since protobuf fields repeat (multiple field 2s, 3s, 4s per blob), we use `prost` to decode into a struct with repeated fields. The order of fields in the wire format preserves the conversation order.

We define a `WarpTask` wrapper that collects repeated fields:
- `repeated UserMessage user_messages`
- `repeated AssistantText assistant_texts`
- `repeated ToolCall tool_calls`
- `repeated ToolResult tool_results`

Messages are interleaved in order of appearance, mapped to `NormalizedSession` messages.

## Detection & Scanning

- **detect()**: Check if the SQLite database file exists at the expected path
- **scan()**: Query `agent_tasks` table, return one `SessionLocation` per row with `last_modified_at` as the modification time
- **parse_session()**: Open the SQLite DB, query the specific `task` BLOB by `task_id`, decode protobuf, build `NormalizedSession`

## Title & Project Path

- **Title**: First user message text, truncated to 100 characters
- **Project path**: Extracted from `UserMessage.context.project_path` inside the protobuf

## Resume Command

`open -a Warp` — Warp doesn't expose a CLI resume mechanism. The session ID is passed for reference but Warp doesn't support resuming specific agent sessions via command line.

## Active Session Detection

Check if any Warp process is running that has recently modified the SQLite database (compare file mtime of `warp.sqlite` against a recent threshold).

## Files to Create/Modify

| File | Change |
|---|---|
| `src-tauri/Cargo.toml` | Add `prost`, `prost-types`, `rusqlite` dependencies |
| `src-tauri/src/adapters/warp.rs` | New — Warp adapter implementing `AgentAdapter` trait |
| `src-tauri/src/adapters/mod.rs` | Add `pub mod warp;` + register `WarpAdapter` in `AdapterRegistry::new()` |
| `src-tauri/src/models.rs` | Add `Warp` variant to `AgentType` enum with `as_str`/`from_str` match arms |
| `src/types/index.ts` | Add `"warp"` to `AgentType` union + `AGENT_COLORS`, `AGENT_TEXT_COLORS`, `AGENT_TINTS`, `AGENT_LABELS` maps |

## Frontend Styling

```typescript
AGENT_COLORS:      warp: "bg-teal-500"
AGENT_TEXT_COLORS:  warp: "text-teal-300"
AGENT_TINTS:        warp: "bg-teal-400/10 border-teal-400/20"
AGENT_LABELS:       warp: "Warp"
```

## Dependencies

- `prost` — protobuf decoding (lightweight, no codegen needed)
- `prost-types` — well-known types (Timestamp)
- `rusqlite` — SQLite reading (with `bundled` feature for portability)

## Risks & Mitigations

- **Protobuf schema changes**: Warp could change field numbers between versions. Mitigation: `prost` will return `None` for unknown/missing fields gracefully. Log warnings for unexpected structures.
- **SQLite access conflicts**: Warp may have the database locked. Mitigation: Open in read-only mode with WAL (same mode Orbit's own DB uses).
- **macOS sandbox**: Group Containers path may have permission restrictions. Mitigation: Tauri apps run outside the App Sandbox by default, so file access should work. Test to confirm.
