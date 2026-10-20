import { useCallback } from "react";
import { CatalogRef, fetchCatalog, listCatalogs, typeLabel } from "../api/catalog";
import { LoadableList, useLoad } from "../components/Loadable";
import PosterCard from "../components/PosterCard";

// Catalogs that need a genre belong on the type pages, not on Home.
const homeCatalogs = () => listCatalogs().then((all) => all.filter((c) => !c.genreRequired));

/** Home: one row per browsable catalog of the installed addons. */
export default function BoardScreen() {
  const [catalogs, reload] = useLoad(homeCatalogs);
  return (
    <LoadableList state={catalogs} onRetry={reload} empty="None of your addons has catalogs.">
      {(list) => (
        <div className="rows">
          {list.map((catalog) => (
            <CatalogRow
              key={`${catalog.transportUrl} ${catalog.type} ${catalog.id}`}
              catalog={catalog}
            />
          ))}
        </div>
      )}
    </LoadableList>
  );
}

function CatalogRow({ catalog }: { catalog: CatalogRef }) {
  const load = useCallback(() => fetchCatalog(catalog), [catalog]);
  const [metas, reload] = useLoad(load);

  // An addon that answers with nothing gets no row rather than an empty one.
  if (metas.status === "ready" && metas.data.length === 0) return null;

  return (
    <section className="row">
      <h2>
        {catalog.name} <span className="muted row-meta">{typeLabel(catalog.type)} · {catalog.addonName}</span>
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
              />
            ))}
          </ul>
        )}
      </LoadableList>
    </section>
  );
}
