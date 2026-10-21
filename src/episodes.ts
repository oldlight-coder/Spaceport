import { Episode, MetaDetails } from "./api/meta";

/** "S1 E3 · Title" for an episode, or its title alone. */
export function episodeHeading(ep: Episode) {
  return ep.season != null && ep.episode != null
    ? `S${ep.season} E${ep.episode} · ${ep.title}`
    : ep.title;
}

/**
 * The episode after `videoId`: the next one in its season, else the first of the next season.
 * Specials (season 0) only lead to other specials, and regular seasons never lead into them.
 */
export function nextEpisode(meta: MetaDetails, videoId: string): Episode | null {
  const current = meta.seasons.flatMap((s) => s.episodes).find((e) => e.id === videoId);
  if (!current) return null;
  const specials = current.season === 0;
  const order = meta.seasons
    .filter((s) => (s.number === 0) === specials)
    .flatMap((s) => s.episodes);
  const index = order.findIndex((e) => e.id === videoId);
  return index >= 0 && index + 1 < order.length ? order[index + 1] : null;
}
