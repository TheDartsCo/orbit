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
        part.toLowerCase() === query.toLowerCase() ? (
          <mark key={i} className="rounded bg-fuchsia-400/25 text-inherit px-0.5">
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
