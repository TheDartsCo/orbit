# Orbit UI Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refresh Orbit into a denser, table-first session browser with a more structured transcript viewer inspired by the provided reference.

**Architecture:** Keep all existing Tauri commands, Zustand state, and data models unchanged. The work is scoped to React presentation components and Tailwind theme tokens so the app keeps the same behavior while presenting sessions and transcripts with stronger information hierarchy.

**Tech Stack:** React 19, TypeScript, Tailwind CSS v4, lucide-react, @tanstack/react-virtual, Vite.

---

### Task 1: Establish Baseline Verification

**Files:**
- Read: `package.json`

**Step 1:** Confirm there is no frontend test runner configured.

**Step 2:** Run `npm run build`.

**Expected:** TypeScript and Vite build pass before UI edits.

### Task 2: Refresh Theme Tokens

**Files:**
- Modify: `src/index.css`

**Step 1:** Update dark theme tokens for a warmer, higher-contrast console surface.

**Step 2:** Add small global utility polish for selection, scrollbars, and button/input font inheritance.

**Step 3:** Run `npm run build`.

**Expected:** Build still passes.

### Task 3: Convert Shell Layout

**Files:**
- Modify: `src/App.tsx`

**Step 1:** Replace the narrow sidebar/main split with a full-width app shell.

**Step 2:** Keep the empty-state behavior for no selected session.

**Step 3:** Run `npm run build`.

**Expected:** Build still passes.

### Task 4: Build Dense Session Browser

**Files:**
- Modify: `src/components/Sidebar/Sidebar.tsx`
- Modify: `src/components/Sidebar/SessionList.tsx`
- Modify: `src/components/Sidebar/SessionItem.tsx`
- Modify: `src/components/Sidebar/SearchBar.tsx`
- Modify: `src/components/Sidebar/FilterBar.tsx`
- Modify: `src/components/common/Badge.tsx`

**Step 1:** Make the browser occupy the left pane with a toolbar, filter chips, search, and table header.

**Step 2:** Render sessions as compact rows with columns for agent, title, date, project, and message count.

**Step 3:** Preserve virtualization, selection, active indicators, and filtering behavior.

**Step 4:** Run `npm run build`.

**Expected:** Build still passes.

### Task 5: Restyle Transcript View

**Files:**
- Modify: `src/components/Transcript/TranscriptView.tsx`
- Modify: `src/components/Transcript/MessageBubble.tsx`
- Modify: `src/components/Transcript/ToolCall.tsx`
- Modify: `src/components/Transcript/MarkdownRenderer.tsx`
- Modify: `src/components/ActionBar/ActionBar.tsx`

**Step 1:** Add a transcript header with selected session metadata and compact controls.

**Step 2:** Restyle user, assistant, system, and tool messages as dense readable blocks with role accents.

**Step 3:** Rework the action bar into a status footer matching the reference.

**Step 4:** Run `npm run build`.

**Expected:** Build passes with no TypeScript errors.

### Task 6: Visual Verification

**Files:**
- No code changes expected.

**Step 1:** Start `npm run dev`.

**Step 2:** Open the local app in the in-app browser.

**Step 3:** Check desktop layout for non-overlap, usable scrolling, table density, transcript readability, and responsive minimum widths.

**Expected:** The app renders a polished dense desktop UI inspired by the reference screenshot.
