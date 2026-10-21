import { createContext, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { onPlayerEvent } from "./api/player";
import { useWatching } from "./watching";

/** New episode ids by series id; series without new episodes are missing. */
export type NewEpisodes = Record<string, string[]>;

export const NewEpisodesContext = createContext<NewEpisodes>({});

export const useNewEpisodes = () => useContext(NewEpisodesContext);

const REFRESH = "spaceport-new-episodes-refresh";

/** Checks again, after watched marks change. */
export const refreshNewEpisodes = () => window.dispatchEvent(new Event(REFRESH));

/**
 * Checks the library, Continue Watching and Watching marks for new episodes: on start, when a
 * mark changes, after watched marks change and after the player closes (watching an episode
 * moves the last watch on).
 */
export function useNewEpisodesLoader(): NewEpisodes {
  const watching = useWatching();
  const [found, setFound] = useState<NewEpisodes>({});
  const tracked = watching.map((i) => ({ id: i.id, type: i.type, since: i.since ?? null }));
  const key = JSON.stringify(tracked);
  useEffect(() => {
    let current = true;
    const load = () =>
      invoke<NewEpisodes>("new_episodes", { tracked })
        .then((result) => current && setFound(result))
        .catch(() => {});
    load();
    window.addEventListener(REFRESH, load);
    const unlisten = onPlayerEvent((event) => {
      // Progress is saved to Stremio as the player closes; check shortly after.
      if (event.kind === "closed") window.setTimeout(load, 2500);
    });
    return () => {
      current = false;
      window.removeEventListener(REFRESH, load);
      unlisten.then((stop) => stop());
    };
    // `key` stands for `tracked`, which is rebuilt on every render.
  }, [key]);
  return found;
}
