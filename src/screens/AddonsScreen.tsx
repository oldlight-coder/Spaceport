import { FormEvent, useState } from "react";
import {
  Addon,
  configureAddon,
  installAddon,
  listAddons,
  removeAddon,
  reorderAddons,
} from "../api/addons";
import { LoadableList, useLoad } from "../components/Loadable";
import { ListSkeleton } from "../components/Skeleton";

export default function AddonsScreen() {
  const [addons, reload, setAddons] = useLoad(listAddons);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** Runs one change against Stremio and shows the collection it returns. */
  async function run(change: () => Promise<Addon[]>) {
    setBusy(true);
    setError(null);
    try {
      setAddons({ status: "ready", data: await change() });
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function install(event: FormEvent) {
    event.preventDefault();
    if (await run(() => installAddon(url))) setUrl("");
  }

  function move(list: Addon[], index: number, delta: number) {
    const order = list.map((a) => a.transportUrl);
    const [moved] = order.splice(index, 1);
    order.splice(index + delta, 0, moved);
    run(() => reorderAddons(order));
  }

  function remove(addon: Addon) {
    if (window.confirm(`Remove ${addon.name}?`)) run(() => removeAddon(addon.transportUrl));
  }

  async function configure(addon: Addon) {
    setError(null);
    try {
      await configureAddon(addon.transportUrl);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <section>
      <h2>Addons</h2>
      <form className="install-form" onSubmit={install}>
        <input
          type="url"
          placeholder="https://…/manifest.json"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          required
          aria-label="Addon manifest URL"
        />
        <button type="submit" disabled={busy}>
          Install
        </button>
      </form>
      {error && <p className="error">{error}</p>}
      <LoadableList
        state={addons}
        onRetry={reload}
        empty="No addons installed."
        skeleton={<ListSkeleton rows={4} />}
      >
        {(list) => (
          <ul className="addon-list">
            {list.map((addon, index) => (
              <li key={addon.transportUrl} className="addon-row">
                <div className="addon-info">
                  <div>
                    <strong>{addon.name}</strong> <span className="muted">v{addon.version}</span>
                    {addon.official && <span className="badge">Official</span>}
                  </div>
                  {addon.description && <p className="muted">{addon.description}</p>}
                </div>
                <div className="addon-actions">
                  <button
                    className="secondary icon"
                    onClick={() => move(list, index, -1)}
                    disabled={busy || index === 0}
                    aria-label={`Move ${addon.name} up`}
                  >
                    ↑
                  </button>
                  <button
                    className="secondary icon"
                    onClick={() => move(list, index, 1)}
                    disabled={busy || index === list.length - 1}
                    aria-label={`Move ${addon.name} down`}
                  >
                    ↓
                  </button>
                  {addon.configurable && (
                    <button className="secondary" onClick={() => configure(addon)}>
                      Configure
                    </button>
                  )}
                  <button
                    className="secondary"
                    onClick={() => remove(addon)}
                    disabled={busy || addon.protected}
                    title={addon.protected ? "This addon cannot be removed" : undefined}
                  >
                    Remove
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </LoadableList>
    </section>
  );
}
