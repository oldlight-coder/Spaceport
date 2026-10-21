import { useCallback, useMemo, useState } from "react";
import { CatalogRef, searchAssist, searchCatalogs } from "../api/catalog";
import PosterCard from "../components/PosterCard";
import CatalogRow from "../components/CatalogRow";
import { LoadableList, useLoad } from "../components/Loadable";
import NeedAddons from "../components/NeedAddons";
import { PosterRowSkeleton, RowsSkeleton } from "../components/Skeleton";

const rowKey = (c: CatalogRef) => `${c.transportUrl} ${c.type} ${c.id}`;

/** Results for one query: a row per search catalog of every installed addon, queried at once. */
export default function SearchScreen({ query }: { query: string }) {
  const [catalogs, reload] = useLoad(searchCatalogs);
  return (
    <section>
      <h2>Results for “{query}”</h2>
      <LoadableList
        state={catalogs}
        onRetry={reload}
        empty={<NeedAddons text="None of your addons supports search." />}
        skeleton={<RowsSkeleton />}
      >
        {(list) => (
          <div className="rows">
            <BestMatches key={`best ${query}`} query={query} />
            <SearchRows key={query} catalogs={list} query={query} />
          </div>
        )}
      </LoadableList>
    </section>
  );
}

/** Titles closest to the query from every addon, typos forgiven, above the per-addon rows. */
function BestMatches({ query }: { query: string }) {
  const load = useCallback(() => searchAssist(query, false, 20), [query]);
  const [hits] = useLoad(load);
  if (hits.status === "loading") {
    return (
      <section className="row">
        <h2>Best matches</h2>
        <PosterRowSkeleton heading={false} />
      </section>
    );
  }
  if (hits.status !== "ready" || hits.data.length === 0) return null;
  return (
    <section className="row">
      <h2>
        Best matches <span className="muted row-meta">from all addons</span>
      </h2>
      <ul className="poster-row">
        {hits.data.map((hit) => (
          <PosterCard
            key={`${hit.type} ${hit.id}`}
            name={hit.name}
            poster={hit.poster}
            title={{ type: hit.type, id: hit.id, source: hit.transportUrl }}
            caption={hit.releaseInfo}
          />
        ))}
      </ul>
    </section>
  );
}

function SearchRows({ catalogs, query }: { catalogs: CatalogRef[]; query: string }) {
  const extra = useMemo<[string, string][]>(() => [["search", query]], [query]);
  const [counts, setCounts] = useState<Record<string, number>>({});
  const finished = Object.keys(counts).length === catalogs.length;
  const total = Object.values(counts).reduce((a, b) => a + b, 0);

  const onLoaded = useCallback(
    (key: string, count: number) => setCounts((prev) => ({ ...prev, [key]: count })),
    [],
  );

  return (
    <>
      {catalogs.map((catalog) => (
        <SearchRow key={rowKey(catalog)} catalog={catalog} extra={extra} onLoaded={onLoaded} />
      ))}
      {finished && total === 0 && <p className="muted">No exact results in your addons.</p>}
    </>
  );
}

interface SearchRowProps {
  catalog: CatalogRef;
  extra: [string, string][];
  onLoaded: (key: string, count: number) => void;
}

function SearchRow({ catalog, extra, onLoaded }: SearchRowProps) {
  const key = rowKey(catalog);
  const report = useCallback((count: number) => onLoaded(key, count), [key, onLoaded]);
  return <CatalogRow catalog={catalog} extra={extra} onLoaded={report} />;
}
