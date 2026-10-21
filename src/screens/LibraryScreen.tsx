import { useState } from "react";
import { LibraryItem, listLibrary } from "../api/library";
import { LoadableList, useLoad } from "../components/Loadable";
import PosterCard from "../components/PosterCard";
import { setWatching, useWatching } from "../watching";

// Movies first, then series, then any other type an addon uses.
const ORDER = ["movie", "series"];
const HEADINGS: Record<string, string> = { movie: "Movies", series: "Series" };
const WATCHING = "watching";
const TAB_KEY = "spaceport.libraryTab";

/** Library items grouped by type, keeping the library's order inside each group. */
function byType(items: LibraryItem[]) {
  const groups = new Map<string, LibraryItem[]>();
  for (const item of items) {
    const list = groups.get(item.type) ?? [];
    list.push(item);
    groups.set(item.type, list);
  }
  const rank = (type: string) => (ORDER.includes(type) ? ORDER.indexOf(type) : ORDER.length);
  return [...groups.entries()]
    .sort(([a], [b]) => rank(a) - rank(b) || a.localeCompare(b))
    .map(([type, list]) => ({
      type,
      heading: HEADINGS[type] ?? type.charAt(0).toUpperCase() + type.slice(1),
      items: list,
    }));
}

function readTab() {
  try {
    return localStorage.getItem(TAB_KEY);
  } catch {
    return null;
  }
}

/** The library in tabs: titles marked as watching, then one tab per type. */
export default function LibraryScreen() {
  const [library, reload] = useLoad(listLibrary);
  const watching = useWatching();
  const [tab, setTab] = useState(readTab);

  function choose(next: string) {
    setTab(next);
    try {
      localStorage.setItem(TAB_KEY, next);
    } catch {
      // The tab is only remembered when storage works.
    }
  }

  const groups = library.status === "ready" ? byType(library.data) : [];
  const tabs = [
    { key: WATCHING, label: "Watching", count: watching.length },
    ...groups.map((g) => ({ key: g.type, label: g.heading, count: g.items.length })),
  ];
  // A remembered tab that no longer exists falls back to the first one with titles.
  const current = tabs.some((t) => t.key === tab)
    ? tab
    : (tabs.find((t) => t.count > 0) ?? tabs[0]).key;
  const group = groups.find((g) => g.type === current);

  return (
    <section>
      <h2>Library</h2>
      <div className="chip-group library-tabs" role="tablist" aria-label="Library">
        {tabs.map((t) => (
          <button
            key={t.key}
            role="tab"
            aria-selected={t.key === current}
            className={t.key === current ? "chip active" : "chip"}
            onClick={() => choose(t.key)}
          >
            {t.label} <span className="muted">{t.count}</span>
          </button>
        ))}
      </div>
      {current === WATCHING ? (
        watching.length === 0 ? (
          <p className="muted">
            Nothing here yet. Use “Mark as watching” on a title’s page to keep it here.
          </p>
        ) : (
          <ul className="poster-grid">
            {watching.map((item) => (
              <PosterCard
                key={item.id}
                name={item.name}
                poster={item.poster}
                title={{ type: item.type, id: item.id }}
                onRemove={() => setWatching(item, false)}
                removeLabel="Remove from Watching"
              />
            ))}
          </ul>
        )
      ) : (
        <LoadableList state={library} onRetry={reload} empty="Your library is empty.">
          {() => (
            <ul className="poster-grid">
              {(group?.items ?? []).map((item) => (
                <PosterCard
                  key={item.id}
                  name={item.name || item.id}
                  poster={item.poster}
                  title={{ type: item.type, id: item.id }}
                />
              ))}
            </ul>
          )}
        </LoadableList>
      )}
    </section>
  );
}
