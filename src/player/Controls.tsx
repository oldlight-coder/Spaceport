import { useCallback, useEffect, useRef, useState } from "react";
import { onPlayerEvent } from "../api/player";
import { control, Control, currentState, onPlaybackState, PlaybackState } from "../api/playerControls";

const HIDE_AFTER_MS = 3000;
const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];

export function formatTime(seconds: number) {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const s = Math.floor(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
}

const send = (c: Control) => {
  control(c).catch(() => {});
};

/** The player controls, drawn in a transparent window over the video. */
export default function Controls() {
  const [state, setState] = useState<PlaybackState | null>(null);
  const [idle, setIdle] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  // While the seek bar is dragged, show the dragged position instead of the playing one.
  const [dragging, setDragging] = useState<number | null>(null);
  const hideTimer = useRef<number | undefined>(undefined);
  const clickTimer = useRef<number | undefined>(undefined);
  const overBar = useRef(false);

  useEffect(() => {
    currentState().then((s) => s && setState(s));
    const stopState = onPlaybackState(setState);
    const stopEvents = onPlayerEvent((event) => {
      if (event.kind === "fallback") setToast(`${event.reason}. Trying ${event.label}…`);
      if (event.kind === "loaded") window.setTimeout(() => setToast(null), 2500);
    });
    return () => {
      stopState.then((stop) => stop());
      stopEvents.then((stop) => stop());
    };
  }, []);

  const paused = state?.paused ?? false;

  // Show the controls on any activity; hide them again while playing.
  const wake = useCallback(() => {
    setIdle(false);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!overBar.current) setIdle(true);
    }, HIDE_AFTER_MS);
  }, []);

  useEffect(() => {
    if (paused) {
      window.clearTimeout(hideTimer.current);
      setIdle(false);
    } else {
      wake();
    }
  }, [paused, wake]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLSelectElement) return;
      wake();
      const volume = state?.volume ?? 100;
      switch (e.key) {
        case " ":
        case "k":
          send({ action: "togglePause" });
          break;
        case "ArrowLeft":
        case "j":
          send({ action: "seek", seconds: -10, relative: true });
          break;
        case "ArrowRight":
        case "l":
          send({ action: "seek", seconds: 10, relative: true });
          break;
        case "ArrowUp":
          send({ action: "setVolume", volume: volume + 5 });
          break;
        case "ArrowDown":
          send({ action: "setVolume", volume: volume - 5 });
          break;
        case "m":
          send({ action: "toggleMute" });
          break;
        case "f":
          send({ action: "toggleFullscreen" });
          break;
        case "Escape":
          send({ action: "back" });
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [state?.volume, wake]);

  // A single click pauses; a double click toggles fullscreen instead.
  function onSurfaceClick(e: React.MouseEvent) {
    if (e.target !== e.currentTarget) return;
    window.clearTimeout(clickTimer.current);
    clickTimer.current = window.setTimeout(() => send({ action: "togglePause" }), 220);
  }

  function onSurfaceDoubleClick(e: React.MouseEvent) {
    if (e.target !== e.currentTarget) return;
    window.clearTimeout(clickTimer.current);
    send({ action: "toggleFullscreen" });
  }

  const duration = state?.duration ?? 0;
  const position = dragging ?? state?.position ?? 0;
  const loading = !state?.stream;
  const audioTracks = state?.audioTracks ?? [];

  return (
    <div
      className={idle ? "surface idle" : "surface"}
      onMouseMove={wake}
      onClick={onSurfaceClick}
      onDoubleClick={onSurfaceDoubleClick}
    >
      <div className="top">
        <button className="icon" onClick={() => send({ action: "close" })} aria-label="Close player">
          ←
        </button>
        <div>
          <div className="title">{state?.title ?? "Spaceport"}</div>
          {state?.stream && <div className="stream">{state.stream}</div>}
        </div>
      </div>

      {(loading || state?.buffering) && (
        <div className="centre">
          <span className="badge">{loading ? "Starting stream…" : "Buffering…"}</span>
        </div>
      )}
      {toast && <div className="toast">{toast}</div>}

      <div
        className="bottom"
        onMouseEnter={() => (overBar.current = true)}
        onMouseLeave={() => (overBar.current = false)}
      >
        <div className="seek-row">
          <span className="time">{formatTime(position)}</span>
          <input
            className="seek"
            type="range"
            min={0}
            max={Math.max(duration, 1)}
            step={1}
            value={Math.min(position, Math.max(duration, 1))}
            disabled={duration <= 0}
            onChange={(e) => setDragging(Number(e.target.value))}
            onPointerUp={(e) => {
              send({ action: "seek", seconds: Number(e.currentTarget.value), relative: false });
              setDragging(null);
            }}
            onKeyUp={(e) => {
              if (dragging != null) {
                send({ action: "seek", seconds: Number(e.currentTarget.value), relative: false });
                setDragging(null);
              }
            }}
            aria-label="Seek"
          />
          <span className="time end">{formatTime(duration)}</span>
        </div>
        <div className="button-row">
          <button
            className="icon"
            onClick={() => send({ action: "togglePause" })}
            aria-label={paused ? "Play" : "Pause"}
          >
            {paused ? "▶" : "❚❚"}
          </button>
          <button
            className="icon"
            onClick={() => send({ action: "seek", seconds: -10, relative: true })}
            aria-label="Back 10 seconds"
          >
            ↺10
          </button>
          <button
            className="icon"
            onClick={() => send({ action: "seek", seconds: 30, relative: true })}
            aria-label="Forward 30 seconds"
          >
            30↻
          </button>
          <button
            className="icon"
            onClick={() => send({ action: "toggleMute" })}
            aria-label={state?.muted ? "Unmute" : "Mute"}
          >
            {state?.muted || state?.volume === 0 ? "🔇" : "🔊"}
          </button>
          <input
            className="volume"
            type="range"
            min={0}
            max={100}
            value={state?.muted ? 0 : Math.min(state?.volume ?? 100, 100)}
            onChange={(e) => send({ action: "setVolume", volume: Number(e.target.value) })}
            aria-label="Volume"
          />
          <span className="grow" />
          {audioTracks.length > 1 && (
            <select
              value={audioTracks.find((t) => t.selected)?.id ?? ""}
              onChange={(e) => send({ action: "setAudioTrack", id: Number(e.target.value) })}
              aria-label="Audio track"
            >
              {audioTracks.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.label}
                </option>
              ))}
            </select>
          )}
          <select
            value={state?.speed ?? 1}
            onChange={(e) => send({ action: "setSpeed", speed: Number(e.target.value) })}
            aria-label="Playback speed"
          >
            {SPEEDS.map((s) => (
              <option key={s} value={s}>
                {s === 1 ? "Normal speed" : `${s}×`}
              </option>
            ))}
          </select>
          <button
            className="icon"
            onClick={() => send({ action: "toggleFullscreen" })}
            aria-label="Fullscreen"
          >
            ⛶
          </button>
        </div>
      </div>
    </div>
  );
}
