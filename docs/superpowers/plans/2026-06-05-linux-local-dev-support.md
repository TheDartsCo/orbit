# Linux Local Dev Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Orbit usable from a local Linux development checkout by indexing Claude, Codex, Cursor, and OpenCode sessions and launching resume commands through common Linux terminals.

**Architecture:** Keep the existing Tauri command and adapter contracts. Add Linux branches inside the existing adapter path-resolution functions, and add a Linux-only terminal launcher in `commands.rs` while keeping macOS behavior unchanged.

**Tech Stack:** Rust, Tauri v2, rusqlite, dirs, React/TypeScript docs surface, Ubuntu/Debian Tauri development prerequisites.

---

## File Map

- Modify `src-tauri/src/adapters/claude.rs`: Linux `~/.claude` detection and `~/.claude/projects` session discovery.
- Modify `src-tauri/src/adapters/codex.rs`: Linux `~/.codex` discovery.
- Modify `src-tauri/src/adapters/cursor.rs`: Linux `~/.cursor` discovery.
- Modify `src-tauri/src/adapters/opencode.rs`: Linux XDG OpenCode candidate directories.
- Modify `src-tauri/src/commands.rs`: OS-aware terminal detection and Linux resume launcher.
- Modify `README.md`: Linux local-dev support wording, prerequisites, and platform support table.
- Modify `CONTRIBUTING.md`: Ubuntu/Debian local setup prerequisites.

## Task 1: Linux Adapter Path Discovery

**Files:**
- Modify: `src-tauri/src/adapters/claude.rs`
- Modify: `src-tauri/src/adapters/codex.rs`
- Modify: `src-tauri/src/adapters/cursor.rs`
- Modify: `src-tauri/src/adapters/opencode.rs`

- [ ] **Step 1: Write failing Claude path tests**

Add this helper test inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/adapters/claude.rs`:

```rust
#[test]
fn project_dirs_from_projects_dir_returns_only_direct_project_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let projects_dir = tmp.path().join("projects");
    let project_a = projects_dir.join("project-a");
    let project_b = projects_dir.join("project-b");
    let nested_session_dir = project_a.join("session-1");
    let nested_subagents_dir = nested_session_dir.join("subagents");

    fs::create_dir_all(&project_a).unwrap();
    fs::create_dir_all(&project_b).unwrap();
    fs::create_dir_all(&nested_subagents_dir).unwrap();
    fs::write(projects_dir.join("not-a-project.jsonl"), "{}").unwrap();

    let mut dirs = ClaudeAdapter::project_dirs_from_projects_dir(&projects_dir);
    dirs.sort();

    assert_eq!(dirs, vec![project_a, project_b]);
}
```

Expected initial failure after running `cargo test adapters::claude::tests::project_dirs_from_projects_dir_returns_only_direct_project_dirs` from `src-tauri/`:

```text
error[E0599]: no function or associated item named `project_dirs_from_projects_dir`
```

- [ ] **Step 2: Implement Claude Linux discovery helper**

Replace `ClaudeAdapter::project_dirs` and `detect` in `src-tauri/src/adapters/claude.rs` with this implementation:

```rust
impl ClaudeAdapter {
    pub fn new() -> Self {
        Self
    }

    fn claude_root() -> Option<PathBuf> {
        if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
            dirs::home_dir().map(|home| home.join(".claude"))
        } else if cfg!(target_os = "windows") {
            // Windows adapter discovery is outside this Linux local-dev scope.
            None
        } else {
            None
        }
    }

    fn project_dirs_from_projects_dir(projects_dir: &Path) -> Vec<PathBuf> {
        if !projects_dir.exists() {
            return Vec::new();
        }

        let mut dirs = Vec::new();
        if let Ok(entries) = std::fs::read_dir(projects_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    dirs.push(entry.path());
                }
            }
        }
        dirs
    }

    fn project_dirs(&self) -> Vec<PathBuf> {
        let Some(claude_root) = Self::claude_root() else {
            return Vec::new();
        };

        Self::project_dirs_from_projects_dir(&claude_root.join("projects"))
    }
}
```

Then replace the `detect` body with:

```rust
async fn detect(&self) -> bool {
    Self::claude_root().is_some_and(|path| path.exists())
}
```

This keeps Windows unsupported and makes Linux mirror the verified `~/.claude/projects` layout.

- [ ] **Step 3: Write failing Codex path test**

Add this test inside `#[cfg(test)] mod tests` in `src-tauri/src/adapters/codex.rs`:

