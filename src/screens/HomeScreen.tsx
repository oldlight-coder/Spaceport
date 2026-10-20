import { useState } from "react";
import { logout, Session, User } from "../api/auth";
import AddonsScreen from "./AddonsScreen";
import BoardScreen from "./BoardScreen";
import DiscoverScreen from "./DiscoverScreen";
import LibraryScreen from "./LibraryScreen";

interface Props {
  user: User;
  onSession: (session: Session) => void;
}

const PAGES = {
  board: "Home",
  movie: "Movies",
  series: "Series",
  anime: "Anime",
  library: "Library",
  addons: "Addons",
} as const;
type Page = keyof typeof PAGES;

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState<Page>("board");

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
      <main className="content">
        {page === "board" && <BoardScreen />}
        {(page === "movie" || page === "series" || page === "anime") && (
          <DiscoverScreen key={page} type={page} title={PAGES[page]} />
        )}
        {page === "library" && <LibraryScreen />}
        {page === "addons" && <AddonsScreen />}
      </main>
    </div>
  );
}
