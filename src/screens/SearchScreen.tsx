import { useCallback, useMemo, useState } from "react";
import { CatalogRef, searchCatalogs } from "../api/catalog";
import CatalogRow from "../components/CatalogRow";
import { LoadableList, useLoad } from "../components/Loadable";

const rowKey = (c: CatalogRef) => `${c.transportUrl} ${c.type} ${c.id}`;

/** Results for one query: a row per search catalog of every installed addon, queried at once. */
export default function SearchScreen({ query }: { query: string }) {
  const [catalogs, reload] = useLoad(searchCatalogs);
  return (
    <section>
      <h2>Results for “{query}”</h2>
      <LoadableList state={catalogs} onRetry={reload} empty="None of your addons supports search.">
        {(list) => <SearchRows key={query} catalogs={list} query={query} />}
      </LoadableList>
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
    <div className="rows">
      {catalogs.map((catalog) => (
        <SearchRow key={rowKey(catalog)} catalog={catalog} extra={extra} onLoaded={onLoaded} />
      ))}
      {finished && total === 0 && <p className="muted">No results.</p>}
    </div>
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
