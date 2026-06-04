# Filters & Search Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire up message role filters in the transcript view and add keyword highlighting in both session list titles and transcript messages when a search query is active.

**Architecture:** All changes are frontend-only. A shared `Highlight` component wraps matching text in `<mark>` tags. The Zustand store gains a `messageRoleFilter` field. The search query from `filters.query` flows down to `SessionItem` (for title highlight) and `MessageBubble` (for content highlight).

**Tech Stack:** React 19, TypeScript, Zustand, @tanstack/react-virtual, Tailwind CSS v4

---

### Task 1: Create shared Highlight component

**Files:**
- Create: `src/components/common/Highlight.tsx`

This is a pure utility component that splits text on a case-insensitive query match and wraps matches in a styled `<mark>` tag. Used by both SessionItem and MessageBubble.

- [ ] **Step 1: Create the Highlight component**

```tsx
interface HighlightProps {
  text: string;
  query: string | undefined | null;
}

export function Highlight({ text, query }: HighlightProps) {
  if (!query || !query.trim()) return <>{text}</>;

  const regex = new RegExp(`(${escapeRegex(query)})`, "gi");
  const parts = text.split(regex);

  if (parts.length === 1) return <>{text}</>;

  return (
    <>
      {parts.map((part, i) =>
        regex.test(part) ? (
          <mark key={i} className="rounded bg-yellow-500/25 text-inherit px-0.5">
            {part}
          </mark>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}

function escapeRegex(str: string): string {
  return str.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
```

- [ ] **Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/common/Highlight.tsx
git commit -m "feat: add shared Highlight component for search keyword matching"
```

---

### Task 2: Wire up message role filter in Zustand store

**Files:**
- Modify: `src/store/useAppStore.ts`

Add a `messageRoleFilter` field to the store. This is a client-side filter applied to the loaded messages array — no backend changes needed.

- [ ] **Step 1: Add messageRoleFilter to the store interface and initial state**

In `src/store/useAppStore.ts`:

Add to the `AppState` interface:
```ts
messageRoleFilter: MessageRole | null;
```

Add to the initial state object:
```ts
messageRoleFilter: null,
```

Import `MessageRole` from the types:
```ts
import type { Session, Message, SessionFilters, IndexStats, MessageRole } from "../types";
```

- [ ] **Step 2: Add setMessageRoleFilter action**

Add to `AppState` interface:
```ts
setMessageRoleFilter: (role: MessageRole | null) => void;
```

Add to the store implementation:
```ts
setMessageRoleFilter: (role: MessageRole | null) => {
  set({ messageRoleFilter: role });
},
```

- [ ] **Step 3: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add src/store/useAppStore.ts
git commit -m "feat: add messageRoleFilter to Zustand store"
```

---

### Task 3: Wire up TranscriptView role filter buttons

**Files:**
- Modify: `src/components/Transcript/TranscriptView.tsx`

The header already has styled role buttons (All / user / assistant / tool) that are hardcoded. Wire them to the store's `messageRoleFilter` and filter the virtual list.

- [ ] **Step 1: Connect the role filter buttons to the store**

In `TranscriptView.tsx`, add store subscriptions:

```tsx
const messageRoleFilter = useAppStore((s) => s.messageRoleFilter);
const setMessageRoleFilter = useAppStore((s) => s.setMessageRoleFilter);
```

- [ ] **Step 2: Filter messages based on role**

After the `messages` constant, add a filtered list:

```tsx
const filteredMessages = messageRoleFilter
  ? messages.filter((m) => m.role === messageRoleFilter)
  : messages;
```

Then update the virtualizer to use `filteredMessages` instead of `messages`:
- `count: filteredMessages.length` (was `messages.length`)
- `const message = filteredMessages[virtualItem.index]` (was `messages[virtualItem.index]`)

Also update `roleCount` calls to still use the full `messages` array (counts show totals, not filtered).

- [ ] **Step 3: Wire up the role filter button click handlers and active styles**

Replace the hardcoded role filter buttons with:

