import { useState } from "react";
import { ChevronRight, ChevronDown, Wrench } from "lucide-react";

interface ToolCallProps {
  toolName: string;
  toolInput: string | null;
  toolOutput: string | null;
}

export function ToolCall({ toolName, toolInput, toolOutput }: ToolCallProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="my-2 rounded-lg border border-border bg-bg-tertiary overflow-hidden">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-sm text-text-secondary hover:bg-bg-hover transition-colors"
      >
        {expanded ? (
          <ChevronDown className="w-4 h-4 shrink-0" />
        ) : (
          <ChevronRight className="w-4 h-4 shrink-0" />
        )}
        <Wrench className="w-4 h-4 shrink-0 text-text-muted" />
        <span className="font-mono text-xs">{toolName}</span>
      </button>
      {expanded && (
        <div className="border-t border-border px-3 py-2 space-y-2">
          {toolInput && (
            <div>
              <div className="text-xs text-text-muted mb-1">Input</div>
              <pre className="text-xs text-text-secondary bg-bg-primary rounded p-2 overflow-x-auto max-h-48 overflow-y-auto">
                <code>{toolInput}</code>
              </pre>
            </div>
          )}
          {toolOutput && (
            <div>
              <div className="text-xs text-text-muted mb-1">Output</div>
              <pre className="text-xs text-text-secondary bg-bg-primary rounded p-2 overflow-x-auto max-h-48 overflow-y-auto">
                <code>
                  {toolOutput.length > 2000
                    ? toolOutput.slice(0, 2000) + "\n..."
                    : toolOutput}
                </code>
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
