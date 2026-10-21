import { useCallback, useEffect, useState } from "react";
import { Episode, getMeta, MetaDetails } from "../api/meta";
import { candidatesFrom, onPlayerEvent, openPlayer } from "../api/player";
import { fetchStreams, RankedStream } from "../api/streams";
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
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);

  useEffect(() => {
    const unlisten = onPlayerEvent((event) => {
      if (event.kind === "failed") {
        setNotice({ text: `Playback failed: ${event.message}`, error: true });
      }
      // A started or closed player makes "Starting…" stale; errors stay until dismissed.
      if (event.kind === "loaded" || event.kind === "closed") {
        setNotice((current) => (current?.error ? current : null));
      }
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const fullTitle = (heading: string) =>
    heading === meta.name ? meta.name : `${meta.name} · ${heading}`;

  /** Plays `first` (or the best stream) and lets the player fall back down the ranked list. */
  async function play(target: StreamTarget, ranked: RankedStream[], first?: RankedStream) {
    const candidates = candidatesFrom(ranked, first);
    if (candidates.length === 0) {
      setNotice({ text: "No playable stream was found.", error: true });
      return;
    }
    setNotice({ text: `Starting ${candidates[0].label}…` });
    try {
      await openPlayer(candidates, fullTitle(target.heading), meta.type, target.videoId);
    } catch (e) {
      setNotice({ text: String(e), error: true });
    }
  }

  /** Auto-play: rank the streams and start the best one without showing the picker. */
  async function autoPlay(target: StreamTarget) {
    setNotice({ text: `Finding the best stream for ${target.heading}…` });
    try {
      const results = await fetchStreams(meta.type, target.videoId);
      if (results.streams.length === 0) {
        setNotice({ text: "No stream passed your filters; choose one yourself.", error: true });
        setTarget(target);
        return;
      }
      await play(target, results.streams);
    } catch (e) {
      setNotice({ text: String(e), error: true });
    }
  }

  const episodeTarget = (ep: Episode): StreamTarget => ({
    videoId: ep.id,
    heading:
      ep.season != null && ep.episode != null
        ? `S${ep.season} E${ep.episode} · ${ep.title}`
        : ep.title,
  });
  const episodeActions: EpisodeActions = {
    play: (ep) => autoPlay(episodeTarget(ep)),
    choose: (ep) => setTarget(episodeTarget(ep)),
  };
  const movieTarget = { videoId: meta.id, heading: meta.name };
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
              <div className="row-actions">
                <button onClick={() => autoPlay(movieTarget)}>▶ Play</button>
                <button className="secondary" onClick={() => setTarget(movieTarget)}>
                  Choose stream
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
      {notice && (
        <div className={notice.error ? "picked error-notice" : "picked"} role="status">
          <span>{notice.text}</span>
          <button className="secondary" onClick={() => setNotice(null)} aria-label="Dismiss">
            ✕
          </button>
        </div>
      )}
      {meta.seasons.length > 0 && <Seasons meta={meta} actions={episodeActions} />}
      {meta.videos.length > 0 && (
        <section>
          <h2>Videos</h2>
          <EpisodeList episodes={meta.videos} actions={episodeActions} />
        </section>
      )}
      {target && (
        <StreamsPanel
          key={target.videoId}
          type={meta.type}
          videoId={target.videoId}
          heading={target.heading}
          onClose={() => setTarget(null)}
          onPick={(stream, ranked) => {
            setTarget(null);
            play(target, ranked, stream);
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

/** What clicking an episode does: play it, or open the picker for it. */
interface EpisodeActions {
  play: (ep: Episode) => void;
  choose: (ep: Episode) => void;
}

function Seasons({ meta, actions }: { meta: MetaDetails; actions: EpisodeActions }) {
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
      <EpisodeList episodes={season.episodes} actions={actions} />
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
  actions: EpisodeActions;
}

function EpisodeList({ episodes, actions }: EpisodeListProps) {
  return (
    <ol className="episodes">
      {episodes.map((ep) => (
        <li key={ep.id} className="episode-row">
          <button className="episode" onClick={() => actions.play(ep)} title="Play">
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
          <button
            className="secondary choose-stream"
            onClick={() => actions.choose(ep)}
            aria-label={`Choose a stream for ${ep.title}`}
            title="Choose stream"
          >
            ⋯
          </button>
        </li>
      ))}
    </ol>
  );
}
