import { useState } from "react";
import { logout, Session, User } from "../api/auth";
import { OpenTitleContext, TitleRef } from "../navigation";
import AddonsScreen from "./AddonsScreen";
import BoardScreen from "./BoardScreen";
import DetailsScreen from "./DetailsScreen";
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
  const [title, setTitle] = useState<TitleRef | null>(null);

  function openTitle(next: TitleRef) {
    setTitle(next);
    window.scrollTo(0, 0);
  }

  async function signOut() {
    setBusy(true);
    try {
      onSession(await logout());
    } finally {
      setBusy(false);
    }
  }

  return (
    <OpenTitleContext.Provider value={openTitle}>
      <div className="app">
        <header className="topbar">
          <span className="brand">Spaceport</span>
          <nav className="tabs">
            {(Object.keys(PAGES) as Page[]).map((key) => (
              <button
                key={key}
                className={key === page ? "tab active" : "tab"}
                onClick={() => {
                  setPage(key);
                  setTitle(null);
                }}
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
        {title && (
          <DetailsScreen
            key={`${title.type} ${title.id}`}
            title={title}
            onBack={() => setTitle(null)}
          />
        )}
        {/* Pages stay mounted under the details view so loaded items and filters survive. */}
        <main className="content" hidden={title !== null}>
          {page === "board" && <BoardScreen />}
          {(page === "movie" || page === "series" || page === "anime") && (
            <DiscoverScreen key={page} type={page} title={PAGES[page]} />
          )}
          {page === "library" && <LibraryScreen />}
          {page === "addons" && <AddonsScreen />}
        </main>
      </div>
    </OpenTitleContext.Provider>
  );
}
