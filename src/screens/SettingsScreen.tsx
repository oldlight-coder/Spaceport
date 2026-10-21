import { ChangeEvent, useState } from "react";
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

interface Props {
  email: string;
  settings: Settings;
  onChange: (settings: Settings) => Promise<void>;
  onSignOut: () => void;
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

  async function update(patch: Partial<Settings>) {
    setError(null);
    try {
      await onChange({ ...settings, ...patch });
    } catch (e) {
      setError(String(e));
    }
  }

  const updateFilters = (patch: Partial<Settings["filters"]>) =>
    update({ filters: { ...settings.filters, ...patch } });

  const style = settings.subtitleStyle;
  const updateStyle = (patch: Partial<SubtitleStyle>) =>
    update({ subtitleStyle: { ...style, ...patch } });

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

  const maxSizeGb = settings.filters.maxSize ? Math.round(settings.filters.maxSize / GIB) : "";

  return (
    <div className="settings">
      <h1>Settings</h1>
      {error && <p className="error">{error}</p>}

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
              <button className="secondary" onClick={onSignOut} disabled={signingOut}>
                Sign out
              </button>
            </div>
            <span className="muted small">The picture stays on this computer.</span>
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
            <input
              type="number"
              min={1}
              placeholder="No limit"
              value={maxSizeGb}
              onChange={(e) =>
                updateFilters({ maxSize: e.target.value ? Number(e.target.value) * GIB : null })
              }
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

      <section className="settings-section">
        <AddonsScreen />
      </section>
    </div>
  );
}
