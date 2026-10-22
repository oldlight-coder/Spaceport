import { useCallback, useEffect } from "react";
import { CatalogRef, fetchCatalog, typeLabel } from "../api/catalog";
import { LoadableList, useLoad } from "./Loadable";
import PosterCard from "./PosterCard";
import { PosterRowSkeleton } from "./Skeleton";

interface Props {
  catalog: CatalogRef;
  extra?: [string, string][];
  /** Called with the number of titles once the row has loaded (0 on error). */
  onLoaded?: (count: number) => void;
  /** A failed row is one quiet line instead of a heading and an error (search results). */
  quietErrors?: boolean;
}

/** One horizontally scrolling row of a catalog. Rows with no titles are not shown. */
export default function CatalogRow({ catalog, extra, onLoaded, quietErrors }: Props) {
  const load = useCallback(() => fetchCatalog(catalog, extra), [catalog, extra]);
  const [metas, reload] = useLoad(load);

  useEffect(() => {
    if (metas.status === "ready") onLoaded?.(metas.data.length);
    if (metas.status === "error") onLoaded?.(0);
  }, [metas, onLoaded]);

  if (metas.status === "ready" && metas.data.length === 0) return null;
  if (metas.status === "error" && quietErrors) {
    const reason = metas.message.replace(/^could not load the addon: /, "");
    return (
      <p className="muted small row-failed">
        {catalog.addonName} ({typeLabel(catalog.type)}) did not answer: {reason}.{" "}
        <button className="text-button" onClick={reload}>
          Try again
        </button>
      </p>
    );
  }

  return (
    <section className="row">
      <h2>
        {catalog.name}{" "}
        <span className="muted row-meta">
          {typeLabel(catalog.type)} · {catalog.addonName}
        </span>
      </h2>
      <LoadableList
        state={metas}
        onRetry={reload}
        empty=""
        skeleton={<PosterRowSkeleton heading={false} />}
      >
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
