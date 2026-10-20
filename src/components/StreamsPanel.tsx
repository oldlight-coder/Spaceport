import { useCallback, useEffect } from "react";
import { fetchStreams, Stream } from "../api/streams";
import { useLoad } from "./Loadable";
import StreamTags from "./StreamTags";

interface Props {
  type: string;
  videoId: string;
  heading: string;
  onClose: () => void;
}

function streamKind(stream: Stream) {
  if (stream.infoHash) return "Torrent";
  if (stream.url) return "Direct";
  if (stream.ytId) return "YouTube";
  return "Web";
}

/** Side panel listing every stream the installed addons returned for one video. */
export default function StreamsPanel({ type, videoId, heading, onClose }: Props) {
  const load = useCallback(() => fetchStreams(type, videoId), [type, videoId]);
  const [results, reload] = useLoad(load);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside className="streams-panel" aria-label="Streams">
      <header className="streams-head">
        <h2>{heading}</h2>
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
          {results.data.addons.length === 0 && (
            <p className="muted">None of your addons provides streams for this title.</p>
          )}
          <ol className="stream-list">
            {results.data.streams.map((s, i) => (
              <li key={i} className="stream">
                <div className="stream-name">
                  <span className="badge">{streamKind(s)}</span> {s.name ?? s.addonName}
                </div>
                <StreamTags parsed={s.parsed} />
                {s.title && <pre className="stream-title">{s.title}</pre>}
              </li>
            ))}
          </ol>
        </>
      )}
    </aside>
  );
}
