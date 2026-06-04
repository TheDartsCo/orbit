# Settings Page with Terminal Selection — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a settings modal where users select their preferred terminal emulator for the Resume button, with macOS auto-detection of Terminal.app, iTerm2, Warp, and Ghostty.

**Architecture:** Backend detects installed terminals via `/Applications/` filesystem checks, persists preference in the existing `settings` SQLite table. `launch_resume` reads preference and routes to correct terminal via per-terminal AppleScript/command logic. Frontend adds a modal with a dropdown, triggered by gear icon in sidebar footer.

**Tech Stack:** Rust (Tauri commands, rusqlite), React + TypeScript + Zustand + Tailwind v4, lucide-react icons.

---

### Task 1: Add `TerminalInfo` model to Rust backend

**Files:**
- Modify: `src-tauri/src/models.rs` (append after line 175)

**Step 1: Add TerminalInfo struct**

Append after the `SessionFilters` struct (after line 175):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub id: String,
    pub name: String,
    pub available: bool,
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add src-tauri/src/models.rs
git commit -m "feat: add TerminalInfo model for terminal detection"
```

---

### Task 2: Add `detect_terminals` command to Rust backend

**Files:**
- Modify: `src-tauri/src/commands.rs` (add after line 171)

**Step 1: Add the detect_terminals command**

Add after the `get_sync_status` function (after line 171):

```rust
#[tauri::command]
pub async fn detect_terminals() -> Result<Vec<TerminalInfo>, String> {
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
```

Note: On non-macOS this will still return Terminal as available but the others may not exist at `/Applications/`. The launch_resume function handles the actual platform-specific logic.

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: add detect_terminals command"
```

---

### Task 3: Add `get_app_settings` and `save_app_setting` commands

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Add settings commands**

Add after `detect_terminals`:

```rust
#[derive(serde::Serialize)]
pub struct AppSettings {
    pub preferred_terminal: Option<String>,
}

#[tauri::command]
pub async fn get_app_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    let preferred_terminal = queries
        .get_setting("preferred_terminal")
        .map_err(|e| e.to_string())?;
    Ok(AppSettings { preferred_terminal })
}

#[tauri::command]
pub async fn save_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let queries = DbQueries::new(&db);
    queries
        .set_setting(&key, &value)
        .map_err(|e| e.to_string())
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: add get_app_settings and save_app_setting commands"
```

---

### Task 4: Modify `launch_resume` to use terminal preference

**Files:**
- Modify: `src-tauri/src/commands.rs` (replace lines 93-127)

**Step 1: Replace launch_resume with terminal-aware version**

Replace the entire `launch_resume` function (lines 93-127) with:

```rust
#[tauri::command]
pub async fn launch_resume(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let cmd = get_resume_command(State::from(&state), session_id).await?;

    let preferred = {
        let db = state.db.lock().await;
        let queries = DbQueries::new(&db);
        queries.get_setting("preferred_terminal").ok().flatten()
    };

    let terminal = preferred.unwrap_or_else(|| "terminal".to_string());

    #[cfg(target_os = "macos")]
    {
        match terminal.as_str() {
            "iterm" => {
                std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(format!(
                        "tell application \"iTerm2\"\nactivate\ncreate window with default profile\n tell current session of current window\nwrite text \"{}\"\nend tell\nend tell",
                        cmd.replace('\\', "\\\\").replace('"', "\\\"")
                    ))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "warp" => {
                std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(format!(
                        "tell application \"Warp\"\nactivate\nend tell\ndelay 0.5\ntell application \"System Events\"\ntell process \"Warp\"\nkeystroke \"t\" using command down\nkeystroke \"{}\"\nkey code 36\nend tell\nend tell",
                        cmd.replace('\\', "\\\\").replace('"', "\\\"")
                    ))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            "ghostty" => {
                std::process::Command::new("open")
                    .args(["-a", "Ghostty", "--args", "-e", "bash", "-c", &cmd])
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
            _ => {
                std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(format!(
                        "tell application \"Terminal\" to do script \"{}\"",
                        cmd.replace('"', "\\\"")
                    ))
                    .spawn()
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("xterm -e {} &", cmd))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "cmd", "/K", &cmd])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
```

**Important:** The `get_resume_command` call on line that uses `State::from(&state)` — check the Tauri v2 API. In Tauri v2, you cannot create a State from a reference like that. Instead, extract the resume command inline or refactor. The simpler approach: call the DB query directly rather than trying to call `get_resume_command` which expects its own `State`:

Actually, let's just keep calling `get_resume_command` the same way as before but we need to handle the State differently. Looking at the original code at line 95:

```rust
let cmd = get_resume_command(state, session_id).await?;
```

This passes `state` directly. Let's keep that pattern. But we need to read the setting BEFORE or AFTER calling get_resume_command. Since get_resume_command locks the db, we should read the setting first, then call get_resume_command. Replace the first two lines:

```rust
    let preferred = {
        let db = state.db.lock().await;
        let queries = DbQueries::new(&db);
        queries.get_setting("preferred_terminal").ok().flatten()
    };

    let cmd = get_resume_command(state, session_id).await?;
```

Wait — this won't work either because `get_resume_command` also tries to lock `state.db`. We need to ensure the lock is dropped before calling get_resume_command. The block scope already handles that — `db` is dropped at the end of the block. This should be fine.

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat: launch_resume uses preferred terminal from settings"
```

---

### Task 5: Register new commands in lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs` (lines 38-47)

**Step 1: Add new commands to invoke_handler**

Replace lines 38-47 with:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_session_messages,
            commands::search_sessions,
            commands::get_resume_command,
            commands::launch_resume,
            commands::get_active_sessions,
            commands::reindex_all,
            commands::get_sync_status,
            commands::detect_terminals,
            commands::get_app_settings,
            commands::save_app_setting,
        ])
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo build`
Expected: compiles with no new errors

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: register new settings and terminal commands"
```

