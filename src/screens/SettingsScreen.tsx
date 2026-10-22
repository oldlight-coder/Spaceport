import { ChangeEvent, useEffect, useRef, useState } from "react";
import { ACCENT_PRESETS, Appearance, ThemeMode } from "../theme";
import { isAndroid } from "../platform";
import { CODECS, Settings, SubtitleStyle } from "../api/settings";
import Avatar from "../components/Avatar";
import LanguageList from "../components/LanguageList";
import AddonsScreen from "./AddonsScreen";

const GIB = 1024 ** 3;

const SUBTITLE_COLOURS = [
  { name: "White", value: "#FFFFFF" },
  { name: "Yellow", value: "#FFFF00" },
  { name: "Cyan", value: "#00FFFF" },
];
const AVATAR_SIZE = 128;

const THEME_MODES: { value: ThemeMode; label: string }[] = [
  { value: "system", label: isAndroid ? "Follow system" : "Follow Windows" },
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
];

interface Props {
  email: string;
  settings: Settings;
  onChange: (settings: Settings) => Promise<void>;
  onSignOut: () => Promise<void>;
  signingOut: boolean;
}

/** Shrinks a picked image to a small square JPEG data URL, cropped to the centre. */
function shrinkImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const side = Math.min(img.width, img.height);
      const canvas = document.createElement("canvas");
      canvas.width = AVATAR_SIZE;
      canvas.height = AVATAR_SIZE;
      canvas
        .getContext("2d")!
        .drawImage(
          img,
          (img.width - side) / 2,
          (img.height - side) / 2,
          side,
          side,
          0,
          0,
          AVATAR_SIZE,
          AVATAR_SIZE,
        );
      URL.revokeObjectURL(url);
      resolve(canvas.toDataURL("image/jpeg", 0.85));
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("That file is not an image Spaceport can read."));
    };
    img.src = url;
  });
}

