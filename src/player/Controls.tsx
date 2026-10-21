import { ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { onPlayerEvent, PLAY_NEXT_EVENT, PLAYER_KEY_EVENT } from "../api/player";
import { formatTime } from "../format";
import {
  control,
  Control,
  currentState,
  loadSubtitleFile,
  onPlaybackState,
  PlaybackState,
  SkipSegment,
} from "../api/playerControls";

const HIDE_AFTER_MS = 3000;
/** The next-episode card appears this many seconds before the end... */
const NEXT_UP_BEFORE_END = 30;
/** ...and counts down from this many seconds. */
const NEXT_UP_COUNTDOWN = 10;
const SKIP_SECONDS = 10;
const SUB_DELAY_STEP = 0.1;
const SUB_SCALE_STEP = 0.1;
const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
/** A seek counts as done once playback is this close to the target, in seconds. */
const SEEK_TOLERANCE = 3;
/** Stop holding the seek target after this long, in case the seek never lands. */
const SEEK_HOLD_MS = 30_000;


const send = (c: Control) => {
  control(c).catch(() => {});
};

type MenuName = "audio" | "subtitles" | "source" | "speed";

interface Option {
  key: string;
  label: string;
  detail?: string;
  selected: boolean;
  onSelect: () => void;
}

interface Section {
  heading?: string;
  options: Option[];
  empty?: string;
  /** Extra controls shown under the heading, before the options. */
  content?: ReactNode;
}

/** A button in the control bar that opens a list of choices above it. */
function Menu({
  name,
  label,
  open,
  onToggle,
  sections,
}: {
  name: MenuName;
  label: string;
  open: boolean;
  onToggle: (name: MenuName) => void;
  sections: Section[];
}) {
  return (
    <div className="menu">
      <button
        className={open ? "menu-button open" : "menu-button"}
        onClick={() => onToggle(name)}
        aria-expanded={open}
      >
        {label}
      </button>
      {open && (
        <div className="menu-panel" role="menu">
          {sections.map((section, i) => (
            <div key={i} className="menu-section">
              {section.heading && <div className="menu-heading">{section.heading}</div>}
              {section.content}
              {section.options.length === 0 && section.empty && (
                <div className="menu-empty">{section.empty}</div>
              )}
              {section.options.map((o) => (
                <button
                  key={o.key}
                  className={o.selected ? "menu-option selected" : "menu-option"}
                  role="menuitemradio"
                  aria-checked={o.selected}
                  onClick={o.onSelect}
                >
                  <span className="check">{o.selected ? "✓" : ""}</span>
                  <span>{o.label}</span>
                  {o.detail && <span className="menu-detail">{o.detail}</span>}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Counts down to the next episode near the end of this one. Ending the countdown, pressing
 * Play now, or the episode ending asks the main window to start the next episode; Cancel stops
 * it for this episode.
 */
function useNextUp(state: PlaybackState | null) {
  const [cancelled, setCancelled] = useState(false);
  const [shownAt, setShownAt] = useState<number | null>(null);
  const [now, setNow] = useState(Date.now());
  // The next episode was asked for; the card stays away until the next video starts.
  const [requested, setRequested] = useState(false);
  const title = state?.title;

  // A new video starts with a fresh countdown.
  useEffect(() => {
    setCancelled(false);
    setShownAt(null);
    setRequested(false);
  }, [title]);

  const remaining = state ? state.duration - state.position : Infinity;
  const active =
    !!state?.nextUp &&
    !!state.stream &&
    state.duration > 60 &&
    remaining <= NEXT_UP_BEFORE_END &&
    !cancelled &&
    !requested;

  useEffect(() => {
    if (active && shownAt == null) {
      const start = Date.now();
      setShownAt(start);
      setNow(start);
    }
    if (!active) setShownAt(null);
  }, [active, shownAt]);

  useEffect(() => {
    if (shownAt == null) return;
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [shownAt]);

  const playNext = useCallback(() => {
    if (requested) return;
    setRequested(true);
    setShownAt(null);
    emit(PLAY_NEXT_EVENT).catch(() => {});
  }, [requested]);

  const elapsed = shownAt == null ? 0 : Math.max(0, now - shownAt);
  const secondsLeft =
    shownAt == null ? null : Math.max(0, NEXT_UP_COUNTDOWN - Math.floor(elapsed / 1000));

  useEffect(() => {
    if (secondsLeft === 0) playNext();
  }, [secondsLeft, playNext]);

  // The episode ended before the countdown did.
  useEffect(() => {
    const stop = onPlayerEvent((event) => {
      if (event.kind === "ended" && state?.nextUp && !cancelled) playNext();
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  }, [state?.nextUp, cancelled, playNext]);

  return {
    label: state?.nextUp ?? null,
    secondsLeft,
    playNext,
    cancel: () => {
      setCancelled(true);
      setShownAt(null);
    },
  };
}

const SKIP_LABELS: Record<SkipSegment["kind"], string> = {
  intro: "Skip Intro",
  outro: "Skip Outro",
  recap: "Skip Recap",
};

/**
 * The AniSkip segment playback is in, if any. With auto-skip on, each segment is jumped past
 * once on entering it (seeking back into it afterwards plays it).
 */
function useSkip(
  state: PlaybackState | null,
  seekTo: (seconds: number) => void,
  notify: (text: string) => void,
) {
  const skipped = useRef<Set<string>>(new Set());
  const title = state?.title;
  useEffect(() => {
    skipped.current = new Set();
  }, [title]);

  const position = state?.position ?? 0;
  // The last second of a segment is left alone, so a skip does not land back in it.
  const current = (state?.skips ?? []).find((s) => position >= s.start && position < s.end - 1);
  const key = current ? `${current.kind}:${current.start}` : null;

  useEffect(() => {
    if (!current || !key || !state?.autoSkip || skipped.current.has(key)) return;
    skipped.current.add(key);
    seekTo(current.end);
    notify(`Skipped ${current.kind}`);
  }, [key, current, state?.autoSkip, seekTo, notify]);

  return current && !state?.autoSkip
    ? { label: SKIP_LABELS[current.kind], skip: () => seekTo(current.end) }
    : null;
}

/** The player controls, drawn in a transparent window over the video. */
export default function Controls() {
  const [state, setState] = useState<PlaybackState | null>(null);
  const [idle, setIdle] = useState(false);
  const [toast, setToast] = useState<string | null>(null);
  const [menu, setMenu] = useState<MenuName | null>(null);
  // While the seek bar is dragged, show the dragged position instead of the playing one.
  const [dragging, setDragging] = useState<number | null>(null);
  // After a seek, keep showing the target until playback gets there: seeking into a part that
  // has not loaded yet can take a few seconds.
  const [pendingSeek, setPendingSeek] = useState<{ target: number; at: number } | null>(null);
  // "Resumed at …" with a way back to the start, shown once per video.
  const [resumeNotice, setResumeNotice] = useState<number | null>(null);
  const resumeShownFor = useRef<string | null>(null);
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

  useEffect(() => {
    if (!state?.stream || state.resumedFrom == null) return;
    if (resumeShownFor.current === state.title) return;
    resumeShownFor.current = state.title;
    setResumeNotice(state.resumedFrom);
    const timer = window.setTimeout(() => setResumeNotice(null), 8000);
    return () => window.clearTimeout(timer);
  }, [state?.stream, state?.resumedFrom, state?.title]);

  // Release the held seek target once playback has reached it (or given up).
  useEffect(() => {
    if (!pendingSeek || !state) return;
    const arrived = Math.abs(state.position - pendingSeek.target) <= SEEK_TOLERANCE;
    if ((arrived && !state.seeking) || Date.now() - pendingSeek.at > SEEK_HOLD_MS) {
      setPendingSeek(null);
    }
  }, [state, pendingSeek]);

  const seekTo = useCallback(
    (target: number) => {
      const duration = state?.duration ?? 0;
      const clamped = Math.max(0, duration > 0 ? Math.min(target, duration - 1) : target);
      setPendingSeek({ target: clamped, at: Date.now() });
      if (state && !state.seekable) {
        setToast("This stream only allows seeking within the part that has loaded.");
      }
      send({ action: "seek", seconds: clamped, relative: false });
    },
    [state],
  );

  const skip = useCallback(
    (delta: number) => {
      const from = pendingSeek?.target ?? state?.position ?? 0;
      seekTo(from + delta);
    },
    [pendingSeek, state, seekTo],
  );

  const paused = state?.paused ?? false;
  const nextUp = useNextUp(state);
  const showToast = useCallback((text: string) => {
    setToast(text);
    window.setTimeout(() => setToast((current) => (current === text ? null : current)), 2500);
  }, []);
  const skipSegment = useSkip(state, seekTo, showToast);

  // Show the controls on any activity; hide them again while playing.
  const wake = useCallback(() => {
    setIdle(false);
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!overBar.current) {
        setIdle(true);
        setMenu(null);
      }
    }, HIDE_AFTER_MS);
  }, []);

  useEffect(() => {
    if (paused || menu) {
      window.clearTimeout(hideTimer.current);
      setIdle(false);
    } else {
      wake();
    }
  }, [paused, menu, wake]);

  // Keys pressed here or forwarded by the main window (PLAYER_KEY_EVENT). Returns whether the
  // key did something.
  const handleKey = useCallback(
    (key: string): boolean => {
      wake();
      const volume = state?.volume ?? 100;
      switch (key.length === 1 ? key.toLowerCase() : key) {
        case " ":
        case "k":
          send({ action: "togglePause" });
          break;
        case "ArrowLeft":
        case "j":
          skip(-SKIP_SECONDS);
          break;
        case "ArrowRight":
        case "l":
          skip(SKIP_SECONDS);
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
        case "z":
          send({ action: "adjustSubtitleDelay", seconds: -SUB_DELAY_STEP });
          break;
        case "x":
          send({ action: "adjustSubtitleDelay", seconds: SUB_DELAY_STEP });
          break;
        case "Escape":
          if (menu) setMenu(null);
          else send({ action: "back" });
          break;
        default:
          return false;
      }
      return true;
    },
    [state?.volume, wake, skip, menu],
  );

  // Registered once: the state changes four times a second, and re-registering the forwarded
  // key listener that often would drop keys sent in between.
  const keyHandler = useRef(handleKey);
  keyHandler.current = handleKey;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (keyHandler.current(e.key)) e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    const unlisten = listen<string>(PLAYER_KEY_EVENT, (e) => keyHandler.current(e.payload));
    return () => {
      window.removeEventListener("keydown", onKey);
      unlisten.then((stop) => stop());
    };
  }, []);

  // A single click pauses (or closes an open menu); a double click toggles fullscreen.
  function onSurfaceClick(e: React.MouseEvent) {
    if (e.target !== e.currentTarget) return;
    if (menu) {
      setMenu(null);
      return;
    }
    window.clearTimeout(clickTimer.current);
    clickTimer.current = window.setTimeout(() => send({ action: "togglePause" }), 220);
  }

  function onSurfaceDoubleClick(e: React.MouseEvent) {
    if (e.target !== e.currentTarget) return;
    window.clearTimeout(clickTimer.current);
    send({ action: "toggleFullscreen" });
  }

  const toggleMenu = (name: MenuName) => setMenu((current) => (current === name ? null : name));
  const choose = (c: Control) => {
    send(c);
    setMenu(null);
  };

  const duration = state?.duration ?? 0;
  const position = dragging ?? pendingSeek?.target ?? state?.position ?? 0;
  const loading = !state?.stream;
  const seeking = pendingSeek != null || state?.seeking;

  const audioTracks = state?.audioTracks ?? [];
  const embeddedSubs = (state?.subtitleTracks ?? []).filter((t) => !t.external);
  const addonSubs = state?.addonSubtitles ?? [];
  const subtitlesOff =
    !embeddedSubs.some((t) => t.selected) && !addonSubs.some((s) => s.selected);
  const currentAudio = audioTracks.find((t) => t.selected);
  const currentSub =
    embeddedSubs.find((t) => t.selected)?.label ?? addonSubs.find((s) => s.selected)?.label;
  const sources = state?.sources ?? [];
  const subDelay = state?.subDelay ?? 0;
  const subScale = state?.subScale ?? 1;
  const setScale = (scale: number) => {
    const clamped = Math.min(2, Math.max(0.5, scale));
    send({ action: "setSubtitleScale", scale: Math.round(clamped * 10) / 10 });
  };
  const speed = state?.speed ?? 1;

  let badge: ReactNode = null;
  if (loading) badge = "Starting stream…";
  else if (seeking) badge = "Seeking…";
  else if (state?.buffering) badge = "Buffering…";

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
          {state && state.skips.length === 0 && state.skipNote && (
            <div className="stream">AniSkip: {state.skipNote}</div>
          )}
        </div>
      </div>

      {badge && (
        <div className="centre">
          <span className="badge">{badge}</span>
        </div>
      )}
      {toast && <div className="toast">{toast}</div>}
      {/* Near the end the next-episode card takes this place and offers Play now instead. */}
      {skipSegment && nextUp.secondsLeft == null && (
        <button className="skip-button" onClick={skipSegment.skip}>
          {skipSegment.label} ⏭
        </button>
      )}
      {nextUp.secondsLeft != null && nextUp.label && (
        <div className="next-up" role="status">
          <div className="next-up-label">
            <span className="muted">Next episode in {nextUp.secondsLeft}</span>
            <strong>{nextUp.label}</strong>
          </div>
          <div className="next-up-actions">
            <button className="primary" onClick={nextUp.playNext}>
              ▶ Play now
            </button>
            <button onClick={nextUp.cancel}>Cancel</button>
          </div>
        </div>
      )}
      {resumeNotice != null && !toast && (
        <div className="toast resume-toast">
          Resumed at {formatTime(resumeNotice)}
          <button
            onClick={() => {
              seekTo(0);
              setResumeNotice(null);
            }}
          >
            Start over
          </button>
        </div>
      )}

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
              seekTo(Number(e.currentTarget.value));
              setDragging(null);
            }}
            onKeyUp={(e) => {
              if (dragging != null) {
                seekTo(Number(e.currentTarget.value));
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
            onClick={() => skip(-SKIP_SECONDS)}
            aria-label={`Back ${SKIP_SECONDS} seconds`}
          >
            ↺{SKIP_SECONDS}
          </button>
          <button
            className="icon"
            onClick={() => skip(SKIP_SECONDS)}
            aria-label={`Forward ${SKIP_SECONDS} seconds`}
          >
            {SKIP_SECONDS}↻
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

          <Menu
            name="audio"
            label={`Audio: ${currentAudio ? shortLabel(currentAudio.label) : "–"}`}
            open={menu === "audio"}
            onToggle={toggleMenu}
            sections={[
              {
                empty: "No audio tracks yet",
                options: audioTracks.map((t) => ({
                  key: String(t.id),
                  label: t.label,
                  selected: t.selected,
                  onSelect: () => choose({ action: "setAudioTrack", id: t.id }),
                })),
              },
            ]}
          />
          <Menu
            name="subtitles"
            label={`Subtitles: ${currentSub ? shortLabel(currentSub) : "Off"}`}
            open={menu === "subtitles"}
            onToggle={toggleMenu}
            sections={[
              {
                options: [
                  {
                    key: "off",
                    label: "Off",
                    selected: subtitlesOff,
                    onSelect: () => choose({ action: "setSubtitle", track: null, key: null }),
                  },
                ],
              },
              {
                heading: "Adjust",
                options: [],
                content: (
                  <div className="adjust">
                    <div className="adjust-row">
                      <span>Delay</span>
                      <button
                        onClick={() =>
                          send({ action: "adjustSubtitleDelay", seconds: -SUB_DELAY_STEP })
                        }
                        aria-label="Show subtitles earlier"
                      >
                        −
                      </button>
                      <span className="adjust-value">
                        {subDelay > 0 ? "+" : ""}
                        {subDelay.toFixed(1)} s
                      </span>
                      <button
                        onClick={() =>
                          send({ action: "adjustSubtitleDelay", seconds: SUB_DELAY_STEP })
                        }
                        aria-label="Show subtitles later"
                      >
                        +
                      </button>
                      <button
                        className="reset"
                        onClick={() => send({ action: "resetSubtitleDelay" })}
                        disabled={subDelay === 0}
                      >
                        Reset
                      </button>
                    </div>
                    <div className="adjust-row">
                      <span>Size</span>
                      <button
                        onClick={() => setScale(subScale - SUB_SCALE_STEP)}
                        aria-label="Smaller subtitles"
                      >
                        −
                      </button>
                      <span className="adjust-value">{Math.round(subScale * 100)}%</span>
                      <button
                        onClick={() => setScale(subScale + SUB_SCALE_STEP)}
                        aria-label="Larger subtitles"
                      >
                        +
                      </button>
                      <button
                        className="reset"
                        onClick={() => setScale(1)}
                        disabled={Math.abs(subScale - 1) < 0.01}
                      >
                        Reset
                      </button>
                    </div>
                  </div>
                ),
              },
              {
                heading: "In the video file",
                empty: "None",
                options: embeddedSubs.map((t) => ({
                  key: `t${t.id}`,
                  label: t.label,
                  selected: t.selected,
                  onSelect: () => choose({ action: "setSubtitle", track: t.id, key: null }),
                })),
              },
              {
                heading: "From your addons",
                empty: "None found (yet)",
                options: addonSubs.map((s) => ({
                  key: s.key,
                  label: s.label,
                  detail: s.addonName,
                  selected: s.selected,
                  onSelect: () => choose({ action: "setSubtitle", track: null, key: s.key }),
                })),
              },
              {
                heading: "From this computer",
                options: [
                  {
                    key: "file",
                    label: "Load from file…",
                    selected: false,
                    onSelect: () => {
                      setMenu(null);
                      loadSubtitleFile().catch((e) => setToast(String(e)));
                    },
                  },
                ],
              },

            ]}
          />
          <Menu
            name="source"
            label="Source"
            open={menu === "source"}
            onToggle={toggleMenu}
            sections={[
              {
                heading: "Streams, best first",
                options: sources.map((label, i) => ({
                  key: String(i),
                  label: `${i + 1}. ${label}`,
                  selected: i === state?.sourceIndex,
                  onSelect: () => choose({ action: "setSource", index: i }),
                })),
              },
            ]}
          />
          <Menu
            name="speed"
            label={speed === 1 ? "Speed" : `${speed}×`}
            open={menu === "speed"}
            onToggle={toggleMenu}
            sections={[
              {
                options: SPEEDS.map((s) => ({
                  key: String(s),
                  label: s === 1 ? "Normal" : `${s}×`,
                  selected: Math.abs(s - speed) < 0.01,
                  onSelect: () => choose({ action: "setSpeed", speed: s }),
                })),
              },
            ]}
          />
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

/** The part of a track label before any details in brackets, to keep the bar short. */
function shortLabel(label: string) {
  return label.replace(/\s*\(.*\)$/, "");
}
