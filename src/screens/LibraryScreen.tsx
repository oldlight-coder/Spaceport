import { useEffect, useState } from "react";
import { LIBRARY_CHANGED, LibraryItem, listLibrary, setInLibrary } from "../api/library";
import { LoadableList, useLoad } from "../components/Loadable";
import PosterCard from "../components/PosterCard";
import { setWatching, useWatching } from "../watching";
import { episodeLabel, watchedShare, watchHistory } from "../api/progress";
import { onPlayerEvent } from "../api/player";
import { PosterGridSkeleton } from "../components/Skeleton";

// Series first, then movies, then any other type an addon uses.
const ORDER = ["series", "movie"];
const HEADINGS: Record<string, string> = { movie: "Movies", series: "Series" };
const WATCHING = "watching";
const HISTORY = "history";
const TAB_KEY = "spaceport.libraryTab";
const SORT_KEY = "spaceport.librarySort";

const SORTS = {
  watched: "Recently watched",
  added: "Recently added",
  name: "Name",
} as const;
type Sort = keyof typeof SORTS;

interface Sortable {
  name: string;
  /** Milliseconds since 1970. */
  added: number;
  lastWatched: number | null;
}

const millis = (iso: string | null | undefined) => {
  const t = iso ? Date.parse(iso) : NaN;
  return Number.isNaN(t) ? null : t;
};

/** Newest first for the dates; titles never watched go after the watched ones, newest added
 * first. */
function sortBy<T>(list: T[], sort: Sort, key: (item: T) => Sortable) {
  const byAdded = (a: Sortable, b: Sortable) => b.added - a.added;
  const compare = (a: Sortable, b: Sortable) => {
    if (sort === "name") return a.name.localeCompare(b.name);
    if (sort === "added") return byAdded(a, b);
    if (a.lastWatched !== b.lastWatched) {
      if (a.lastWatched == null) return 1;
      if (b.lastWatched == null) return -1;
      return b.lastWatched - a.lastWatched;
    }
    return byAdded(a, b);
  };
  return [...list].sort((a, b) => compare(key(a), key(b)));
}

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

function read(key: string) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function remember(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Only remembered when storage works.
  }
}