export default function SettingsScreen({ email, settings, onChange, onSignOut, signingOut }: Props) {
  const [error, setError] = useState<string | null>(null);
  // Changes merge into the newest settings, not the copy a delayed save captured.
  const latest = useRef(settings);
  latest.current = settings;

  async function update(patch: Partial<Settings>) {
    setError(null);
    try {
      await onChange({ ...latest.current, ...patch });
    } catch (e) {
      setError(String(e));
    }
  }

  const updateFilters = (patch: Partial<Settings["filters"]>) =>
    update({ filters: { ...latest.current.filters, ...patch } });

  const style = settings.subtitleStyle;
  const updateStyle = (patch: Partial<SubtitleStyle>) =>
    update({ subtitleStyle: { ...latest.current.subtitleStyle, ...patch } });

  async function pickAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    try {
      await update({ avatar: await shrinkImage(file) });
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    }
  }


  return (
    <div className="settings">
      <h1>Settings</h1>
      {error && <p className="error">{error}</p>}

      <section className="settings-section">
        <h2>Appearance</h2>
        <AppearanceSettings
          value={settings.appearance}
          onChange={(patch) =>
            update({ appearance: { ...latest.current.appearance, ...patch } })
          }
        />
      </section>

      <section className="settings-section">
        <h2>Account</h2>
        <div className="account">
          <Avatar email={email} avatar={settings.avatar} size={72} />
          <div className="account-info">
            <strong>{email}</strong>
            <span className="muted">Signed in to Stremio</span>
            <div className="row-actions">
              <label className="button secondary">
                Choose picture
                <input type="file" accept="image/*" onChange={pickAvatar} hidden />
              </label>
              {settings.avatar && (
                <button className="secondary" onClick={() => update({ avatar: null })}>
                  Remove picture
                </button>
              )}
              <button
                className="secondary"
                onClick={() => onSignOut().catch((e) => setError(`Could not sign out: ${e}`))}
                disabled={signingOut}
              >
                Sign out
              </button>
            </div>
          </div>
        </div>
      </section>

      <section className="settings-section">
        <h2>Streams</h2>
        <p className="muted small">
          Preferences change the ranking. Filters hide streams; ones without the information are
          kept.
        </p>
        <div className="settings-grid">
          <LanguageList
            label="Audio languages"
            value={settings.audioLanguages}
            onChange={(audioLanguages) => update({ audioLanguages })}
          />
          <LanguageList
            label="Subtitle languages"
            value={settings.subtitleLanguages}
            onChange={(subtitleLanguages) => update({ subtitleLanguages })}
          />
          <label className="setting">
            <span className="setting-label">Preferred resolution</span>
            <select
              value={settings.preferredResolution}
              onChange={(e) => update({ preferredResolution: Number(e.target.value) })}
            >
              <option value={2160}>4K</option>
              <option value={1440}>1440p</option>
              <option value={1080}>1080p</option>
              <option value={720}>720p</option>
            </select>
          </label>
          <label className="setting">
            <span className="setting-label">Minimum quality</span>
            <select
              value={settings.filters.minResolution ?? ""}
              onChange={(e) =>
                updateFilters({ minResolution: e.target.value ? Number(e.target.value) : null })
              }
            >
              <option value="">Any</option>
              <option value={480}>480p</option>
              <option value={720}>720p</option>
              <option value={1080}>1080p</option>
              <option value={2160}>4K</option>
            </select>
          </label>
          <label className="setting">
            <span className="setting-label">Maximum size (GB)</span>
            <SizeField
              bytes={settings.filters.maxSize}
              onChange={(maxSize) => updateFilters({ maxSize })}
            />
          </label>
          <fieldset className="setting">
            <legend className="setting-label">Blocked codecs</legend>
            <div className="checks">
              {CODECS.map((codec) => (
                <label key={codec}>
                  <input
                    type="checkbox"
                    checked={settings.filters.blockedCodecs.includes(codec)}
                    onChange={(e) =>
                      updateFilters({
                        blockedCodecs: e.target.checked
                          ? [...settings.filters.blockedCodecs, codec]
                          : settings.filters.blockedCodecs.filter((c) => c !== codec),
                      })
                    }
                  />{" "}
                  {codec}
                </label>
              ))}
            </div>
          </fieldset>
        </div>
      </section>

      <section className="settings-section">
        <h2>Subtitles</h2>
        <p className="muted small">
          Applies to every video, including one that is playing. Styled subtitles (common for
          anime) keep their own colours and outline.
        </p>
        <div className="settings-grid">
          <label className="setting">
            <span className="setting-label">Size: {Math.round(style.scale * 100)}%</span>
            <input
              type="range"
              min={50}
              max={200}
              step={10}
              value={Math.round(style.scale * 100)}
              onChange={(e) => updateStyle({ scale: Number(e.target.value) / 100 })}
            />
          </label>
          <fieldset className="setting">
            <legend className="setting-label">Colour</legend>
            <div className="checks">
              {SUBTITLE_COLOURS.map(({ name, value }) => (
                <label key={value}>
                  <input
                    type="radio"
                    name="subtitle-colour"
                    checked={style.color.toUpperCase() === value}
                    onChange={() => updateStyle({ color: value })}
                  />{" "}
                  <span className="swatch" style={{ background: value }} /> {name}
                </label>
              ))}
            </div>
          </fieldset>
          <fieldset className="setting">
            <legend className="setting-label">Background</legend>
            <div className="checks">
              <label>
                <input
                  type="radio"
                  name="subtitle-background"
                  checked={style.background === "outline"}
                  onChange={() => updateStyle({ background: "outline" })}
                />{" "}
                Outline
              </label>
              <label>
                <input
                  type="radio"
                  name="subtitle-background"
                  checked={style.background === "box"}
                  onChange={() => updateStyle({ background: "box" })}
                />{" "}
                Dark box
              </label>
            </div>
          </fieldset>
          <label className="setting">
            <span className="setting-label">
              Position: {style.position === 100 ? "bottom" : `${100 - style.position}% higher`}
            </span>
            <input
              type="range"
              min={50}
              max={100}
              value={style.position}
              onChange={(e) => updateStyle({ position: Number(e.target.value) })}
            />
          </label>
        </div>
      </section>

      <section className="settings-section">
        <h2>Playback</h2>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.autoPlayNext}
            onChange={(e) => update({ autoPlayNext: e.target.checked })}
          />
          <span>
            <strong>Play the next episode automatically</strong>
            <span className="muted small">
              Near the end of an episode a countdown starts the next one. When off, the next
              episode is offered with a Play button instead.
            </span>
          </span>
        </label>
      </section>

      <section className="settings-section">
        <h2>Anime</h2>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.animeAutoSkip}
            onChange={(e) => update({ animeAutoSkip: e.target.checked })}
          />
          <span>
            <strong>Skip intros, outros and recaps automatically</strong>
            <span className="muted small">
              Uses community timings from AniSkip. When off, a Skip button appears instead.
            </span>
          </span>
        </label>
      </section>

      <section className="settings-section">
        <h2>Torrents</h2>
        <label className="toggle">
          <input
            type="checkbox"
            checked={settings.torrentDht}
            onChange={(e) => update({ torrentDht: e.target.checked })}
          />
          <span>
            <strong>Find peers with DHT</strong>
            <span className="muted small">
              Helps torrents without trackers start. DHT keeps one network port open; turn it off
              to use trackers only. Takes effect after restarting Spaceport.
            </span>
          </span>
        </label>
        {isAndroid && (
          <label className="toggle">
            <input
              type="checkbox"
              checked={settings.torrentWifiOnly}
              onChange={(e) => update({ torrentWifiOnly: e.target.checked })}
            />
            <span>
              <strong>Only on Wi-Fi</strong>
              <span className="muted small">
                Torrents do not start on mobile data. Debrid and direct links still play.
              </span>
            </span>
          </label>
        )}
        <label className="setting cache-setting">
          <span className="setting-label">Torrent cache size (GB)</span>
          <input
            type="number"
            min={1}
            max={2000}
            value={settings.torrentCacheGb}
            onChange={(e) => {
              const gb = Math.round(Number(e.target.value));
              if (gb >= 1 && gb <= 2000) update({ torrentCacheGb: gb });
            }}
          />
          <span className="muted small">
            When downloads pass this size, torrents you are not watching are deleted, least
            recently watched first. Everything is deleted when Spaceport closes.
          </span>
        </label>
      </section>

      <section className="settings-section" id="settings-addons">
        <AddonsScreen />
      </section>
    </div>
  );
}

