import { useState, useCallback, useEffect, useMemo, useRef } from "react";
import { Search, X } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function SearchBar() {
  const [value, setValue] = useState("");
  const search = useAppStore((s) => s.search);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const isMac = useMemo(
    () =>
      typeof navigator !== "undefined" &&
      /mac|iphone|ipad|ipod/i.test(navigator.platform),
    []
  );
  const shortcutLabel = isMac ? "⌘F" : "Ctrl F";

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const v = e.target.value;
      setValue(v);
      if (debounceRef.current != null) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(() => search(v), 300);
    },
    [search]
  );

  const clear = useCallback(() => {
    setValue("");
    search("");
  }, [search]);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      const modifierPressed = isMac ? event.metaKey : event.ctrlKey;
      if (!modifierPressed || event.key.toLowerCase() !== "f") return;

      event.preventDefault();
      inputRef.current?.focus();
      inputRef.current?.select();
    };

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [isMac]);

  return (
    <div className="relative min-w-0 flex-1">
      <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted" />
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={handleChange}
        placeholder="Search sessions"
        className="h-9 w-full rounded-lg border border-border-light bg-bg-secondary pl-9 pr-24 text-sm text-text-primary shadow-inner placeholder:text-text-muted focus:border-accent focus:outline-none"
      />
      <span className="pointer-events-none absolute right-8 top-1/2 -translate-y-1/2 rounded border border-border bg-bg-tertiary px-1.5 py-0.5 text-[10px] font-semibold text-text-muted">
        {shortcutLabel}
      </span>
      {value && (
        <button
          onClick={clear}
          className="absolute right-2 top-1/2 -translate-y-1/2 rounded p-0.5 text-text-muted hover:bg-bg-hover hover:text-text-secondary"
          aria-label="Clear search"
        >
          <X className="h-4 w-4" />
        </button>
      )}
    </div>
  );
}
