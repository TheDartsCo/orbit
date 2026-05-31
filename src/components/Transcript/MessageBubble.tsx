import type { Message } from "../../types";
import { MarkdownRenderer } from "./MarkdownRenderer";
import { ToolCall } from "./ToolCall";
import { User, Bot } from "lucide-react";

interface MessageBubbleProps {
  message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
  const isUser = message.role === "user";
  const isTool = message.role === "tool";

  if (isTool && message.tool_name) {
    return (
      <ToolCall
        toolName={message.tool_name}
        toolInput={message.tool_input}
        toolOutput={message.tool_output}
      />
    );
  }

  return (
    <div className="flex gap-3 py-3">
      <div
        className={`w-7 h-7 rounded-full flex items-center justify-center shrink-0 mt-0.5 ${
          isUser
            ? "bg-accent/20 text-accent"
            : "bg-bg-tertiary text-text-secondary"
        }`}
      >
        {isUser ? <User className="w-4 h-4" /> : <Bot className="w-4 h-4" />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-xs font-medium text-text-muted mb-1">
          {isUser ? "You" : "Assistant"}
        </div>
        {message.content && (
          <div className="text-sm text-text-primary">
            <MarkdownRenderer content={message.content} />
          </div>
        )}
      </div>
    </div>
  );
}
