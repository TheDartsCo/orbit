import type { Message } from "../../types";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { ToolCall } from "./ToolCall";
import { Bot, Terminal, User } from "lucide-react";
import { isDisplayableMessage } from "./messageVisibility";

interface MessageBubbleProps {
  message: Message;
  searchQuery: string | null;
}

export function MessageBubble({ message, searchQuery }: MessageBubbleProps) {
  if (!isDisplayableMessage(message)) {
    return null;
  }

  const isUser = message.role === "user";
  const isSystem = message.role === "system";
  const isTool = message.role === "tool";

  if (isTool && message.tool_name) {
    return (
      <ToolCall
        toolName={message.tool_name}
        toolInput={message.tool_input}
        toolOutput={message.tool_output}
        searchQuery={searchQuery}
      />
    );
  }

  const styles = isUser
    ? {
        icon: "bg-blue-500/15 text-blue-300",
        block: "border-blue-500/35 bg-blue-500/10",
        label: "You",
      }
    : isSystem
      ? {
          icon: "bg-amber-500/15 text-amber-300",
          block: "border-amber-500/35 bg-amber-500/10",
          label: "System",
        }
      : {
          icon: "bg-fuchsia-500/15 text-fuchsia-300",
          block: "border-fuchsia-500/35 bg-fuchsia-500/10",
          label: "Assistant",
        };

  return (
    <article className={`rounded-lg border-l-4 ${styles.block}`}>
      <div className="flex items-start gap-3 px-4 py-3">
        <div
          className={`mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full ${styles.icon}`}
        >
          {isUser ? (
            <User className="h-4 w-4" />
          ) : isSystem ? (
            <Terminal className="h-4 w-4" />
          ) : (
            <Bot className="h-4 w-4" />
          )}
        </div>
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-xs font-bold text-text-secondary">
              {styles.label}
            </span>
            {message.timestamp && (
              <span className="text-[11px] text-text-muted">
                {new Date(message.timestamp).toLocaleString()}
              </span>
            )}
          </div>
          {message.content ? (
            <div className="text-sm leading-6 text-text-primary">
              <MarkdownRenderer content={message.content} query={searchQuery} />
            </div>
          ) : null}
        </div>
      </div>
    </article>
  );
}
