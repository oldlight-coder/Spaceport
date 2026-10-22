import { useState } from "react";
import { CatalogRef, typeLabel } from "../api/catalog";
import { arrange, catalogKey, useCatalogLayout } from "../catalogLayout";

interface Props {
  /** Which page's arrangement this edits: "home", or a type page's type. */
  page: string;
  heading: string;
  /** Every catalog the page could show, hidden ones included. */
  catalogs: CatalogRef[];
  onClose: () => void;
}

/** Reorder and hide catalogs; changes are saved as they are made. */
export default function CatalogEditor({ page, heading, catalogs, onClose }: Props) {
  const keyOf = (c: CatalogRef) => catalogKey(page, c);
  const { layout, save } = useCatalogLayout();
  const [error, setError] = useState<string | null>(null);
  const list = arrange(page, catalogs, layout, true);
  const hidden = new Set(layout?.hidden ?? []);

  function store(order: CatalogRef[], nextHidden: Set<string>) {
    // Keys of catalogs on other pages stay as they were.
    const here = new Set(catalogs.map(keyOf));
    const others = (layout?.order ?? []).filter((k) => !here.has(k));
    const otherHidden = (layout?.hidden ?? []).filter((k) => !here.has(k));
    setError(null);
    save({
      order: [...order.map(keyOf), ...others],
      hidden: [...otherHidden, ...[...nextHidden].filter((k) => here.has(k))],
    }).catch((e) => setError(String(e)));
  }

  function move(index: number, step: number) {
    const next = [...list];
    const [item] = next.splice(index, 1);
    next.splice(index + step, 0, item);
    store(next, hidden);
  }

  function toggle(c: CatalogRef) {
    const next = new Set(hidden);
    const key = keyOf(c);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    store(list, next);
  }

  return (
    <aside className="streams-panel catalog-editor" aria-label={heading}>
      <header className="streams-head">
        <div>
          <h2>{heading}</h2>
          <span className="muted small">Order and visibility are saved on this computer.</span>
        </div>
        <button className="secondary" onClick={onClose} aria-label="Close">
          ✕
        </button>
      </header>
      {error && <p className="error">{error}</p>}
      <button
        className="text-button reset-layout"
        onClick={() => store(catalogs, new Set())}
      >
        Reset to the addons' order
      </button>
      <ol className="catalog-list">
        {list.map((c, i) => {
          const off = hidden.has(keyOf(c));
          return (
            <li key={keyOf(c)} className={off ? "off" : undefined}>
              <div className="catalog-name">
                <strong>{c.name}</strong>
                <span className="muted small">
                  {typeLabel(c.type)} · {c.addonName}
                </span>
              </div>
              <button
                className="secondary icon"
                onClick={() => move(i, -1)}
                disabled={i === 0}
                aria-label={`Move ${c.name} up`}
              >
                ↑
              </button>
              <button
                className="secondary icon"
                onClick={() => move(i, 1)}
                disabled={i === list.length - 1}
                aria-label={`Move ${c.name} down`}
              >
                ↓
              </button>
              <button
                className={off ? "secondary visibility" : "secondary visibility on"}
                onClick={() => toggle(c)}
                aria-pressed={!off}
              >
                {off ? "Hidden" : "Shown"}
              </button>
            </li>
          );
        })}
      </ol>
    </aside>
  );
}