```tsx
<div className="flex h-10 items-center gap-3 border-t border-border px-4 text-xs font-semibold text-text-secondary">
  <button
    onClick={() => setMessageRoleFilter(null)}
    className={`rounded-full border px-3 py-1 ${
      messageRoleFilter === null
        ? "border-accent bg-accent/10 text-accent"
        : "border-border text-text-secondary hover:bg-bg-hover"
    }`}
  >
    All
  </button>
  <button
    onClick={() => setMessageRoleFilter("user")}
    className={`rounded-full border px-3 py-1 ${
      messageRoleFilter === "user"
        ? "border-blue-400 bg-blue-400/10 text-blue-400"
        : "border-border text-text-secondary hover:bg-bg-hover"
    }`}
  >
    {roleCount(messages, "user")} user
  </button>
  <button
    onClick={() => setMessageRoleFilter("assistant")}
    className={`rounded-full border px-3 py-1 ${
      messageRoleFilter === "assistant"
        ? "border-fuchsia-400 bg-fuchsia-400/10 text-fuchsia-400"
        : "border-border text-text-secondary hover:bg-bg-hover"
    }`}
  >
    {roleCount(messages, "assistant")} assistant
  </button>
  <button
    onClick={() => setMessageRoleFilter("tool")}
    className={`rounded-full border px-3 py-1 ${
      messageRoleFilter === "tool"
        ? "border-green-400 bg-green-400/10 text-green-400"
        : "border-border text-text-secondary hover:bg-bg-hover"
    }`}
  >
    {roleCount(messages, "tool")} tool
  </button>
  <span className="text-text-muted">{messages.length} total</span>
</div>
```

- [ ] **Step 4: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add src/components/Transcript/TranscriptView.tsx
git commit -m "feat: wire up message role filter buttons in transcript header"
```

---

### Task 4: Add search highlight to session list items

**Files:**
- Modify: `src/components/Sidebar/SessionItem.tsx`
- Modify: `src/components/Sidebar/SessionList.tsx`

Pass the active search query down to SessionItem. Highlight matching text in the session title.

- [ ] **Step 1: Pass search query from SessionList to SessionItem**

In `SessionList.tsx`, read the search query from the store:

```tsx
const filters = useAppStore((s) => s.filters);
const searchQuery = filters.query || null;
```

Pass it to each `SessionItem`:

```tsx
<SessionItem
  session={session}
  isSelected={selectedSessionId === session.id}
  onClick={() => selectSession(session.id)}
  searchQuery={searchQuery}
