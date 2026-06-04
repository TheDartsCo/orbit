# OS-Dependent Adapter Paths Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Gate every adapter's current discovery root behind macOS and leave explicit Linux and Windows placeholders.

**Architecture:** Keep OS branching inside each adapter's root-resolution function. Preserve macOS behavior and return no root on unimplemented platforms.

**Tech Stack:** Rust, Tauri v2, Cargo tests

---

### Task 1: Add platform-gated adapter roots

**Files:**
- Modify: `src-tauri/src/adapters/claude.rs`
- Modify: `src-tauri/src/adapters/codex.rs`
- Modify: `src-tauri/src/adapters/cursor.rs`
- Modify: `src-tauri/src/adapters/opencode.rs`
- Modify: `src-tauri/src/adapters/copilot.rs`
- Modify: `src-tauri/src/adapters/qoder.rs`
- Modify: `src-tauri/src/adapters/warp.rs`

**Step 1:** Run existing adapter tests to establish the baseline.

Run: `cargo test adapters::`

**Step 2:** Add explicit macOS, Linux, and Windows branches to each initial path-resolution function.

Preserve current path lookup in macOS branches. Return `None` or `Vec::new()` in Linux and Windows branches with `// To be implemented.` comments.

**Step 3:** Format and run adapter tests.

Run: `cargo fmt --check`

Run: `cargo test adapters::`

**Step 4:** Run complete verification.

Run: `cargo test`

Run: `git diff --check`
