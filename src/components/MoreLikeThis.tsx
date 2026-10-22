import { useCallback } from "react";
import { fetchCatalog, listCatalogs, MetaPreview } from "../api/catalog";
import { useLoad } from "./Loadable";
import PosterCard from "./PosterCard";

interface Props {
  id: string;
  type: string;
  genres: string[];
  /** The addon the title was found in, preferred for its own catalogs. */
  source?: string;
}

/** Most titles shown in the row. */
const LIMIT = 20;

/**
 * Titles of the same type and first genre, from a catalog that can filter by genre: the
 * title's own addon first, then any other. Nothing is shown when no catalog can.
 */
export default function MoreLikeThis({ id, type, genres, source }: Props) {
  const load = useCallback(async (): Promise<MetaPreview[]> => {
    const genre = genres[0];
    if (!genre) return [];
    const catalogs = (await listCatalogs()).filter(
      (c) => c.type === type && c.genres.some((g) => g.toLowerCase() === genre.toLowerCase()),
    );
    const catalog = catalogs.find((c) => c.transportUrl === source) ?? catalogs[0];
    if (!catalog) return [];
    const match = catalog.genres.find((g) => g.toLowerCase() === genre.toLowerCase()) ?? genre;
    const metas = await fetchCatalog(catalog, [["genre", match]]);
    return metas.filter((m) => m.id !== id).slice(0, LIMIT);
  }, [id, type, genres, source]);
  const [similar] = useLoad(load);
  if (similar.status !== "ready" || similar.data.length === 0) return null;
  return (
    <section className="row">
      <h2>
        More like this <span className="muted row-meta">{genres[0]}</span>
      </h2>
      <ul className="poster-row">
        {similar.data.map((meta) => (
          <PosterCard
            key={meta.id}
            name={meta.name}
            poster={meta.poster}
            shape={meta.posterShape}
            title={{ type: meta.type || type, id: meta.id }}
          />
        ))}
      </ul>
    </section>
  );
}