```rust
#[test]
fn data_dir_path_from_home_uses_dot_codex_on_unix() {
    let home = std::path::Path::new("/home/orbit-user");

    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        assert_eq!(
            CodexAdapter::data_dir_path_from_home(home),
            Some(home.join(".codex"))
        );
    } else {
        assert!(CodexAdapter::data_dir_path_from_home(home).is_none());
    }
}
```

Expected initial failure after running `cargo test adapters::codex::tests::data_dir_path_from_home_uses_dot_codex_on_unix` from `src-tauri/`:

```text
error[E0599]: no function or associated item named `data_dir_path_from_home`
```

- [ ] **Step 4: Implement Codex Linux discovery helper**

Replace `CodexAdapter::data_dir` in `src-tauri/src/adapters/codex.rs` with:

```rust
fn data_dir_path_from_home(home: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        Some(home.join(".codex"))
    } else if cfg!(target_os = "windows") {
        // Windows adapter discovery is outside this Linux local-dev scope.
        None
    } else {
        None
    }
}

fn data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let codex_dir = Self::data_dir_path_from_home(&home)?;
    if codex_dir.exists() {
        Some(codex_dir)
    } else {
        None
    }
}
```

- [ ] **Step 5: Write failing Cursor path test**

Add this test inside `#[cfg(test)] mod tests` in `src-tauri/src/adapters/cursor.rs`:

```rust
#[test]
fn data_dir_path_from_home_uses_dot_cursor_on_unix() {
    let home = std::path::Path::new("/home/orbit-user");

    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        assert_eq!(
            CursorAdapter::data_dir_path_from_home(home),
            Some(home.join(".cursor"))
        );
    } else {
        assert!(CursorAdapter::data_dir_path_from_home(home).is_none());
    }
}
```

Expected initial failure after running `cargo test adapters::cursor::tests::data_dir_path_from_home_uses_dot_cursor_on_unix` from `src-tauri/`:

```text
error[E0599]: no function or associated item named `data_dir_path_from_home`
```

- [ ] **Step 6: Implement Cursor Linux discovery helper**

Replace `CursorAdapter::data_dir` in `src-tauri/src/adapters/cursor.rs` with:

```rust
fn data_dir_path_from_home(home: &Path) -> Option<PathBuf> {
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        Some(home.join(".cursor"))
    } else if cfg!(target_os = "windows") {
        // Windows adapter discovery is outside this Linux local-dev scope.
        None
    } else {
        None
    }
}

fn data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let cursor_dir = Self::data_dir_path_from_home(&home)?;
    if cursor_dir.exists() {
        Some(cursor_dir)
    } else {
        None
    }
}
```

- [ ] **Step 7: Write failing OpenCode XDG candidate test**

Add this test inside `#[cfg(test)] mod tests` in `src-tauri/src/adapters/opencode.rs`:

```rust
#[test]
fn candidate_data_dirs_from_sources_deduplicates_xdg_paths() {
    let home = std::path::PathBuf::from("/home/orbit-user");
    let data_local = std::path::PathBuf::from("/home/orbit-user/.local/share");
    let data = std::path::PathBuf::from("/home/orbit-user/.local/share");
    let config = std::path::PathBuf::from("/home/orbit-user/.config");

    let dirs = OpenCodeAdapter::candidate_data_dirs_from_sources(
        Some(home.clone()),
        Some(data_local),
        Some(data),
        Some(config.clone()),
    );

    assert_eq!(
        dirs,
        vec![
            home.join(".local/share/opencode"),
            config.join("opencode"),
        ]
    );
}
```

Expected initial failure after running `cargo test adapters::opencode::tests::candidate_data_dirs_from_sources_deduplicates_xdg_paths` from `src-tauri/`:

```text
error[E0599]: no function or associated item named `candidate_data_dirs_from_sources`
```

