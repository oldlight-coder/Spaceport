import { FormEvent, useState } from "react";
import { addToHistory, clearHistory, readHistory, removeFromHistory } from "../searchHistory";

interface Props {
  onSearch: (term: string) => void;
}

/** The top bar search field, with recent searches below it while it has focus. */
export default function SearchBox({ onSearch }: Props) {
  const [input, setInput] = useState("");
  const [open, setOpen] = useState(false);
  const [history, setHistory] = useState(readHistory);

  const typed = input.trim().toLowerCase();
  const shown = history.filter((t) => !typed || t.toLowerCase().includes(typed));

  function run(term: string) {
    setInput(term);
    setHistory(addToHistory(term));
    setOpen(false);
    onSearch(term);
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    const term = input.trim();
    if (term) run(term);
  }

  return (
    <form
      className="search-form spacer"
      onSubmit={submit}
      role="search"
      // Closes when focus leaves the field and its list, but not while moving between them.
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setOpen(false);
      }}
      onKeyDown={(e) => e.key === "Escape" && setOpen(false)}
    >
      <input
        type="search"
        placeholder="Search"
        value={input}
        onChange={(e) => {
          setInput(e.target.value);
          setOpen(true);
        }}
        onFocus={() => setOpen(true)}
        aria-label="Search all addons"
      />
      {open && shown.length > 0 && (
        <div className="search-history" role="listbox" aria-label="Recent searches">
          <div className="search-history-head">
            <span className="muted small">Recent searches</span>
            <button type="button" className="text-button" onClick={() => setHistory(clearHistory())}>
              Clear all
            </button>
          </div>
          <ul>
            {shown.map((term) => (
              <li key={term}>
                <button type="button" className="search-history-term" onClick={() => run(term)}>
                  {term}
                </button>
                <button
                  type="button"
                  className="search-history-remove"
                  onClick={() => setHistory(removeFromHistory(term))}
                  aria-label={`Remove ${term} from recent searches`}
                  title="Remove"
                >
                  ✕
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </form>
  );
}