/** Gigabytes with one decimal; empty, zero or nonsense means no limit. */
function SizeField({ bytes, onChange }: { bytes: number | null; onChange: (b: number | null) => void }) {
  const shown = (b: number | null) => (b ? String(Math.round((b / GIB) * 10) / 10) : "");
  // The text as typed ("1." mid-typing), reset when the saved value changes elsewhere.
  const [text, setText] = useState(() => shown(bytes));
  useEffect(() => setText((current) => (Number(current) * GIB === bytes ? current : shown(bytes))), [bytes]);
  return (
    <input
      type="number"
      min={0.1}
      step={0.1}
      placeholder="No limit"
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        const gb = Number(e.target.value);
        onChange(e.target.value && gb > 0 && Number.isFinite(gb) ? Math.round(gb * GIB) : null);
      }}
    />
  );
}

interface AppearanceProps {
  value: Appearance;
  onChange: (patch: Partial<Appearance>) => void;
}

/** Light, dark or Windows' mode, and the accent colour the Material You palette comes from. */
function AppearanceSettings({ value, onChange }: AppearanceProps) {
  // The colour picker reports every step of a drag; the choice is saved once it rests.
  const [accent, setAccent] = useState(value.accent);
  const timer = useRef<number>(undefined);
  useEffect(() => setAccent(value.accent), [value.accent]);
  useEffect(() => () => window.clearTimeout(timer.current), []);

  function pickAccent(next: string, wait = 0) {
    setAccent(next);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => onChange({ accent: next }), wait);
  }

  const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
  return (
    <div className="settings-grid">
      <fieldset className="setting">
        <legend className="setting-label">Theme</legend>
        <div className="segmented" role="radiogroup" aria-label="Theme">
          {THEME_MODES.map((mode) => (
            <button
              key={mode.value}
              role="radio"
              aria-checked={value.mode === mode.value}
              className={value.mode === mode.value ? "active" : undefined}
              onClick={() => onChange({ mode: mode.value })}
            >
              {mode.label}
            </button>
          ))}
        </div>
      </fieldset>
      <fieldset className="setting">
        <legend className="setting-label">Accent colour</legend>
        <div className="accent-swatches">
          {ACCENT_PRESETS.map((color) => (
            <button
              key={color}
              className={same(color, accent) ? "accent-swatch active" : "accent-swatch"}
              style={{ background: color }}
              onClick={() => pickAccent(color)}
              aria-label={`Accent ${color}`}
              title={color}
            />
          ))}
          <label className="accent-custom" title="Choose any colour">
            <input
              type="color"
              value={accent}
              onChange={(e) => pickAccent(e.target.value.toUpperCase(), 300)}
              aria-label="Custom accent colour"
            />
            Custom
          </label>
        </div>
        <span className="muted small">
          Spaceport builds its light and dark colours from this, the Material You way.
        </span>
      </fieldset>
    </div>
  );
}