/>
```

- [ ] **Step 2: Update SessionItem to accept and use searchQuery**

Update the `SessionItemProps` interface:

```tsx
interface SessionItemProps {
  session: Session;
  isSelected: boolean;
  onClick: () => void;
  searchQuery: string | null;
}
```

Import `Highlight` from `"../../components/common/Highlight"`.

Update the component signature:
```tsx
export function SessionItem({ session, isSelected, onClick, searchQuery }: SessionItemProps) {
```

Replace the title div (line ~33-35) with:

```tsx
<div className="min-w-0 truncate font-semibold tracking-[0]">
  <Highlight text={session.title} query={searchQuery} />
</div>
```

- [ ] **Step 3: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add src/components/Sidebar/SessionItem.tsx src/components/Sidebar/SessionList.tsx
git commit -m "feat: highlight search keyword in session list titles"
```

---

### Task 5: Add search highlight to transcript messages

**Files:**
- Modify: `src/components/Transcript/TranscriptView.tsx`
- Modify: `src/components/Transcript/MessageBubble.tsx`
- Modify: `src/components/Transcript/MarkdownRenderer.tsx`
- Modify: `src/components/Transcript/ToolCall.tsx`

Pass the search query through TranscriptView -> MessageBubble -> MarkdownRenderer. Highlight matching text in message content. Also highlight in tool input/output within ToolCall.

- [ ] **Step 1: Pass searchQuery through TranscriptView to MessageBubble**

In `TranscriptView.tsx`, read the filter query:

```tsx
const filters = useAppStore((s) => s.filters);
const searchQuery = filters.query || null;
```

Pass to each `MessageBubble`:

```tsx
<MessageBubble message={message} searchQuery={searchQuery} />
```

- [ ] **Step 2: Update MessageBubble to accept and pass searchQuery**

Update props:

```tsx
interface MessageBubbleProps {
  message: Message;
  searchQuery: string | null;
}
```

Update signature:
```tsx
export function MessageBubble({ message, searchQuery }: MessageBubbleProps) {
```

Pass to MarkdownRenderer:

```tsx
<MarkdownRenderer content={message.content} query={searchQuery} />
```

Pass to ToolCall:

```tsx
<ToolCall
  toolName={message.tool_name}
  toolInput={message.tool_input}
  toolOutput={message.tool_output}
  searchQuery={searchQuery}
/>
```

- [ ] **Step 3: Update MarkdownRenderer to highlight search terms**

Update props:

```tsx
interface MarkdownRendererProps {
  content: string;
  query?: string | null;
}
```

Use ReactMarkdown's `components` prop to intercept text nodes and apply highlighting:

```tsx
import { Highlight } from "../common/Highlight";

export function MarkdownRenderer({ content, query }: MarkdownRendererProps) {
  return (
    <div className="max-w-none break-words [&_*:first-child]:mt-0 [&_*:last-child]:mb-0 [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-border-light [&_blockquote]:pl-3 [&_blockquote]:text-text-secondary [&_code]:rounded [&_code]:bg-black/35 [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[0.92em] [&_li]:my-1 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_pre]:my-3 [&_pre]:max-h-[420px] [&_pre]:overflow-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-border [&_pre]:bg-[#080808] [&_pre]:p-3 [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_strong]:font-bold [&_table]:my-3 [&_table]:w-full [&_table]:border-collapse [&_td]:border [&_td]:border-border [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-border [&_th]:bg-bg-tertiary [&_th]:px-2 [&_th]:py-1 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={
          query
            ? {
                text: ({ children }) => {
                  const text = String(children);
                  if (!query || !text) return <>{children}</>;
                  return <Highlight text={text} query={query} />;
                },
              }
            : undefined
        }
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
```

Note: The `text` component in react-markdown intercepts raw text nodes before they get rendered, allowing us to wrap them with the `Highlight` component. We only apply this when a query is active to avoid overhead.

- [ ] **Step 4: Update ToolCall to accept searchQuery**

Update ToolCall props:

```tsx
interface ToolCallProps {
  toolName: string;
  toolInput: string | null;
  toolOutput: string | null;
  searchQuery?: string | null;
}
```

Update signature:
```tsx
export function ToolCall({ toolName, toolInput, toolOutput, searchQuery }: ToolCallProps) {
```

Import Highlight:
```tsx
import { Highlight } from "../common/Highlight";
```

Replace the tool input `<code>` block:
```tsx
<code>{searchQuery ? <Highlight text={toolInput} query={searchQuery} /> : toolInput}</code>
```

Replace the tool output `<code>` block:
```tsx
<code>
  {searchQuery
    ? <Highlight text={toolOutput.length > 2000 ? toolOutput.slice(0, 2000) + "\n..." : toolOutput} query={searchQuery} />
    : toolOutput.length > 2000
      ? toolOutput.slice(0, 2000) + "\n..."
      : toolOutput}
</code>
```

- [ ] **Step 5: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 6: Commit**

```bash
git add src/components/Transcript/TranscriptView.tsx src/components/Transcript/MessageBubble.tsx src/components/Transcript/MarkdownRenderer.tsx src/components/Transcript/ToolCall.tsx
git commit -m "feat: highlight search keywords in transcript messages and tool calls"
```

---

### Task 6: Auto-scroll to first matching message in transcript

**Files:**
- Modify: `src/components/Transcript/TranscriptView.tsx`

When a session is opened and a search query is active, scroll the transcript to the first message that contains the search keyword instead of scrolling to the bottom.

- [ ] **Step 1: Add scroll-to-match logic**

In `TranscriptView.tsx`, replace the existing `useEffect` for scroll with:

```tsx
useEffect(() => {
  if (messages.length === 0 || !parentRef.current) return;

  if (searchQuery && filteredMessages.length > 0) {
    const firstMatchIdx = filteredMessages.findIndex((m) => {
      const q = searchQuery.toLowerCase();
      return (
        m.content?.toLowerCase().includes(q) ||
        m.tool_input?.toLowerCase().includes(q) ||
        m.tool_output?.toLowerCase().includes(q)
      );
    });
    if (firstMatchIdx >= 0) {
      requestAnimationFrame(() => {
        virtualizer.scrollToIndex(firstMatchIdx, { align: "start" });
      });
      return;
    }
  }

  parentRef.current.scrollTop = parentRef.current.scrollHeight;
}, [selectedSessionId, searchQuery]);
```

Note: The dependency array uses `selectedSessionId` and `searchQuery` only so it triggers on session change or search change, not on every render.

- [ ] **Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/components/Transcript/TranscriptView.tsx
git commit -m "feat: auto-scroll to first matching message when searching in transcript"
```

---

### Task 7: Clear messageRoleFilter on session change

**Files:**
- Modify: `src/store/useAppStore.ts`

When selecting a different session, reset the message role filter to avoid confusion.

- [ ] **Step 1: Reset filter in selectSession**

In the `selectSession` action, add `messageRoleFilter: null` to the set call:

```ts
selectSession: async (id: string) => {
    set({ selectedSessionId: id, messages: [], loading: true, messageRoleFilter: null });
```

- [ ] **Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add src/store/useAppStore.ts
git commit -m "fix: reset message role filter when switching sessions"
```
