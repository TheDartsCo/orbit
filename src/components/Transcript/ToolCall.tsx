import { useState } from "react";
import { ChevronDown, ChevronRight, Wrench } from "lucide-react";
import { Highlight } from "../common/Highlight";

interface ToolCallProps {
  toolName: string;
  toolInput: string | null;
  toolOutput: string | null;
  searchQuery?: string | null;
}

export function ToolCall({ toolName, toolInput, toolOutput, searchQuery }: ToolCallProps) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="overflow-hidden rounded-lg border-l-4 border-green-500/70 bg-green-500/10">
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-4 py-2.5 text-left text-sm text-text-secondary transition-colors hover:bg-green-500/10"
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 shrink-0" />
        ) : (
          <ChevronRight className="h-4 w-4 shrink-0" />
        )}
        <Wrench className="h-4 w-4 shrink-0 text-green-300" />
        <span className="font-mono text-xs font-bold text-green-300">
          {toolName}
        </span>
        <span className="ml-auto text-[11px] text-text-muted">
          {expanded ? "Hide details" : "Show details"}
        </span>
      </button>
      {expanded && (
        <div className="space-y-3 border-t border-green-500/15 px-4 py-3">
          {toolInput && (
            <div>
              <div className="mb-1 text-xs font-semibold text-text-muted">
                Input
              </div>
              <pre className="max-h-56 overflow-auto rounded-md border border-border bg-bg-primary p-3 text-xs leading-5 text-text-secondary">
                <code>{searchQuery ? <Highlight text={toolInput} query={searchQuery} /> : toolInput}</code>
              </pre>
            </div>
          )}
          {toolOutput && (
            <div>
              <div className="mb-1 text-xs font-semibold text-text-muted">
                Output
              </div>
              <pre className="max-h-72 overflow-auto rounded-md border border-border bg-bg-primary p-3 text-xs leading-5 text-text-secondary">
                <code>
                  {searchQuery
                    ? <Highlight text={toolOutput.length > 2000 ? toolOutput.slice(0, 2000) + "\n..." : toolOutput} query={searchQuery} />
                    : toolOutput.length > 2000
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
