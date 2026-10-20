import { useCallback, useState } from "react";
import { Episode, getMeta, MetaDetails } from "../api/meta";
import { RankedStream } from "../api/streams";
import { useLoad } from "../components/Loadable";
import StreamsPanel from "../components/StreamsPanel";
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

/** The video whose streams are shown in the side panel. */
interface StreamTarget {
  videoId: string;
  heading: string;
}

function Details({ meta }: { meta: MetaDetails }) {
  const [target, setTarget] = useState<StreamTarget | null>(null);
  // Until the player arrives (M4) a pick is only confirmed here.
  const [picked, setPicked] = useState<{ heading: string; stream: RankedStream } | null>(null);
  const openEpisode = (ep: Episode) =>
    setTarget({
      videoId: ep.id,
      heading:
        ep.season != null && ep.episode != null
          ? `S${ep.season} E${ep.episode} · ${ep.title}`
          : ep.title,
    });
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
            {meta.seasons.length === 0 && meta.videos.length === 0 && (
              <div>
                <button onClick={() => setTarget({ videoId: meta.id, heading: meta.name })}>
                  Streams
                </button>
              </div>
            )}
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
      {picked && (
        <div className="picked" role="status">
          <span>
            Picked for {picked.heading}:{" "}
            <strong>{picked.stream.name ?? picked.stream.addonName}</strong>
            {picked.stream.parsed.releaseGroup && ` (${picked.stream.parsed.releaseGroup})`}. The
            player comes next.
          </span>
          <button className="secondary" onClick={() => setPicked(null)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      )}
      {meta.seasons.length > 0 && <Seasons meta={meta} onOpen={openEpisode} />}
      {meta.videos.length > 0 && (
        <section>
          <h2>Videos</h2>
          <EpisodeList episodes={meta.videos} onOpen={openEpisode} />
        </section>
      )}
      {target && (
        <StreamsPanel
          key={target.videoId}
          type={meta.type}
          videoId={target.videoId}
          heading={target.heading}
          onClose={() => setTarget(null)}
          onPick={(stream) => {
            setPicked({ heading: target.heading, stream });
            setTarget(null);
          }}
        />
      )}
    </>
  );
}

const seasonLabel = (n: number) => (n === 0 ? "Specials" : `Season ${n}`);

// The last season opened per title is kept locally so returning to a title shows it again.
const seasonKey = (id: string) => `spaceport.season.${id}`;

function readSeason(id: string): number | null {
  try {
    const value = localStorage.getItem(seasonKey(id));
    return value === null ? null : Number(value);
  } catch {
    return null;
  }
}

function saveSeason(id: string, season: number) {
  try {
    localStorage.setItem(seasonKey(id), String(season));
  } catch {
    // Storage can be unavailable; the choice then only lasts for this view.
  }
}

function Seasons({ meta, onOpen }: { meta: MetaDetails; onOpen: (ep: Episode) => void }) {
  const [selected, setSelected] = useState(() => readSeason(meta.id) ?? meta.seasons[0].number);
  const season = meta.seasons.find((s) => s.number === selected) ?? meta.seasons[0];

  function select(number: number) {
    setSelected(number);
    saveSeason(meta.id, number);
  }
  return (
    <section>
      <div className="season-head">
        <h2>Episodes</h2>
        <select
          value={season.number}
          onChange={(e) => select(Number(e.target.value))}
          aria-label="Season"
        >
          {meta.seasons.map((s) => (
            <option key={s.number} value={s.number}>
              {seasonLabel(s.number)} ({s.episodes.length})
            </option>
          ))}
        </select>
      </div>
      <EpisodeList episodes={season.episodes} onOpen={onOpen} />
    </section>
  );
}

function formatDate(value: string | null) {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleDateString();
}

interface EpisodeListProps {
  episodes: Episode[];
  onOpen: (ep: Episode) => void;
}

function EpisodeList({ episodes, onOpen }: EpisodeListProps) {
  return (
    <ol className="episodes">
      {episodes.map((ep) => (
        <li key={ep.id}>
          <button className="episode" onClick={() => onOpen(ep)}>
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
          </button>
        </li>
      ))}
    </ol>
  );
}
