import { listCatalogs } from "../api/catalog";
import CatalogRow from "../components/CatalogRow";
import { LoadableList, useLoad } from "../components/Loadable";

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
