import { FormEvent, useEffect, useRef, useState } from "react";
import { SearchHit, searchAssist, typeLabel } from "../api/catalog";
import { useOpenTitle } from "../navigation";
import { addToHistory, clearHistory, readHistory, removeFromHistory } from "../searchHistory";

interface Props {
  onSearch: (term: string) => void;
}

/** Suggestions start after this many letters and this pause in typing. */
const MIN_LETTERS = 2;
const PAUSE_MS = 250;
const SUGGESTIONS = 6;

/** "Movie" rather than the page name "Movies". */
const kindLabel = (type: string) => (type === "movie" ? "Movie" : typeLabel(type));

/** The top bar search field: title suggestions while typing, recent searches below them. */
export default function SearchBox({ onSearch }: Props) {
  const openTitle = useOpenTitle();
  const [input, setInput] = useState("");
  const [open, setOpen] = useState(false);
  const [history, setHistory] = useState(readHistory);
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [busy, setBusy] = useState(false);
  // The highlighted entry for the arrow keys: suggestions first, then recent searches.
  const [active, setActive] = useState(-1);
  const asked = useRef(0);

  const typed = input.trim();
  const lowered = typed.toLowerCase();
  const recent = history.filter((t) => !lowered || t.toLowerCase().includes(lowered));
  const shownHits = typed.length >= MIN_LETTERS ? hits : [];
  const total = shownHits.length + recent.length;

  useEffect(() => {
    setActive(-1);
    if (typed.length < MIN_LETTERS) {
      setHits([]);
      setBusy(false);
      return;
    }
    const ticket = ++asked.current;
    setBusy(true);
    const timer = window.setTimeout(() => {
      searchAssist(typed, true, SUGGESTIONS)
        .then((found) => ticket === asked.current && setHits(found))
        .catch(() => ticket === asked.current && setHits([]))
        .finally(() => ticket === asked.current && setBusy(false));
    }, PAUSE_MS);
    return () => window.clearTimeout(timer);
  }, [typed]);

  function run(term: string) {
    setInput(term);
    setHistory(addToHistory(term));
    setOpen(false);
    onSearch(term);
  }

  function pick(hit: SearchHit) {
    if (typed) setHistory(addToHistory(typed));
    setOpen(false);
    setInput("");
    openTitle({ type: hit.type, id: hit.id, source: hit.transportUrl });
  }

  function submit(event: FormEvent) {
    event.preventDefault();
    // `highlighted` is -1 when the remembered row no longer exists (a list that shrank).
    const hit = highlighted >= 0 ? shownHits[highlighted] : undefined;
    const term = highlighted >= shownHits.length ? recent[highlighted - shownHits.length] : undefined;
    if (hit) return pick(hit);
    if (term) return run(term);
    if (typed) run(typed);
  }

  function changeHistory(next: string[]) {
    setHistory(next);
    setActive(-1);
  }

  function onKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Escape") return setOpen(false);
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    setOpen(true);
    if (total === 0) return;
    const step = e.key === "ArrowDown" ? 1 : -1;
    // -1 is the text field itself; the keys cycle through the list and back to it.
    setActive((current) => {
      const next = current + step;
      if (next >= total) return -1;
      if (next < -1) return total - 1;
      return next;
    });
  }

  const highlighted = active >= 0 && active < total ? active : -1;
  const showList = open && (total > 0 || (busy && typed.length >= MIN_LETTERS));

  return (
    <form
      className="search-form spacer"
      onSubmit={submit}
      role="search"
      // Closes when focus leaves the field and its list, but not while moving between them.
      onBlur={(e) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setOpen(false);
      }}
      onKeyDown={onKeyDown}
    >
      <input
        type="search"
        placeholder="Search"
        value={input}
        onChange={(e) => {
          setInput(e.target.value);
          setOpen(true);
        }}
        onFocus={() => {
          setOpen(true);
          setActive(-1);
        }}
        aria-label="Search all addons"
        aria-autocomplete="list"
        autoComplete="off"
      />
      {showList && (
        <div className="search-history" role="listbox" aria-label="Search suggestions">
          {typed.length >= MIN_LETTERS && (
            <>
              <div className="search-history-head">
                <span className="muted small">Titles</span>
                {busy && <span className="mini-spinner" aria-label="Searching" />}
              </div>
              {shownHits.length === 0 && !busy && (
                <p className="muted small search-none">No titles yet; press Enter to search.</p>
              )}
              <ul>
                {shownHits.map((hit, i) => (
                  <li key={`${hit.type} ${hit.id}`} className={i === highlighted ? "active" : undefined}>
                    <button
                      type="button"
                      className="search-suggestion"
                      onClick={() => pick(hit)}
                      onMouseEnter={() => setActive(i)}
                    >
                      {hit.poster ? (
                        <img src={hit.poster} alt="" loading="lazy" referrerPolicy="no-referrer" />
                      ) : (
                        <span className="suggestion-thumb">{hit.name.slice(0, 1)}</span>
                      )}
                      <span className="suggestion-text">
                        <span className="suggestion-name">{hit.name}</span>
                        <span className="muted small">
                          {[kindLabel(hit.type), hit.releaseInfo].filter(Boolean).join(" · ")}
                        </span>
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
          {recent.length > 0 && (
            <>
              <div className="search-history-head">
                <span className="muted small">Recent searches</span>
                <button type="button" className="text-button" onClick={() => changeHistory(clearHistory())}>
                  Clear all
                </button>
              </div>
              <ul>
                {recent.map((term, i) => (
                  <li
                    key={term}
                    className={shownHits.length + i === highlighted ? "active" : undefined}
                  >
                    <button
                      type="button"
                      className="search-history-term"
                      onClick={() => run(term)}
                      onMouseEnter={() => setActive(shownHits.length + i)}
                    >
                      {term}
                    </button>
                    <button
                      type="button"
                      className="search-history-remove"
                      onClick={() => changeHistory(removeFromHistory(term))}
                      aria-label={`Remove ${term} from recent searches`}
                      title="Remove"
                    >
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>
      )}
    </form>
  );
}
