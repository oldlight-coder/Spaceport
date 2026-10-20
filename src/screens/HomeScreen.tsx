import { ReactNode, useCallback, useEffect, useState } from "react";
import { logout, Session, User } from "../api/auth";
import { Addon, LibraryItem, listAddons, listLibrary } from "../api/library";

interface Props {
  user: User;
  onSession: (session: Session) => void;
}

type Load<T> = { status: "loading" } | { status: "error"; message: string } | { status: "ready"; data: T };

function useLoad<T>(load: () => Promise<T>) {
  const [state, setState] = useState<Load<T>>({ status: "loading" });
  const reload = useCallback(() => {
    setState({ status: "loading" });
    load()
      .then((data) => setState({ status: "ready", data }))
      .catch((e) => setState({ status: "error", message: String(e) }));
  }, [load]);
  useEffect(reload, [reload]);
  return [state, reload] as const;
}

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);
  const [library, reloadLibrary] = useLoad(listLibrary);
  const [addons, reloadAddons] = useLoad(listAddons);

  async function signOut() {
    setBusy(true);
    try {
      onSession(await logout());
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">Spaceport</span>
        <span className="muted">{user.email}</span>
        <button className="secondary" onClick={signOut} disabled={busy}>
          Sign out
        </button>
      </header>
      <main className="content">
        <section>
          <h2>Library</h2>
          <Section state={library} onRetry={reloadLibrary} empty="Your library is empty.">
            {(items) => <LibraryGrid items={items} />}
          </Section>
        </section>
        <section>
          <h2>Installed addons</h2>
          <Section state={addons} onRetry={reloadAddons} empty="No addons installed.">
            {(list) => <AddonList addons={list} />}
          </Section>
        </section>
      </main>
    </div>
  );
}

interface SectionProps<T> {
  state: Load<T[]>;
  onRetry: () => void;
  empty: string;
  children: (data: T[]) => ReactNode;
}

function Section<T>({ state, onRetry, empty, children }: SectionProps<T>) {
  switch (state.status) {
    case "loading":
      return <p className="muted">Loading…</p>;
    case "error":
      return (
        <div className="inline-error">
          <p className="error">{state.message}</p>
          <button className="secondary" onClick={onRetry}>
            Retry
          </button>
        </div>
      );
    case "ready":
      return state.data.length === 0 ? <p className="muted">{empty}</p> : <>{children(state.data)}</>;
  }
}

function LibraryGrid({ items }: { items: LibraryItem[] }) {
  return (
    <ul className="poster-grid">
      {items.map((item) => (
        <li key={item.id} className="poster-card" title={item.name}>
          {item.poster ? (
            <img src={item.poster} alt="" loading="lazy" referrerPolicy="no-referrer" />
          ) : (
            <div className="poster-placeholder">{item.name.slice(0, 1)}</div>
          )}
          <span className="poster-title">{item.name || item.id}</span>
        </li>
      ))}
    </ul>
  );
}

function AddonList({ addons }: { addons: Addon[] }) {
  return (
    <ul className="addon-list">
      {addons.map((addon) => (
        <li key={addon.transportUrl}>
          <div>
            <strong>{addon.name}</strong> <span className="muted">v{addon.version}</span>
            {addon.official && <span className="badge">Official</span>}
          </div>
          {addon.description && <p className="muted">{addon.description}</p>}
        </li>
      ))}
    </ul>
  );
}
