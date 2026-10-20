import { useCallback, useEffect } from "react";
import { CatalogRef, fetchCatalog, typeLabel } from "../api/catalog";
import { LoadableList, useLoad } from "./Loadable";
import PosterCard from "./PosterCard";

interface Props {
  catalog: CatalogRef;
  extra?: [string, string][];
  /** Called with the number of titles once the row has loaded (0 on error). */
  onLoaded?: (count: number) => void;
}

/** One horizontally scrolling row of a catalog. Rows with no titles are not shown. */
export default function CatalogRow({ catalog, extra, onLoaded }: Props) {
  const load = useCallback(() => fetchCatalog(catalog, extra), [catalog, extra]);
  const [metas, reload] = useLoad(load);

  useEffect(() => {
    if (metas.status === "ready") onLoaded?.(metas.data.length);
    if (metas.status === "error") onLoaded?.(0);
  }, [metas, onLoaded]);

  if (metas.status === "ready" && metas.data.length === 0) return null;

  return (
    <section className="row">
      <h2>
        {catalog.name}{" "}
        <span className="muted row-meta">
          {typeLabel(catalog.type)} · {catalog.addonName}
        </span>
      </h2>
      <LoadableList state={metas} onRetry={reload} empty="">
        {(items) => (
          <ul className="poster-row">
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
        )}
      </LoadableList>
    </section>
  );
}