- [ ] **Step 8: Implement OpenCode Linux XDG candidates**

Replace `OpenCodeAdapter::candidate_data_dirs` in `src-tauri/src/adapters/opencode.rs` with:

```rust
fn candidate_data_dirs_from_sources(
    home: Option<PathBuf>,
    data_local: Option<PathBuf>,
    data: Option<PathBuf>,
    config: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut seen = HashSet::new();

    let mut push = |path: Option<PathBuf>| {
        if let Some(path) = path {
            if seen.insert(path.clone()) {
                dirs.push(path);
            }
        }
    };

    push(home.map(|home| home.join(".local/share/opencode")));
    push(data_local.map(|dir| dir.join("opencode")));
    push(data.map(|dir| dir.join("opencode")));
    push(config.map(|dir| dir.join("opencode")));

    dirs
}

fn candidate_data_dirs() -> Vec<PathBuf> {
    if cfg!(target_os = "macos") || cfg!(target_os = "linux") {
        Self::candidate_data_dirs_from_sources(
            dirs::home_dir(),
            dirs::data_local_dir(),
            dirs::data_dir(),
            dirs::config_dir(),
        )
    } else if cfg!(target_os = "windows") {
        // Windows adapter discovery is outside this Linux local-dev scope.
        Vec::new()
    } else {
        Vec::new()
    }
}
```

- [ ] **Step 9: Run adapter path tests**

Run from `src-tauri/`:

```bash
cargo test adapters::claude::tests::project_dirs_from_projects_dir_returns_only_direct_project_dirs
cargo test adapters::codex::tests::data_dir_path_from_home_uses_dot_codex_on_unix
cargo test adapters::cursor::tests::data_dir_path_from_home_uses_dot_cursor_on_unix
cargo test adapters::opencode::tests::candidate_data_dirs_from_sources_deduplicates_xdg_paths
```

Expected: all four commands pass.

- [ ] **Step 10: Commit adapter discovery changes**

Run:

```bash
git add src-tauri/src/adapters/claude.rs src-tauri/src/adapters/codex.rs src-tauri/src/adapters/cursor.rs src-tauri/src/adapters/opencode.rs
git commit -m "feat: enable linux adapter discovery"
```

## Task 2: Linux Terminal Detection And Resume Launch

**Files:**
- Modify: `src-tauri/src/commands.rs`