---

### Task 6: Add frontend types for terminal info

**Files:**
- Modify: `src/types/index.ts` (append after line 125)

**Step 1: Add TerminalInfo type and TERMINAL_LABELS**

Append after the `AGENT_LABELS` constant:

```typescript
export interface TerminalInfo {
  id: string;
  name: string;
  available: boolean;
}

export const TERMINAL_LABELS: Record<string, string> = {
  terminal: "Terminal",
  iterm: "iTerm2",
  warp: "Warp",
  ghostty: "Ghostty",
};
```

**Step 2: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: no new errors

**Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat: add TerminalInfo type and labels"
```

---

### Task 7: Add settings state and actions to Zustand store

**Files:**
- Modify: `src/store/useAppStore.ts`

**Step 1: Add settings state fields to the interface**

Add to the `AppState` interface (after `sortConfig: SortConfig | null;` at line 29):

```typescript
  availableTerminals: TerminalInfo[];
  preferredTerminal: string | null;
  settingsOpen: boolean;

  loadSettings: () => Promise<void>;
  setPreferredTerminal: (id: string) => Promise<void>;
  openSettings: () => void;
  closeSettings: () => void;
```

Also add `TerminalInfo` to the import on line 3.

**Step 2: Add default values and implementations in the store**

After `sortConfig: null,` (line 60), add default values:

```typescript
  availableTerminals: [],
  preferredTerminal: null,
  settingsOpen: false,