/** The library in tabs: titles marked as watching, then one tab per type. */
export default function LibraryScreen() {
  const [library, reload, setLibrary] = useLoad(listLibrary);
  const [error, setError] = useState<string | null>(null);
  // Titles added or removed on their page show up here without a restart.
  useEffect(() => {
    window.addEventListener(LIBRARY_CHANGED, reload);
    return () => window.removeEventListener(LIBRARY_CHANGED, reload);
  }, [reload]);

  // The card goes at once; the Stremio library follows.
  async function remove(item: LibraryItem) {
    if (library.status !== "ready") return;
    setError(null);
    setLibrary({ status: "ready", data: library.data.filter((i) => i.id !== item.id) });
    try {
      await setInLibrary(
        { id: item.id, type: item.type, name: item.name, poster: item.poster, videoIds: [], genres: [] },
        false,
      );
    } catch (e) {
      setError(`Could not remove ${item.name || item.id}: ${e}`);
      reload();
    }
  }
  const watching = useWatching();
  const [tab, setTab] = useState(() => read(TAB_KEY));
  const [sort, setSort] = useState<Sort>(() => {
    const saved = read(SORT_KEY);
    return saved && saved in SORTS ? (saved as Sort) : "watched";
  });

  function choose(next: string) {
    setTab(next);
    remember(TAB_KEY, next);
  }

  function chooseSort(next: Sort) {
    setSort(next);
    remember(SORT_KEY, next);
  }

  const inLibrary = new Map(
    (library.status === "ready" ? library.data : []).map((i) => [i.id, i]),
  );
  const libraryKey = (item: LibraryItem): Sortable => ({
    name: item.name || item.id,
    added: millis(item.added) ?? 0,
    lastWatched: millis(item.lastWatched),
  });
  // A marked title's date added is when it was marked; its last watch comes from the library.
  const watchingSorted = sortBy(watching, sort, (item) => ({
    name: item.name,
    added: item.since ?? 0,
    lastWatched: millis(inLibrary.get(item.id)?.lastWatched),
  }));

  const groups = library.status === "ready" ? byType(library.data) : [];
  const tabs: { key: string; label: string; count?: number }[] = [
    { key: WATCHING, label: "Watching", count: watching.length },
    { key: HISTORY, label: "History" },
    ...groups.map((g) => ({ key: g.type, label: g.heading, count: g.items.length })),
  ];
  // A remembered tab that no longer exists falls back to the first one with titles.
  const current = tabs.some((t) => t.key === tab)
    ? tab
    : (tabs.find((t) => (t.count ?? 0) > 0) ?? tabs[0]).key;
  const group = groups.find((g) => g.type === current);

  return (
    <section>
      <h2>Library</h2>
      <div className="library-bar">
        <div className="chip-group" role="tablist" aria-label="Library">
          {tabs.map((t) => (
            <button
              key={t.key}
              role="tab"
              aria-selected={t.key === current}
              className={t.key === current ? "chip active" : "chip"}
              onClick={() => choose(t.key)}
            >
              {t.label} {t.count != null && <span className="muted">{t.count}</span>}
            </button>
          ))}
        </div>
        {current !== HISTORY && (
        <select
          value={sort}
          onChange={(e) => chooseSort(e.target.value as Sort)}
          aria-label="Sort library"
        >
          {(Object.keys(SORTS) as Sort[]).map((key) => (
            <option key={key} value={key}>
              {SORTS[key]}
            </option>
          ))}
        </select>
        )}
      </div>
      {current === HISTORY ? (
        <HistoryTab />
      ) : current === WATCHING ? (
        watching.length === 0 ? (
          <p className="muted">
            Nothing here yet. Use “Mark as watching” on a title’s page to keep it here.
          </p>
        ) : (
          <ul className="poster-grid">
            {watchingSorted.map((item) => (
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
        <>
          {error && <p className="error">{error}</p>}
          <LoadableList
          state={library}
          onRetry={reload}
          empty="Your library is empty."
          skeleton={<PosterGridSkeleton />}
        >
          {() => (
            <ul className="poster-grid">
              {sortBy(group?.items ?? [], sort, libraryKey).map((item) => (
                <PosterCard
                  key={item.id}
                  name={item.name || item.id}
                  poster={item.poster}
                  title={{ type: item.type, id: item.id }}
                  onRemove={() => remove(item)}
                  removeLabel="Remove from library"
                />
              ))}
            </ul>
          )}
          </LoadableList>
        </>
      )}
    </section>
  );
}

/** "Today", "Yesterday", "3 days ago", or the date for older plays. */
function whenWatched(iso: string) {
  const then = new Date(iso);
  if (Number.isNaN(then.getTime())) return "";
  const day = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((day(new Date()) - day(then)) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return then.toLocaleDateString();
}

/** Everything played, newest first, with the episode and when. */
function HistoryTab() {
  const [history, reload] = useLoad(watchHistory);
  useEffect(() => {
    const unlisten = onPlayerEvent((event) => {
      // Progress is saved as the player closes; read again shortly after.
      if (event.kind === "closed") window.setTimeout(reload, 2000);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [reload]);
  return (
    <LoadableList
      state={history}
      onRetry={reload}
      empty="Nothing watched yet."
      skeleton={<PosterGridSkeleton />}
    >
      {(items) => (
        <ul className="poster-grid">
          {items.map((item) => (
            <PosterCard
              key={item.id}
              name={item.name}
              poster={item.poster}
              title={{ type: item.type, id: item.id }}
              progress={watchedShare(item)}
              caption={[episodeLabel(item.videoId, item.id), whenWatched(item.lastWatched)]
                .filter(Boolean)
                .join(" · ")}
            />
          ))}
        </ul>
      )}
    </LoadableList>
  );
}
