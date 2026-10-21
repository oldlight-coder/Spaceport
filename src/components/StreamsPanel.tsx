import { useCallback, useEffect, useMemo, useState } from "react";
import { fetchStreams, noStreamsReason, RankedStream, Stream } from "../api/streams";
import NeedAddons from "./NeedAddons";
import { useLoad } from "./Loadable";
import StreamTags from "./StreamTags";

interface Props {
  type: string;
  videoId: string;
  heading: string;
  onClose: () => void;
  /** Called when the user picks a stream to play, with the ranked list for fallbacks. */
  onPick: (stream: RankedStream, ranked: RankedStream[]) => void;
}

function streamKind(stream: Stream) {
  if (stream.infoHash) return "Torrent";
  if (stream.url) return "Direct";
  if (stream.ytId) return "YouTube";
  return "Web";
}

/** Only direct links and torrents can be played inside Spaceport. */
const playable = (s: Stream) => Boolean(s.url || s.infoHash);

const QUALITIES = [
  { key: "all", label: "All", test: () => true },
  { key: "2160", label: "4K", test: (r: number | null) => r != null && r >= 2160 },
  { key: "1080", label: "1080p", test: (r: number | null) => r != null && r >= 1080 && r < 2160 },
  { key: "720", label: "720p", test: (r: number | null) => r != null && r >= 720 && r < 1080 },
  { key: "other", label: "Other", test: (r: number | null) => r == null || r < 720 },
] as const;
type QualityKey = (typeof QUALITIES)[number]["key"];

/** The stream picker: every stream the addons returned, ranked best first. */
export default function StreamsPanel({ type, videoId, heading, onClose, onPick }: Props) {
  const load = useCallback(() => fetchStreams(type, videoId), [type, videoId]);
  const [results, reload] = useLoad(load);
  const [quality, setQuality] = useState<QualityKey>("all");
  const [addon, setAddon] = useState("");

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const streams = results.status === "ready" ? results.data.streams : [];
  const qualityTest = QUALITIES.find((q) => q.key === quality)!.test;
  const shown = useMemo(
    () =>
      streams.filter(
        (s) => qualityTest(s.parsed.resolution) && (!addon || s.transportUrl === addon),
      ),
    [streams, qualityTest, addon],
  );
  const addonsWithStreams =
    results.status === "ready" ? results.data.addons.filter((a) => a.count > 0) : [];

  return (
    <aside className="streams-panel" aria-label="Streams">
      <header className="streams-head">
        <div>
          <h2>{heading}</h2>
          <span className="muted small">Choose a stream</span>
        </div>
        <button className="secondary" onClick={onClose} aria-label="Close streams">
          ✕
        </button>
      </header>
      {results.status === "loading" && <p className="muted">Asking your addons…</p>}
      {results.status === "error" && (
        <div className="inline-error">
          <p className="error">{results.message}</p>
          <button className="secondary" onClick={reload}>
            Retry
          </button>
        </div>
      )}
      {results.status === "ready" && (
        <>
          <ul className="addon-outcomes">
            {results.data.addons.map((a) => (
              <li key={a.transportUrl} className={a.error ? "error" : "muted"}>
                {a.addonName}: {a.error ?? `${a.count} stream${a.count === 1 ? "" : "s"}`}
              </li>
            ))}
          </ul>
          {results.data.addons.length === 0 ? (
            <NeedAddons text={noStreamsReason(results.data)!} />
          ) : (
            streams.length === 0 && (
              <div className="inline-error">
                <p className="muted">{noStreamsReason(results.data)}</p>
                <button className="secondary" onClick={reload}>
                  Try again
                </button>
              </div>
            )
          )}
          {streams.length > 0 && (
            <div className="picker-filters">
              <div className="chip-group" role="group" aria-label="Quality">
                {QUALITIES.map((q) => {
                  const count = streams.filter((s) => q.test(s.parsed.resolution)).length;
                  if (q.key !== "all" && count === 0) return null;
                  return (
                    <button
                      key={q.key}
                      className={quality === q.key ? "chip active" : "chip"}
                      onClick={() => setQuality(q.key)}
                    >
                      {q.label} <span className="muted">{count}</span>
                    </button>
                  );
                })}
              </div>
              {addonsWithStreams.length > 1 && (
                <select value={addon} onChange={(e) => setAddon(e.target.value)} aria-label="Addon">
                  <option value="">All addons</option>
                  {addonsWithStreams.map((a) => (
                    <option key={a.transportUrl} value={a.transportUrl}>
                      {a.addonName}
                    </option>
                  ))}
                </select>
              )}
            </div>
          )}
          <ol className="stream-list">
            {shown.map((s) => (
              <StreamItem
                key={streams.indexOf(s)}
                stream={s}
                best={s === streams[0]}
                onPick={(picked) => onPick(picked, streams)}
              />
            ))}
          </ol>
          {streams.length > 0 && shown.length === 0 && (
            <p className="muted">No streams match these choices.</p>
          )}
          {results.data.hidden.length > 0 && (
            <details className="hidden-streams">
              <summary>{results.data.hidden.length} hidden by your filters</summary>
              <ol className="stream-list">
                {results.data.hidden.map((s, i) => (
                  <StreamItem
                    key={i}
                    stream={s}
                    note={s.hiddenBecause}
                    onPick={(picked) => onPick(picked, streams)}
                  />
                ))}
              </ol>
            </details>
          )}
        </>
      )}
    </aside>
  );
}

interface StreamItemProps {
  stream: RankedStream;
  best?: boolean;
  note?: string;
  onPick: (stream: RankedStream) => void;
}

function StreamItem({ stream: s, best, note, onPick }: StreamItemProps) {
  const canPlay = playable(s);
  return (
    <li className={note ? "stream filtered" : "stream"}>
      <div className="stream-top">
        <div className="stream-name">
          <span className="badge">{streamKind(s)}</span> {s.name ?? s.addonName}
        </div>
        <span className="stream-score" title="Spaceport score">
          {best && <span className="best">Best</span>} {Math.round(s.score.total)}
        </span>
      </div>
      {note && <p className="error small">{note}</p>}
      <StreamTags parsed={s.parsed} />
      {s.title && <pre className="stream-title">{s.title}</pre>}
      <div className="stream-actions">
        <details className="score-parts">
          <summary>Why this score</summary>
          <ul>
            {s.score.parts.map((p, j) => (
              <li key={j} className={p.points < 0 ? "minus" : undefined}>
                <span>{p.reason}</span>
                <span>
                  {p.points > 0 ? "+" : ""}
                  {p.points}
                </span>
              </li>
            ))}
          </ul>
        </details>
        <button
          className="play-button"
          onClick={() => onPick(s)}
          disabled={!canPlay}
          title={canPlay ? undefined : "This stream cannot be played in Spaceport"}
        >
          ▶ Play
        </button>
      </div>
    </li>
  );
}
