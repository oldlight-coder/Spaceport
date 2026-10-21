import { useEffect } from "react";
import { listCatalogs } from "../api/catalog";
import { onPlayerEvent } from "../api/player";
import { continueWatching, episodeLabel, watchedShare } from "../api/progress";
import PosterCard from "../components/PosterCard";
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
          <ContinueRow />
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

/** Titles started and not finished, from the Stremio library, most recent first. */
function ContinueRow() {
  const [items, reload] = useLoad(continueWatching);
  useEffect(() => {
    const unlisten = onPlayerEvent((event) => {
      // Progress is saved to Stremio as the player closes; refresh shortly after.
      if (event.kind === "closed") window.setTimeout(reload, 2000);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [reload]);
  if (items.status !== "ready" || items.data.length === 0) return null;
  return (
    <section className="row">
      <h2>Continue Watching</h2>
      <ul className="poster-row">
        {items.data.map((item) => (
          <PosterCard
            key={item.id}
            name={item.name}
            poster={item.poster}
            title={{ type: item.type, id: item.id }}
            progress={watchedShare(item)}
            caption={episodeLabel(item.videoId, item.id)}
          />
        ))}
      </ul>
    </section>
  );
}
