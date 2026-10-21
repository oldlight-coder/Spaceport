import { useCallback, useEffect, useRef, useState } from "react";
import { CatalogRef, fetchCatalog, listCatalogs, MetaPreview } from "../api/catalog";
import { LoadableList, useLoad } from "../components/Loadable";
import NeedAddons from "../components/NeedAddons";
import PosterCard from "../components/PosterCard";

interface Props {
  type: string;
  title: string;
}

/** Movies, Series or Anime: one catalog at a time, with a genre filter and infinite scroll. */
export default function DiscoverScreen({ type, title }: Props) {
  const load = useCallback(
    () => listCatalogs().then((all) => all.filter((c) => c.type === type)),
    [type],
  );
  const [catalogs, reload] = useLoad(load);
  return (
    <section>
      <h2>{title}</h2>
      <LoadableList
        state={catalogs}
        onRetry={reload}
        empty={<NeedAddons text={`None of your addons has ${title.toLowerCase()} catalogs.`} />}
      >
        {(list) => <CatalogBrowser key={type} catalogs={list} />}
      </LoadableList>
    </section>
  );
}

const catalogKey = (c: CatalogRef) => `${c.transportUrl} ${c.id}`;

function CatalogBrowser({ catalogs }: { catalogs: CatalogRef[] }) {
  const [catalogIndex, setCatalogIndex] = useState(0);
  const catalog = catalogs[Math.min(catalogIndex, catalogs.length - 1)];
  const [genre, setGenre] = useState(catalog.genreRequired ? catalog.genres[0] : "");

  function selectCatalog(index: number) {
    const next = catalogs[index];
    setCatalogIndex(index);
    setGenre(next.genreRequired ? next.genres[0] : "");
  }

  return (
    <>
      <div className="filters">
        <select
          value={catalogIndex}
          onChange={(e) => selectCatalog(Number(e.target.value))}
          aria-label="Catalog"
        >
          {catalogs.map((c, i) => (
            <option key={catalogKey(c)} value={i}>
              {c.name} · {c.addonName}
            </option>
          ))}
        </select>
        {catalog.genres.length > 0 && (
          <select value={genre} onChange={(e) => setGenre(e.target.value)} aria-label="Genre">
            {!catalog.genreRequired && <option value="">All genres</option>}
            {catalog.genres.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
        )}
      </div>
      <InfiniteGrid key={`${catalogKey(catalog)} ${genre}`} catalog={catalog} genre={genre} />
    </>
  );
}

/** Loads further pages with `skip` while the end of the grid is on screen. */
function InfiniteGrid({ catalog, genre }: { catalog: CatalogRef; genre: string }) {
  const [items, setItems] = useState<MetaPreview[]>([]);
  const [loading, setLoading] = useState(false);
  const [done, setDone] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sentinel = useRef<HTMLDivElement>(null);
  // The observer can fire again before React re-renders, so paging state is read from refs.
  const paging = useRef({ items: [] as MetaPreview[], done: false, inFlight: false });

  const loadMore = useCallback(async () => {
    const state = paging.current;
    if (state.inFlight || state.done) return;
    state.inFlight = true;
    setLoading(true);
    setError(null);
    const extra: [string, string][] = [];
    if (genre) extra.push(["genre", genre]);
    if (state.items.length > 0) extra.push(["skip", String(state.items.length)]);
    try {
      const page = await fetchCatalog(catalog, extra);
      const seen = new Set(state.items.map((m) => m.id));
      const fresh = page.filter((m) => !seen.has(m.id));
      state.items = [...state.items, ...fresh];
      setItems(state.items);
      // Stop when the addon cannot page, runs out, or repeats the same page.
      if (!catalog.supportsSkip || fresh.length === 0) {
        state.done = true;
        setDone(true);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      state.inFlight = false;
      setLoading(false);
    }
  }, [catalog, genre]);

  useEffect(() => {
    const node = sentinel.current;
    if (!node || error) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadMore();
      },
      { rootMargin: "600px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [loadMore, error, items]);

  return (
    <>
      <ul className="poster-grid">
        {items.map((meta) => (
          <PosterCard
            key={meta.id}
            name={meta.name}
            poster={meta.poster}
            shape={meta.posterShape}
            title={{ type: meta.type || catalog.type, id: meta.id, source: catalog.transportUrl }}
          />
        ))}
      </ul>
      {error && (
        <div className="inline-error">
          <p className="error">{error}</p>
          <button className="secondary" onClick={loadMore}>
            Retry
          </button>
        </div>
      )}
      {loading && <p className="muted">Loading…</p>}
      {done && items.length === 0 && <p className="muted">Nothing found.</p>}
      <div ref={sentinel} className="sentinel" />
    </>
  );
}