```

After the `setGitBranchFilter` action (after line 225), add:

```typescript
  loadSettings: async () => {
    try {
      const [terminals, settings] = await Promise.all([
        invoke<TerminalInfo[]>("detect_terminals"),
        invoke<{ preferred_terminal: string | null }>("get_app_settings"),
      ]);
      set({
        availableTerminals: terminals,
        preferredTerminal: settings.preferred_terminal ?? null,
      });
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  },

  setPreferredTerminal: async (id: string) => {
    try {
      await invoke("save_app_setting", { key: "preferred_terminal", value: id });
      set({ preferredTerminal: id });
    } catch (e) {
      console.error("Failed to save terminal preference:", e);
    }
  },

  openSettings: () => set({ settingsOpen: true }),
  closeSettings: () => set({ settingsOpen: false }),
```

**Step 3: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: no new errors

**Step 4: Commit**

```bash
git add src/store/useAppStore.ts
git commit -m "feat: add settings state and actions to store"
```

---

### Task 8: Create SettingsModal component

**Files:**
- Create: `src/components/Settings/SettingsModal.tsx`

**Step 1: Create the SettingsModal**

Create file `src/components/Settings/SettingsModal.tsx`:

```tsx
import { useEffect } from "react";
import { X } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";
import { TERMINAL_LABELS } from "../../types";

export function SettingsModal() {
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const closeSettings = useAppStore((s) => s.closeSettings);
  const availableTerminals = useAppStore((s) => s.availableTerminals);
  const preferredTerminal = useAppStore((s) => s.preferredTerminal);
  const setPreferredTerminal = useAppStore((s) => s.setPreferredTerminal);
  const loadSettings = useAppStore((s) => s.loadSettings);

  useEffect(() => {
    if (settingsOpen) loadSettings();
  }, [settingsOpen, loadSettings]);

  if (!settingsOpen) return null;

  const available = availableTerminals.filter((t) => t.available);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/60"
        onClick={closeSettings}
      />
      <div className="relative z-10 w-full max-w-md rounded-xl border border-border bg-bg-secondary p-6 shadow-2xl">
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold text-text-primary">Settings</h2>
          <button
            onClick={closeSettings}
            className="rounded-md p-1 text-text-muted transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4">
          <div>
            <label className="mb-1.5 block text-xs font-semibold uppercase tracking-wide text-text-secondary">
              Terminal Emulator
            </label>
            <p className="mb-2 text-xs text-text-muted">
              Choose which terminal to use when resuming sessions.
            </p>
            <select
              value={preferredTerminal ?? "terminal"}
              onChange={(e) => setPreferredTerminal(e.target.value)}
              className="h-9 w-full rounded-lg border border-border bg-bg-primary px-3 text-sm font-medium text-text-primary outline-none transition-colors focus:border-accent"
            >
              {available.map((t) => (
                <option key={t.id} value={t.id}>
                  {TERMINAL_LABELS[t.id] ?? t.name}
                </option>
              ))}
            </select>
          </div>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: no new errors

**Step 3: Commit**

```bash
mkdir -p src/components/Settings
git add src/components/Settings/SettingsModal.tsx
git commit -m "feat: add SettingsModal component"
```

---

### Task 9: Add gear icon to sidebar footer and wire up modal

**Files:**
- Modify: `src/components/Sidebar/Sidebar.tsx`
- Modify: `src/App.tsx`

**Step 1: Add Settings import and gear icon to Sidebar.tsx**

Add `Settings` to the lucide-react import (line 10-16):

```typescript
import {
  Check,
  ChevronDown,
  Columns3,
  Filter,
  RefreshCw,
  Settings,
  X,
} from "lucide-react";
```

Add store selectors for settings (after line 100):

```typescript
  const openSettings = useAppStore((s) => s.openSettings);
```

Add gear button in the footer bar. In the footer `<div>` (line 475), add a settings button before the refresh button (before line 493):

```tsx
        <button
          onClick={openSettings}
          className="flex h-full w-8 shrink-0 items-center justify-center border-l border-border text-text-muted transition-colors hover:bg-bg-hover hover:text-text-secondary"
          aria-label="Settings"
        >
          <Settings className="h-4 w-4" />
        </button>
```

**Step 2: Add SettingsModal to App.tsx**

Add import:

```typescript
import { SettingsModal } from "./components/Settings/SettingsModal";
```

Add `<SettingsModal />` right after the `<Sidebar>` component (after line 80):

```tsx
      <Sidebar width={sidebarWidth} />
      <SettingsModal />
```

**Step 3: Add loadSettings call to App.tsx startup**

Add to the imports:

```typescript
const loadSettings = useAppStore((s) => s.loadSettings);
```

Add `loadSettings()` to the useEffect (after `loadSyncStatus()` on line 27):

```typescript
  useEffect(() => {
    loadSessions();
    loadSyncStatus();
    loadSettings();
    const interval = setInterval(refreshActiveSessions, 5000);
    return () => clearInterval(interval);
  }, [loadSessions, refreshActiveSessions, loadSyncStatus, loadSettings]);
```

**Step 4: Verify TypeScript**

Run: `npx tsc --noEmit`
Expected: no new errors

**Step 5: Commit**

```bash
git add src/components/Sidebar/Sidebar.tsx src/App.tsx
git commit -m "feat: wire up settings gear icon and modal"
```

---

### Task 10: Build and verify end-to-end

**Step 1: Build Rust backend**

Run: `cd src-tauri && cargo build`
Expected: no errors

**Step 2: TypeScript check**

Run: `npx tsc --noEmit`
Expected: no errors

**Step 3: Run full app**

Run: `npm run tauri dev`
Expected: App opens, gear icon visible in sidebar footer, clicking it opens settings modal with detected terminals, selecting one persists and is used on next Resume click.
