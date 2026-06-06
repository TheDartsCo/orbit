import { useEffect, useState, type ReactNode } from "react";
import { Check, Copy, X } from "lucide-react";

interface WindowsResumeModalProps {
  sessionId: string;
  command: string | null;
  error: string | null;
  onClose: () => void;
}

export function WindowsResumeModal({
  sessionId,
  command,
  error,
  onClose,
}: WindowsResumeModalProps) {
  const [copied, setCopied] = useState<"session" | "command" | null>(null);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  const copy = async (value: string, field: "session" | "command") => {
    await navigator.clipboard.writeText(value);
    setCopied(field);
    window.setTimeout(() => setCopied(null), 1500);
  };

  const CopyIcon = ({ field }: { field: "session" | "command" }) =>
    copied === field ? (
      <Check className="h-3.5 w-3.5 text-success" />
    ) : (
      <Copy className="h-3.5 w-3.5" />
    );

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60" onClick={onClose} />
      <div className="relative z-10 w-full max-w-xl rounded-lg border border-border bg-bg-secondary shadow-2xl">
        <div className="flex items-center justify-between border-b border-border px-5 py-4">
          <h2 className="text-sm font-bold text-text-primary">
            Resume on Windows
          </h2>
          <button
            onClick={onClose}
            aria-label="Close"
            className="rounded-md p-1 text-text-muted transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="space-y-4 px-5 py-4">
          <p className="text-sm text-text-secondary">
            Automatic resume will be supported soon on Windows. For now, copy
            the session details and run the command manually.
          </p>

          <CopyableField
            label="Session ID"
            value={sessionId}
            onCopy={() => copy(sessionId, "session")}
            icon={<CopyIcon field="session" />}
          />

          {command ? (
            <CopyableField
              label="Command"
              value={command}
              onCopy={() => copy(command, "command")}
              icon={<CopyIcon field="command" />}
            />
          ) : (
            <div>
              <p className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-text-secondary">
                Command
              </p>
              <p className="rounded-md border border-border bg-bg-primary px-3 py-2 text-xs text-text-muted">
                {error ?? "This adapter does not provide a resume command."}
              </p>
            </div>
          )}
        </div>

        <div className="flex justify-end border-t border-border px-5 py-3">
          <button
            onClick={onClose}
            className="rounded-md bg-bg-tertiary px-4 py-1.5 text-xs font-semibold text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

function CopyableField({
  label,
  value,
  onCopy,
  icon,
}: {
  label: string;
  value: string;
  onCopy: () => void;
  icon: ReactNode;
}) {
  return (
    <div>
      <p className="mb-1.5 text-xs font-semibold uppercase tracking-wide text-text-secondary">
        {label}
      </p>
      <div className="flex items-center gap-2 rounded-md border border-border bg-bg-primary p-2">
        <code className="min-w-0 flex-1 select-all overflow-x-auto whitespace-nowrap px-1 text-xs text-text-primary">
          {value}
        </code>
        <button
          onClick={onCopy}
          aria-label={`Copy ${label.toLowerCase()}`}
          className="shrink-0 rounded-md p-1.5 text-text-secondary transition-colors hover:bg-bg-hover hover:text-text-primary"
        >
          {icon}
        </button>
      </div>
    </div>
  );
}
