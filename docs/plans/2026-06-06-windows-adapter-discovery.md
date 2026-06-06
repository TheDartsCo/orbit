# Windows Adapter Discovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Windows session discovery for all eight adapters and replace automatic Windows resume launching with a copyable fallback dialog.

**Architecture:** Add small shared platform-path helpers and keep adapter-specific candidate lists and validation inside each adapter. Expose the runtime OS through Tauri, then let the action bar show a Windows-only resume dialog while retaining a backend guard against launching.

**Tech Stack:** Rust, Tauri v2, React 19, TypeScript, Tailwind CSS v4

---

### Task 1: Shared Windows path helpers

**Files:**
- Modify: `src-tauri/src/adapters/mod.rs`

**Steps:**
1. Add failing unit tests for joining home, roaming, and local roots with adapter-relative paths.
2. Run `cargo test adapters::tests::platform_paths -- --nocapture` and confirm failure.
3. Add a `PlatformPaths` helper with injectable roots for tests and a system constructor using `dirs`.
4. Run the focused test and confirm it passes.

### Task 2: File-backed adapter discovery

**Files:**
- Modify: `src-tauri/src/adapters/claude.rs`
- Modify: `src-tauri/src/adapters/codex.rs`
- Modify: `src-tauri/src/adapters/copilot.rs`
- Modify: `src-tauri/src/adapters/cursor.rs`
- Modify: `src-tauri/src/adapters/opencode.rs`

**Steps:**
1. Add failing tests for each adapter's Windows candidates and selection rules.
2. Run the focused adapter tests and confirm the Windows placeholder behavior fails.
3. Implement Windows candidate construction from `PlatformPaths`.
4. Reuse each adapter's existing format checks to select valid roots.
5. Run focused tests and confirm they pass.

### Task 3: Application-data adapter discovery

**Files:**
- Modify: `src-tauri/src/adapters/jetbrains.rs`
- Modify: `src-tauri/src/adapters/qoder.rs`
- Modify: `src-tauri/src/adapters/warp.rs`

**Steps:**
1. Add failing temporary-directory tests for JetBrains history, Qoder database, and Warp database candidates.
2. Run focused tests and confirm failure.
3. Implement deterministic roaming/local candidate lists.
4. Require expected history directories or database files before selecting a candidate.
5. Run focused tests and confirm they pass.

### Task 4: Runtime OS command and Windows launch guard

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Steps:**
1. Add a unit-tested helper returning the compile-time OS label.
2. Register a `get_platform` Tauri command.
3. Replace the Windows `cmd` launch branch with an explicit unsupported error.
4. Run Rust tests.

### Task 5: Copyable Windows resume dialog

**Files:**
- Create: `src/components/ActionBar/WindowsResumeModal.tsx`
- Modify: `src/components/ActionBar/ActionBar.tsx`

**Steps:**
1. Load the runtime platform from `get_platform`.
2. On Windows, fetch the resume command when supported and open the dialog.
3. Display the session ID and command in read-only copyable fields with copy feedback.
4. Keep macOS launch behavior and direct Copy Resume behavior unchanged.
5. Run `npm run build`.

### Task 6: Full verification

**Steps:**
1. Run `cargo fmt --check`.
2. Run `cargo test`.
3. Run `npm run build`.
4. Run `git diff --check`.
5. Review the final diff against all eight adapters and both resume paths.