- [ ] **Step 1: Write failing Linux terminal helper tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/commands.rs`:

```rust
#[cfg(target_os = "linux")]
#[test]
fn linux_terminal_args_match_terminal_style() {
    assert_eq!(
        linux_terminal_args(LinuxTerminalArgStyle::DashDash, "echo hello"),
        vec![
            "--".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ]
    );
    assert_eq!(
        linux_terminal_args(LinuxTerminalArgStyle::DashE, "echo hello"),
        vec![
            "-e".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ]
    );
    assert_eq!(
        linux_terminal_args(LinuxTerminalArgStyle::Xdg, "echo hello"),
        vec![
            "sh".to_string(),
            "-lc".to_string(),
            "echo hello".to_string(),
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
fn preferred_linux_terminal_wins_when_available() {
    let resolved =
        resolve_linux_terminal_with(Some("kitty"), Some("ghostty"), |exe| exe == "kitty")
            .unwrap();

    assert_eq!(resolved.executable, "kitty");
    assert_eq!(resolved.arg_style, LinuxTerminalArgStyle::DashE);
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_env_is_used_before_fallbacks() {
    let resolved = resolve_linux_terminal_with(
        Some("kitty"),
        Some("ghostty"),
        |exe| exe == "ghostty",
    )
    .unwrap();

    assert_eq!(resolved.executable, "ghostty");
    assert_eq!(resolved.arg_style, LinuxTerminalArgStyle::DashE);
}

#[cfg(target_os = "linux")]
#[test]
fn linux_terminal_detection_reports_common_terminals() {
    let terminals = linux_terminal_infos_with(|exe| exe == "kgx" || exe == "xterm");

    assert!(terminals.iter().any(|t| t.id == "kgx" && t.available));
    assert!(terminals.iter().any(|t| t.id == "xterm" && t.available));
    assert!(terminals
        .iter()
        .any(|t| t.id == "gnome-terminal" && !t.available));
}
```

Expected initial failure after running `cargo test commands::tests::linux_terminal_args_match_terminal_style` from `src-tauri/`:

```text
error[E0433]: failed to resolve: use of undeclared type `LinuxTerminalArgStyle`
```

- [ ] **Step 2: Add Linux terminal helper types and constants**

Add this code in `src-tauri/src/commands.rs` after `run_osascript`:

```rust
#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxTerminalArgStyle {
    DashDash,
    DashE,
    Xdg,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy)]
struct LinuxTerminalDefinition {
    id: &'static str,
    name: &'static str,
    executable: &'static str,
    arg_style: LinuxTerminalArgStyle,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedLinuxTerminal {
    executable: String,
    arg_style: LinuxTerminalArgStyle,
}

#[cfg(target_os = "linux")]
const LINUX_TERMINALS: &[LinuxTerminalDefinition] = &[
    LinuxTerminalDefinition {
        id: "xdg-terminal-exec",
        name: "XDG Terminal",
        executable: "xdg-terminal-exec",
        arg_style: LinuxTerminalArgStyle::Xdg,
    },
    LinuxTerminalDefinition {
        id: "kgx",
        name: "GNOME Console",
        executable: "kgx",
        arg_style: LinuxTerminalArgStyle::DashDash,
    },
    LinuxTerminalDefinition {
        id: "gnome-terminal",
        name: "GNOME Terminal",
        executable: "gnome-terminal",
        arg_style: LinuxTerminalArgStyle::DashDash,
    },
    LinuxTerminalDefinition {
        id: "konsole",
        name: "Konsole",
        executable: "konsole",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
    LinuxTerminalDefinition {
        id: "xfce4-terminal",
        name: "XFCE Terminal",
        executable: "xfce4-terminal",
        arg_style: LinuxTerminalArgStyle::DashDash,
    },
    LinuxTerminalDefinition {
        id: "mate-terminal",
        name: "MATE Terminal",
        executable: "mate-terminal",
        arg_style: LinuxTerminalArgStyle::DashDash,
    },
    LinuxTerminalDefinition {
        id: "tilix",
        name: "Tilix",
        executable: "tilix",
        arg_style: LinuxTerminalArgStyle::DashDash,
    },
    LinuxTerminalDefinition {
        id: "alacritty",
        name: "Alacritty",
        executable: "alacritty",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
    LinuxTerminalDefinition {
        id: "kitty",
        name: "Kitty",
        executable: "kitty",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
    LinuxTerminalDefinition {
        id: "ghostty",
        name: "Ghostty",
        executable: "ghostty",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
    LinuxTerminalDefinition {
        id: "wezterm",
        name: "WezTerm",
        executable: "wezterm",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
    LinuxTerminalDefinition {
        id: "xterm",
        name: "xterm",
        executable: "xterm",
        arg_style: LinuxTerminalArgStyle::DashE,
    },
];
```

- [ ] **Step 3: Add Linux terminal pure helper functions**

Add this code below the constants from Step 2:

```rust
#[cfg(target_os = "linux")]
fn linux_terminal_args(style: LinuxTerminalArgStyle, command: &str) -> Vec<String> {
    match style {
        LinuxTerminalArgStyle::DashDash => vec![
            "--".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            command.to_string(),
        ],
        LinuxTerminalArgStyle::DashE => vec![
            "-e".to_string(),
            "sh".to_string(),
            "-lc".to_string(),
            command.to_string(),
        ],
        LinuxTerminalArgStyle::Xdg => {
            vec!["sh".to_string(), "-lc".to_string(), command.to_string()]
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_definition_for_id(id: &str) -> Option<&'static LinuxTerminalDefinition> {
    LINUX_TERMINALS.iter().find(|terminal| terminal.id == id)
}

#[cfg(target_os = "linux")]
fn linux_arg_style_for_executable(executable: &str) -> LinuxTerminalArgStyle {
    let basename = std::path::Path::new(executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(executable);

    LINUX_TERMINALS
        .iter()
        .find(|terminal| terminal.executable == basename)
        .map(|terminal| terminal.arg_style)
        .unwrap_or(LinuxTerminalArgStyle::DashE)
}

#[cfg(target_os = "linux")]
fn resolve_linux_terminal_with<F>(
    preferred: Option<&str>,
    terminal_env: Option<&str>,
    is_available: F,
) -> Option<ResolvedLinuxTerminal>
where
    F: Fn(&str) -> bool,
{
    if let Some(preferred) = preferred.and_then(linux_definition_for_id) {
        if is_available(preferred.executable) {
            return Some(ResolvedLinuxTerminal {
                executable: preferred.executable.to_string(),
                arg_style: preferred.arg_style,
            });
        }
    }

    if let Some(terminal_env) = terminal_env.filter(|value| !value.trim().is_empty()) {
        if is_available(terminal_env) {
            return Some(ResolvedLinuxTerminal {
                executable: terminal_env.to_string(),
                arg_style: linux_arg_style_for_executable(terminal_env),
            });
        }
    }

    LINUX_TERMINALS.iter().find_map(|terminal| {
        is_available(terminal.executable).then(|| ResolvedLinuxTerminal {
            executable: terminal.executable.to_string(),
            arg_style: terminal.arg_style,
        })
    })
}

#[cfg(target_os = "linux")]
fn linux_terminal_infos_with<F>(is_available: F) -> Vec<TerminalInfo>
where
    F: Fn(&str) -> bool,
{
    LINUX_TERMINALS
        .iter()
        .map(|terminal| TerminalInfo {
            id: terminal.id.to_string(),
            name: terminal.name.to_string(),
            available: is_available(terminal.executable),
        })
        .collect()
}
```

- [ ] **Step 4: Add Linux executable lookup and launcher**

Add this code below the pure helpers:

```rust
#[cfg(target_os = "linux")]
fn linux_executable_available(executable: &str) -> bool {
    let executable_path = std::path::Path::new(executable);
    if executable_path.components().count() > 1 {
        return executable_path.is_file();
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .any(|dir| dir.join(executable).is_file())
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn resolve_linux_terminal(preferred: Option<&str>) -> Option<ResolvedLinuxTerminal> {
    let terminal_env = std::env::var("TERMINAL").ok();
    resolve_linux_terminal_with(
        preferred,
        terminal_env.as_deref(),
        linux_executable_available,
    )
}

#[cfg(target_os = "linux")]
fn launch_linux_terminal(command: &str, preferred: Option<&str>) -> Result<(), String> {
    let terminal = resolve_linux_terminal(preferred).ok_or_else(|| {
        "No supported Linux terminal found. Install xterm, GNOME Terminal, Konsole, Ghostty, WezTerm, Kitty, Alacritty, or set TERMINAL.".to_string()
    })?;

    let mut process = std::process::Command::new(&terminal.executable);
    for arg in linux_terminal_args(terminal.arg_style, command) {
        process.arg(arg);
    }

    process.spawn().map(|_| ()).map_err(|e| {
        format!(
            "Failed to launch terminal {}: {}",
            terminal.executable, e
        )
    })
}
```

- [ ] **Step 5: Update `launch_resume` to use OS-specific terminal handling**

Replace this block in `launch_resume`:

```rust
let terminal = preferred.unwrap_or_else(|| "terminal".to_string());

#[cfg(target_os = "macos")]
{
    match terminal.as_str() {
```

with:

```rust
#[cfg(target_os = "macos")]
{
    let terminal = preferred.as_deref().unwrap_or("terminal");
    match terminal {
```

Then replace the Linux block:

```rust
#[cfg(target_os = "linux")]
{
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("xterm -e {} &", cmd))
        .spawn()
        .map_err(|e| e.to_string())?;
}
```

with:

```rust
#[cfg(target_os = "linux")]
{
    launch_linux_terminal(&cmd, preferred.as_deref())?;
}
```

- [ ] **Step 6: Make `detect_terminals` OS-aware**

Replace the full `detect_terminals` function with:

```rust
#[tauri::command]
pub async fn detect_terminals() -> Result<Vec<TerminalInfo>, String> {
    #[cfg(target_os = "macos")]
    {
        let terminals = vec![
            TerminalInfo {
                id: "terminal".to_string(),
                name: "Terminal".to_string(),
                available: true,
            },
            TerminalInfo {
                id: "iterm".to_string(),
                name: "iTerm2".to_string(),
                available: std::path::Path::new("/Applications/iTerm.app").exists(),
            },
            TerminalInfo {
                id: "warp".to_string(),
                name: "Warp".to_string(),
                available: std::path::Path::new("/Applications/Warp.app").exists(),
            },
            TerminalInfo {
                id: "ghostty".to_string(),
                name: "Ghostty".to_string(),
                available: std::path::Path::new("/Applications/Ghostty.app").exists(),
            },
        ];
        Ok(terminals)
    }

    #[cfg(target_os = "linux")]
    {
        Ok(linux_terminal_infos_with(linux_executable_available))
    }

    #[cfg(target_os = "windows")]
    {
        Ok(vec![TerminalInfo {
            id: "cmd".to_string(),
            name: "Command Prompt".to_string(),
            available: true,
        }])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
}
```

- [ ] **Step 7: Run terminal helper tests**

Run from `src-tauri/`:

```bash
cargo test commands::tests::linux_terminal_args_match_terminal_style
cargo test commands::tests::preferred_linux_terminal_wins_when_available
cargo test commands::tests::terminal_env_is_used_before_fallbacks
cargo test commands::tests::linux_terminal_detection_reports_common_terminals
cargo test commands::tests::terminal_applescript_reads_command_from_argv
cargo test commands::tests::iterm_uses_an_executable_command_file
```

Expected: all commands pass on Linux. The macOS AppleScript tests should still pass because they are pure string tests.

- [ ] **Step 8: Commit terminal changes**

Run:

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: support linux resume terminals"
```

## Task 3: Linux Local Dev Documentation

**Files:**
- Modify: `README.md`
- Modify: `CONTRIBUTING.md`

- [ ] **Step 1: Update README platform wording**

In `README.md`, replace the opening platform paragraph:

```markdown
Orbit is a native session browser for AI coding agents. It finds the session
history already stored on your Mac, normalizes it into one local index, and
gives you a fast way to search, filter, read, and resume past work.

Orbit v0.1 is **macOS-first**. Linux and Windows builds are not tested yet.
```

with:

```markdown
Orbit is a native session browser for AI coding agents. It finds the session
history already stored on your machine, normalizes it into one local index, and
gives you a fast way to search, filter, read, and resume past work.

Orbit v0.1 is **macOS-first for release builds**. Local Linux development
support is available for Claude Code, Codex, Cursor, and OpenCode. Linux
release packages are not published or fully tested yet.
```

- [ ] **Step 2: Update README build prerequisites**

Replace the current `Build from source` paragraph in `README.md`:

```markdown
You need Node.js 18 or newer, Rust, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for macOS.
```

with:

````markdown
You need Node.js 18 or newer, Rust, and the
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform.

On Ubuntu/Debian, install Tauri's Linux development dependencies first:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```
````

This package list follows Tauri v2's official Linux prerequisites page: `https://v2.tauri.app/start/prerequisites/`.

- [ ] **Step 3: Replace README supported agents table**

Replace the existing `Supported agents` table with:

```markdown
| Agent | macOS discovery | Linux local dev discovery | Transcript parsing | Resume |
| --- | --- | --- | --- | --- |
| Claude Code | Yes | Yes | Yes | Yes |
| Codex | Yes | Yes | Yes | Yes |
| GitHub Copilot CLI | Yes | Not yet | Yes | Yes on macOS |
| Cursor | Yes | Yes | Yes | Opens project |
| OpenCode | Yes | Yes | Yes | Yes |
| Warp | Yes | Not yet | Yes | Not yet |
| Qoder | Yes | Not yet | Yes | Not yet |
```

Then add this paragraph below the table:

```markdown
Linux local dev support means the app can be built and run from source on a
Linux desktop with Tauri prerequisites installed. Published Linux release
packages are still outside the v0.1 support boundary.
```

- [ ] **Step 4: Update README database location wording**

Replace:

````markdown
The local database lives at:

```text
~/Library/Application Support/co.thedarts.orbit/orbit.db
```
````

with:

````markdown
The local database uses the platform data directory. On macOS it usually lives
at:

```text
~/Library/Application Support/co.thedarts.orbit/orbit.db
```

On Linux it usually lives under:

```text
~/.local/share/orbit/orbit.db
```
````

- [ ] **Step 5: Update README limitations**

Replace this limitation:

```markdown
- macOS is the only tested platform.
```

with:

```markdown
- macOS is the only release-tested platform.
- Linux is supported for local development with Claude Code, Codex, Cursor, and
  OpenCode discovery.
```

- [ ] **Step 6: Update CONTRIBUTING local setup**

Replace this sentence in `CONTRIBUTING.md`:

```markdown
Orbit v0.1 development is tested on macOS.
```

with:

```markdown
Orbit v0.1 release builds are tested on macOS. Local Linux development is
supported for Claude Code, Codex, Cursor, and OpenCode adapters.
```

Then add this section after the dependency list:

````markdown
On Ubuntu/Debian, install Tauri's Linux development dependencies:

```bash
sudo apt update
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```
````

- [ ] **Step 7: Commit documentation changes**

Run:

```bash
git add README.md CONTRIBUTING.md
git commit -m "docs: document linux local development"
```

## Task 4: Full Verification And Manual Linux Smoke Test

**Files:**
- Verify: all modified files

- [ ] **Step 1: Install frontend dependencies**

Run from the repo root:

```bash
npm install
```

Expected: command exits with status 0 and creates `node_modules/`.

- [ ] **Step 2: Run frontend build**

Run from the repo root:

```bash
npm run build
```

Expected:

```text
> orbit@0.1.0 build
> tsc && vite build
```

The command exits with status 0.

- [ ] **Step 3: Run Rust formatting check**

Run from `src-tauri/`:

```bash
cargo fmt --check
```

Expected: command exits with status 0 and prints no diff.

- [ ] **Step 4: Run focused Rust tests**

Run from `src-tauri/`:

```bash
cargo test adapters::claude::tests::project_dirs_from_projects_dir_returns_only_direct_project_dirs
cargo test adapters::codex::tests::data_dir_path_from_home_uses_dot_codex_on_unix
cargo test adapters::cursor::tests::data_dir_path_from_home_uses_dot_cursor_on_unix
cargo test adapters::opencode::tests::candidate_data_dirs_from_sources_deduplicates_xdg_paths
cargo test commands::tests::linux_terminal_args_match_terminal_style
cargo test commands::tests::preferred_linux_terminal_wins_when_available
cargo test commands::tests::terminal_env_is_used_before_fallbacks
cargo test commands::tests::linux_terminal_detection_reports_common_terminals
```

Expected: every command exits with status 0.

- [ ] **Step 5: Run full Rust test suite**

Run from `src-tauri/`:

```bash
cargo test
```

Expected: command exits with status 0.

If this fails with `gdk-3.0.pc` or another GTK/WebKitGTK `pkg-config` error,
install the Ubuntu/Debian dependencies documented in Task 3 and rerun the
command.

- [ ] **Step 6: Run whitespace check**

Run from the repo root:

```bash
git diff --check
```

Expected: command exits with status 0.

- [ ] **Step 7: Run local Tauri smoke test**

Run from the repo root on the Linux desktop:

```bash
npm run tauri dev
```

Expected:

- The Vite dev server starts on port 1420.
- The Tauri window opens.
- Clicking refresh/reindex completes without a backend error.
- The sync status shows indexed sessions for local Claude, Codex, and OpenCode data when those directories exist.
- Cursor is considered path-supported if `~/.cursor` exists; if this machine has no Cursor transcripts, note that Cursor was not manually smoke-tested.

- [ ] **Step 8: Commit any verification-only fixes**

If verification required small code or docs corrections, commit them:

```bash
git add README.md CONTRIBUTING.md src-tauri/src/adapters/claude.rs src-tauri/src/adapters/codex.rs src-tauri/src/adapters/cursor.rs src-tauri/src/adapters/opencode.rs src-tauri/src/commands.rs
git commit -m "fix: polish linux local dev support"
```

Skip this commit if verification required no additional edits.
