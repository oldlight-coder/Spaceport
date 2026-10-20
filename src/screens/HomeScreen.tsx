import { useState } from "react";
import { logout, Session, User } from "../api/auth";
import AddonsScreen from "./AddonsScreen";
import LibraryScreen from "./LibraryScreen";

interface Props {
  user: User;
  onSession: (session: Session) => void;
}

const PAGES = { library: "Library", addons: "Addons" } as const;
type Page = keyof typeof PAGES;

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState<Page>("library");

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
        <nav className="tabs">
          {(Object.keys(PAGES) as Page[]).map((key) => (
            <button
              key={key}
              className={key === page ? "tab active" : "tab"}
              onClick={() => setPage(key)}
            >
              {PAGES[key]}
            </button>
          ))}
        </nav>
        <span className="muted spacer">{user.email}</span>
        <button className="secondary" onClick={signOut} disabled={busy}>
          Sign out
        </button>
      </header>
      <main className="content">{page === "library" ? <LibraryScreen /> : <AddonsScreen />}</main>
    </div>
  );
}
