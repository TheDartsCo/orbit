import { useState, useEffect, useCallback, useRef } from "react";
import { ChevronDown, ChevronUp, X } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function SearchNav() {
  const searchQuery = useAppStore((s) => s.filters.query) || null;
  const messages = useAppStore((s) => s.messages);
  const [totalMatches, setTotalMatches] = useState(0);
  const [currentIndex, setCurrentIndex] = useState(0);
  const markedRefs = useRef<HTMLElement[]>([]);

  useEffect(() => {
    if (!searchQuery) {
      setTotalMatches(0);
      setCurrentIndex(0);
      markedRefs.current = [];
      return;
    }

    const countMarks = () => {
      const container = document.querySelector("[data-transcript-container]");
      if (!container) return;
      const marks = Array.from(container.querySelectorAll("mark")) as HTMLElement[];
      markedRefs.current = marks;
      setTotalMatches(marks.length);
      setCurrentIndex(marks.length > 0 ? 1 : 0);
      if (marks.length > 0) {
        highlightActive(marks, 0);
      }
    };

    const timer = setTimeout(countMarks, 100);
    return () => clearTimeout(timer);
  }, [searchQuery, messages]);

  const highlightActive = (marks: HTMLElement[], idx: number) => {
    marks.forEach((m, i) => {
      if (i === idx) {
        m.style.backgroundColor = "rgba(234, 179, 8, 0.45)";
        m.scrollIntoView({ behavior: "smooth", block: "center" });
      } else {
        m.style.backgroundColor = "";
      }
    });
  };

  const goNext = useCallback(() => {
    if (markedRefs.current.length === 0) return;
    const next = currentIndex >= markedRefs.current.length ? 1 : currentIndex + 1;
    setCurrentIndex(next);
    highlightActive(markedRefs.current, next - 1);
  }, [currentIndex]);

  const goPrev = useCallback(() => {
    if (markedRefs.current.length === 0) return;
    const prev = currentIndex <= 1 ? markedRefs.current.length : currentIndex - 1;
    setCurrentIndex(prev);
    highlightActive(markedRefs.current, prev - 1);
  }, [currentIndex]);

  const clearSearch = useAppStore((s) => s.search);

  if (!searchQuery || totalMatches === 0) return null;

  return (
    <div className="absolute right-4 top-2 z-10 flex items-center gap-1 rounded-lg border border-border bg-bg-secondary px-2 py-1 shadow-lg">
      <span className="px-1.5 text-xs font-medium text-text-secondary">
        {currentIndex} of {totalMatches}
      </span>
      <button
        onClick={goPrev}
        className="rounded p-1 text-text-muted hover:bg-bg-hover hover:text-text-primary"
        aria-label="Previous match"
      >
        <ChevronUp className="h-3.5 w-3.5" />
      </button>
      <button
        onClick={goNext}
        className="rounded p-1 text-text-muted hover:bg-bg-hover hover:text-text-primary"
        aria-label="Next match"
      >
        <ChevronDown className="h-3.5 w-3.5" />
      </button>
      <button
        onClick={() => clearSearch("")}
        className="rounded p-1 text-text-muted hover:bg-bg-hover hover:text-text-primary"
        aria-label="Clear search"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
