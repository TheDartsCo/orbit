# Orbit Logo Theme Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Apply the solid colors from Orbit's new logo to the application
chrome while preserving agent-specific colors.

**Architecture:** Use `src/index.css` as the source of truth for shared surface,
border, text, and accent colors. Replace remaining hard-coded neutral and
search-highlight colors with those tokens or solid logo palette colors.

**Tech Stack:** React, TypeScript, Tailwind CSS v4

---

### Task 1: Update shared theme tokens

**Files:**
- Modify: `src/index.css`

**Steps:**
1. Audit the current token values and scrollbar colors.
2. Replace neutral grays with the approved navy-violet palette.
3. Replace the current blue accent with logo blue.
4. Confirm no gradients or glow were introduced.

### Task 2: Replace hard-coded neutral surfaces

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/Transcript/TranscriptView.tsx`
- Modify: `src/components/Transcript/MarkdownRenderer.tsx`
- Modify: `src/components/Transcript/ToolCall.tsx`

**Steps:**
1. Replace hard-coded black and gray backgrounds with shared theme tokens.
2. Keep semantic tool and agent colors unchanged.
3. Audit frontend components for remaining old neutral values.

### Task 3: Align search highlights

**Files:**
- Modify: `src/components/common/Highlight.tsx`
- Modify: `src/components/Transcript/MarkdownRenderer.tsx`

**Steps:**
1. Replace yellow search highlights with muted magenta.
2. Confirm highlighted text remains readable.

### Task 4: Verify

**Steps:**
1. Run the color audit.
2. Run `npm run build`.
3. Open the local frontend and inspect the empty state and controls.
4. Run `git diff --check`.
