import { useCallback, useEffect, useState } from "react";
import { Episode, getMeta, MetaDetails } from "../api/meta";
import { candidatesFrom, onPlayerEvent, openPlayer } from "../api/player";
import { titleProgress, VideoProgress, watchedShare } from "../api/progress";
import { getWatched, setWatched, WatchedMarks } from "../api/watched";
import { TitleInfo } from "../api/player";
import { formatTime } from "../format";
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

/** Saved positions by video id; refreshed whenever the player closes. */
function useTitleProgress(id: string) {
  const [progress, setProgress] = useState<Record<string, VideoProgress>>({});
  const load = useCallback(() => {
    titleProgress(id)
      .then(setProgress)
      .catch(() => {});
  }, [id]);
  useEffect(() => {
    load();
    const unlisten = onPlayerEvent((event) => {
      // The last position is saved as the player closes; read it a moment later.
      if (event.kind === "closed") window.setTimeout(load, 1000);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [load]);
  return [progress, load] as const;
}

/** Watched marks from the Stremio library; refreshed whenever the player closes. */
function useWatched(item: TitleInfo) {
  const [marks, setMarks] = useState<WatchedMarks>({ title: false, videos: [] });
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    const load = () => getWatched(item).then(setMarks).catch(() => {});
    load();
    const unlisten = onPlayerEvent((event) => {
      if (event.kind === "closed") window.setTimeout(load, 1500);
    });
    return () => {
      unlisten.then((stop) => stop());
    };
    // The item object is rebuilt on every render; its id is what identifies it.
  }, [item.id]);

  async function mark(videos: string[], watched: boolean) {
    setBusy(true);
    try {
      setMarks(await setWatched(item, videos, watched));
    } finally {
      setBusy(false);
    }
  }
  return { marks, mark, busy };
}

function Details({ meta }: { meta: MetaDetails }) {
  const [target, setTarget] = useState<StreamTarget | null>(null);
  const [notice, setNotice] = useState<{ text: string; error?: boolean } | null>(null);
  const [progress, reloadProgress] = useTitleProgress(meta.id);
  const item: TitleInfo = {
    id: meta.id,
    type: meta.type,
    name: meta.name,
    poster: meta.poster,
    videoIds: meta.videoIds,
  };
  const watched = useWatched(item);
  const watchedSet = new Set(watched.marks.videos);
  const allWatched =
    watched.marks.title ||
    (meta.videoIds.length > 0 && meta.videoIds.every((id) => watchedSet.has(id)));
  // Marking watched also clears saved positions, so progress is read again afterwards.
  const markWatched = (videos: string[], value: boolean) =>
    watched
      .mark(videos, value)
      .then(reloadProgress)
      .catch((e) => setNotice({ text: String(e), error: true }));

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
      await openPlayer(candidates, fullTitle(target.heading), item, target.videoId);
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
    isWatched: (ep) => watchedSet.has(ep.id),
    setWatched: (ids, value) => markWatched(ids, value),
    busy: watched.busy,
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
            <div className="row-actions">
              <button
                className={allWatched ? "secondary watched-toggle on" : "secondary watched-toggle"}
                onClick={() => markWatched([], !allWatched)}
                disabled={watched.busy}
              >
                {allWatched ? "✓ Watched" : "Mark as watched"}
              </button>
            </div>
            {meta.seasons.length === 0 && meta.videos.length === 0 && (
              <div className="row-actions">
                <button onClick={() => autoPlay(movieTarget)}>
                  {progress[meta.id]?.position > 0
                    ? `▶ Resume from ${formatTime(progress[meta.id].position)}`
                    : "▶ Play"}
                </button>
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
      {meta.seasons.length > 0 && <Seasons meta={meta} actions={episodeActions} progress={progress} />}
      {meta.videos.length > 0 && (
        <section>
          <h2>Videos</h2>
          <EpisodeList episodes={meta.videos} actions={episodeActions} progress={progress} />
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
  isWatched: (ep: Episode) => boolean;
  setWatched: (ids: string[], watched: boolean) => void;
  busy: boolean;
}

interface SeasonsProps {
  meta: MetaDetails;
  actions: EpisodeActions;
  progress: Record<string, VideoProgress>;
}

function Seasons({ meta, actions, progress }: SeasonsProps) {
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
        <SeasonWatchedButton episodes={season.episodes} actions={actions} />
      </div>
      <EpisodeList episodes={season.episodes} actions={actions} progress={progress} />
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
  progress: Record<string, VideoProgress>;
}

function EpisodeList({ episodes, actions, progress }: EpisodeListProps) {
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
              <EpisodeProgress progress={progress[ep.id]} />
            </div>
          </button>
          <button
            className={
              actions.isWatched(ep) ? "secondary episode-mark on" : "secondary episode-mark"
            }
            onClick={() => actions.setWatched([ep.id], !actions.isWatched(ep))}
            disabled={actions.busy}
            aria-label={
              actions.isWatched(ep) ? `Mark ${ep.title} unwatched` : `Mark ${ep.title} watched`
            }
            title={actions.isWatched(ep) ? "Watched (click to undo)" : "Mark watched"}
          >
            {actions.isWatched(ep) ? "✓" : "○"}
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

function EpisodeProgress({ progress }: { progress?: VideoProgress }) {
  const share = watchedShare(progress);
  if (share == null || !progress) return null;
  return (
    <>
      <span className="progress-bar" aria-label={`${Math.round(share * 100)}% watched`}>
        <span style={{ width: `${share * 100}%` }} />
      </span>
      <span className="muted small">Resume from {formatTime(progress.position)}</span>
    </>
  );
}

function SeasonWatchedButton({ episodes, actions }: { episodes: Episode[]; actions: EpisodeActions }) {
  const all = episodes.length > 0 && episodes.every((ep) => actions.isWatched(ep));
  return (
    <button
      className="secondary season-mark"
      onClick={() => actions.setWatched(episodes.map((ep) => ep.id), !all)}
      disabled={actions.busy}
    >
      {all ? "Mark season unwatched" : "Mark season watched"}
    </button>
  );
}
