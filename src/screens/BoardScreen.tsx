import { useEffect, useState } from "react";
import { listCatalogs } from "../api/catalog";
import { onPlayerEvent } from "../api/player";
import { continueWatching, dismissContinue, episodeLabel, watchedShare } from "../api/progress";
import PosterCard from "../components/PosterCard";
import CatalogRow from "../components/CatalogRow";
import { LoadableList, useLoad } from "../components/Loadable";
import NeedAddons from "../components/NeedAddons";

// Catalogs that need a genre belong on the type pages, not on Home.
const homeCatalogs = () => listCatalogs().then((all) => all.filter((c) => !c.genreRequired));

/** Home: one row per browsable catalog of the installed addons. */
export default function BoardScreen() {
  const [catalogs, reload] = useLoad(homeCatalogs);
  return (
    <LoadableList
      state={catalogs}
      onRetry={reload}
      empty={<NeedAddons text="None of your addons has catalogs. Install one to browse here." />}
    >
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
  const [items, reload, setItems] = useLoad(continueWatching);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const unlisten = onPlayerEvent((event) => {
      // Progress is saved to Stremio as the player closes; refresh shortly after.
      if (event.kind === "closed") window.setTimeout(reload, 2000);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [reload]);
  if (items.status !== "ready" || (items.data.length === 0 && !error)) return null;

  // The row updates at once; the Stremio library follows.
  async function dismiss(ids: string[]) {
    if (items.status !== "ready") return;
    setError(null);
    setItems({ status: "ready", data: items.data.filter((i) => !ids.includes(i.id)) });
    try {
      await dismissContinue(ids);
    } catch (e) {
      setError(`Could not update Continue Watching: ${e}`);
      reload();
    }
  }

  return (
    <section className="row">
      <h2 className="row-head">
        Continue Watching
        {items.data.length > 1 && (
          <button className="text-button" onClick={() => dismiss(items.data.map((i) => i.id))}>
            Clear all
          </button>
        )}
      </h2>
      {error && <p className="error small">{error}</p>}
      <ul className="poster-row">
        {items.data.map((item) => (
          <PosterCard
            key={item.id}
            name={item.name}
            poster={item.poster}
            title={{ type: item.type, id: item.id }}
            progress={watchedShare(item)}
            caption={episodeLabel(item.videoId, item.id)}
            onRemove={() => dismiss([item.id])}
            removeLabel="Remove from Continue Watching"
          />
        ))}
      </ul>
    </section>
  );
}
