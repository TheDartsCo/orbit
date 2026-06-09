# Linux Local Dev Support Design

## Goal

Make Orbit usable from a local Linux development checkout on Ubuntu/Debian-style
desktops. The first Linux pass should allow a developer to build the app,
index local sessions for the most common filesystem-backed agents, and resume
sessions through an installed terminal.

This is not a Linux release-packaging project. It does not add `.deb`,
AppImage, Flatpak, or CI release workflows.

## Scope

Linux local dev support covers these adapters:

- Claude Code
- Codex
- Cursor
- OpenCode

Copilot, Warp, and Qoder remain macOS-only in this pass because their Linux
storage formats have not been verified from real samples.

## Adapter Discovery

The existing adapter contract remains unchanged. Each adapter still owns
detection, scanning, parsing, resume-command construction, and active-session
detection. Linux-specific path discovery stays inside each adapter's current
path-resolution function.

Claude on Linux uses `~/.claude/projects`, matching the observed local Linux
layout and the existing macOS parser behavior. Detection should return true
when `~/.claude` exists. Scanning should continue to inspect only top-level
project `*.jsonl` files so nested subagent transcripts remain excluded.

Codex on Linux uses `~/.codex` when it exists. The existing recursive scanner
already finds `~/.codex/sessions/YYYY/MM/DD/*.jsonl` and skips root-level JSON
files such as `auth.json`.

OpenCode on Linux uses XDG-style candidates, deduplicated before probing:

- `~/.local/share/opencode`
- `dirs::data_local_dir()/opencode`
- `dirs::data_dir()/opencode`
- `dirs::config_dir()/opencode`

Cursor on Linux uses `~/.cursor` when it exists. The existing scanner continues
to look for `projects/*/agent-transcripts/*/*.jsonl`. If Cursor stores Linux
transcripts elsewhere, that path should be added only after verification from a
real Linux install.

Parser behavior should not change unless Linux samples prove a format mismatch.

## Resume Behavior

Linux resume should not assume `xterm`.

Add a Linux-only terminal launcher in `commands.rs`. It should choose a
terminal in this order:

1. The saved `preferred_terminal`, if it maps to an installed Linux terminal.
2. `$TERMINAL`, if set and executable.
3. Common desktop terminals: `xdg-terminal-exec`, `kgx`, `gnome-terminal`,
   `konsole`, `xfce4-terminal`, `mate-terminal`, `tilix`, `alacritty`, `kitty`,
   `ghostty`, `wezterm`, and `xterm`.

Different terminals need different argument shapes:

- `gnome-terminal`, `kgx`, `xfce4-terminal`, `mate-terminal`, and `tilix`:
  `-- sh -lc <cmd>`
- `konsole`: `-e sh -lc <cmd>`
- `alacritty`, `kitty`, `ghostty`, `wezterm`, and `xterm`:
  `-e sh -lc <cmd>`
- `xdg-terminal-exec`: `sh -lc <cmd>`

If no terminal is available, `launch_resume` should return a clear actionable
error naming supported terminals and telling the user they can set `TERMINAL`.

`detect_terminals` should become OS-aware. On Linux it should report Linux
terminal options instead of macOS-only Terminal, iTerm2, Warp, and Ghostty.

## Documentation

The README should describe Linux support narrowly:

- Local Linux dev support exists for Claude, Codex, Cursor, and OpenCode.
- Linux release builds are not yet published or fully tested.
- Copilot, Warp, and Qoder are still macOS-only until verified.

The README or CONTRIBUTING guide should document Ubuntu/Debian Tauri
prerequisites, including GTK/WebKitGTK development packages.

The supported-agents table should make platform support explicit so a Linux
developer does not assume every adapter works on Linux.

## Verification

Verification should be local and concrete:

- Install frontend dependencies if needed, then run `npm run build`.
- Run `cargo fmt --check` from `src-tauri/`.
- Run `cargo test` from `src-tauri/`.
- After Ubuntu/Debian GTK and WebKitGTK development packages are installed,
  run `npm run tauri dev` and manually reindex.
- Confirm the reindex sees real local Claude, Codex, and OpenCode files.
  Cursor can be documented as path-supported but not locally smoke-tested if
  the machine has no Cursor transcripts.

## Non-Goals

- No Linux release packaging.
- No Linux CI workflow.
- No Copilot, Warp, or Qoder Linux support.
- No live file watcher wiring.
- No parser rewrites without verified Linux format differences.
