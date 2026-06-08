import { useRef, useEffect, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Highlight } from "../common/Highlight";

interface MarkdownRendererProps {
  content: string;
  query?: string | null;
}

export function MarkdownRenderer({ content, query }: MarkdownRendererProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [renderPlainText, setRenderPlainText] = useState(false);

  useEffect(() => {
    setRenderPlainText(false);
  }, [content]);

  useEffect(() => {
    if (renderPlainText || !content.trim() || !containerRef.current) return;

    const hasVisibleText = Boolean(containerRef.current.textContent?.trim());
    const hasVisibleMedia = Boolean(
      containerRef.current.querySelector("img, svg, table, pre, code"),
    );

    if (!hasVisibleText && !hasVisibleMedia) {
      setRenderPlainText(true);
    }
  }, [content, renderPlainText]);

  useEffect(() => {
    if (!query || !query.trim() || !containerRef.current) return;

    const walker = document.createTreeWalker(
      containerRef.current,
      NodeFilter.SHOW_TEXT,
    );

    const textNodes: Text[] = [];
    while (walker.nextNode()) {
      const node = walker.currentNode as Text;
      if (node.textContent && node.textContent.toLowerCase().includes(query.toLowerCase())) {
        textNodes.push(node);
      }
    }

    const regex = new RegExp(
      `(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`,
      "gi",
    );

    for (const textNode of textNodes) {
      const text = textNode.textContent!;
      const parts = text.split(regex);
      if (parts.length === 1) continue;

      const parent = textNode.parentNode!;
      const fragment = document.createDocumentFragment();
      for (const part of parts) {
        if (part.toLowerCase() === query.toLowerCase()) {
          const mark = document.createElement("mark");
          mark.className = "rounded bg-fuchsia-400/25 text-inherit px-0.5";
          mark.textContent = part;
          fragment.appendChild(mark);
        } else {
          fragment.appendChild(document.createTextNode(part));
        }
      }
      parent.replaceChild(fragment, textNode);
    }

    return () => {
      if (!containerRef.current) return;
      const marks = containerRef.current.querySelectorAll("mark");
      for (const mark of marks) {
        const text = document.createTextNode(mark.textContent || "");
        mark.parentNode?.replaceChild(text, mark);
      }
      containerRef.current.normalize();
    };
  }, [content, query]);

  return (
    <div
      ref={containerRef}
      className="max-w-none break-words [&_*:first-child]:mt-0 [&_*:last-child]:mb-0 [&_a]:text-accent [&_a]:underline [&_blockquote]:border-l-2 [&_blockquote]:border-border-light [&_blockquote]:pl-3 [&_blockquote]:text-text-secondary [&_code]:rounded [&_code]:bg-bg-primary/70 [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-[0.92em] [&_li]:my-1 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-2 [&_pre]:my-3 [&_pre]:max-h-[420px] [&_pre]:overflow-auto [&_pre]:rounded-md [&_pre]:border [&_pre]:border-border [&_pre]:bg-bg-primary [&_pre]:p-3 [&_pre_code]:bg-transparent [&_pre_code]:p-0 [&_strong]:font-bold [&_table]:my-3 [&_table]:w-full [&_table]:border-collapse [&_td]:border [&_td]:border-border [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-border [&_th]:bg-bg-tertiary [&_th]:px-2 [&_th]:py-1 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-5"
    >
      {renderPlainText ? (
        <div className="whitespace-pre-wrap">
          {query ? <Highlight text={content} query={query} /> : content}
        </div>
      ) : (
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
          {content}
        </ReactMarkdown>
      )}
    </div>
  );
}
