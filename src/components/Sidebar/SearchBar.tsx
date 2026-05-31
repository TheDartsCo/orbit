import { useState, useCallback, useRef } from "react";
import { Search, X } from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function SearchBar() {
  const [value, setValue] = useState("");
  const search = useAppStore((s) => s.search);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(null);

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

  return (
    <div className="relative">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
      <input
        type="text"
        value={value}
        onChange={handleChange}
        placeholder="Search sessions..."
        className="w-full bg-bg-tertiary border border-border rounded-lg pl-9 pr-8 py-2 text-sm text-text-primary placeholder-text-muted focus:outline-none focus:border-accent"
      />
      {value && (
        <button
          onClick={clear}
          className="absolute right-2 top-1/2 -translate-y-1/2 text-text-muted hover:text-text-secondary"
        >
          <X className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
