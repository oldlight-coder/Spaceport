import { useCallback, useState } from "react";
import { Episode, getMeta, MetaDetails } from "../api/meta";
import { useLoad } from "../components/Loadable";
import { TitleRef } from "../navigation";

interface Props {
  title: TitleRef;
  onBack: () => void;
}

export default function DetailsScreen({ title, onBack }: Props) {
  const load = useCallback(() => getMeta(title.type, title.id, title.source), [title]);
  const [meta, reload] = useLoad(load);

  return (
    <div className="details">
      <button className="secondary back" onClick={onBack}>
        ← Back
      </button>
      {meta.status === "loading" && <p className="muted">Loading…</p>}
      {meta.status === "error" && (
        <div className="inline-error">
          <p className="error">{meta.message}</p>
          <button className="secondary" onClick={reload}>
            Retry
          </button>
        </div>
      )}
      {meta.status === "ready" && <Details meta={meta.data} />}
    </div>
  );
}

function Details({ meta }: { meta: MetaDetails }) {
  const facts = [meta.releaseInfo, meta.runtime, meta.imdbRating && `IMDb ${meta.imdbRating}`].filter(
    Boolean,
  );
  return (
    <>
      <header className="details-hero">
        {meta.background && (
          <img className="backdrop" src={meta.background} alt="" referrerPolicy="no-referrer" />
        )}
        <div className="details-head">
          {meta.poster && (
            <img className="details-poster" src={meta.poster} alt="" referrerPolicy="no-referrer" />
          )}
          <div className="details-text">
            {meta.logo ? (
              <img className="details-logo" src={meta.logo} alt={meta.name} referrerPolicy="no-referrer" />
            ) : (
              <h1>{meta.name}</h1>
            )}
            {facts.length > 0 && <p className="muted">{facts.join(" · ")}</p>}
            {meta.genres.length > 0 && (
              <ul className="chips">
                {meta.genres.map((g) => (
                  <li key={g}>{g}</li>
                ))}
              </ul>
            )}
            {meta.description && <p className="description">{meta.description}</p>}
            {meta.directors.length > 0 && (
              <p>
                <span className="muted">Director </span>
                {meta.directors.join(", ")}
              </p>
            )}
            {meta.cast.length > 0 && (
              <p>
                <span className="muted">Cast </span>
                {meta.cast.join(", ")}
              </p>
            )}
          </div>
        </div>
      </header>
      {meta.seasons.length > 0 && <Seasons meta={meta} />}
      {meta.videos.length > 0 && (
        <section>
          <h2>Videos</h2>
          <EpisodeList episodes={meta.videos} />
        </section>
      )}
    </>
  );
}

const seasonLabel = (n: number) => (n === 0 ? "Specials" : `Season ${n}`);

function Seasons({ meta }: { meta: MetaDetails }) {
  const [selected, setSelected] = useState(meta.seasons[0].number);
  const season = meta.seasons.find((s) => s.number === selected) ?? meta.seasons[0];
  return (
    <section>
      <div className="season-head">
        <h2>Episodes</h2>
        <select
          value={season.number}
          onChange={(e) => setSelected(Number(e.target.value))}
          aria-label="Season"
        >
          {meta.seasons.map((s) => (
            <option key={s.number} value={s.number}>
              {seasonLabel(s.number)} ({s.episodes.length})
            </option>
          ))}
        </select>
      </div>
      <EpisodeList episodes={season.episodes} />
    </section>
  );
}

function formatDate(value: string | null) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString();
}

function EpisodeList({ episodes }: { episodes: Episode[] }) {
  return (
    <ol className="episodes">
      {episodes.map((ep) => (
        <li key={ep.id} className="episode">
          {ep.thumbnail ? (
            <img src={ep.thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer" />
          ) : (
            <div className="episode-thumb" />
          )}
          <div>
            <strong>
              {ep.episode != null && `${ep.episode}. `}
              {ep.title}
            </strong>
            {ep.released && <span className="muted"> · {formatDate(ep.released)}</span>}
            {ep.overview && <p className="muted">{ep.overview}</p>}
          </div>
        </li>
      ))}
    </ol>
  );
}
