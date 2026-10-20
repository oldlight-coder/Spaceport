import { FormEvent, useState } from "react";
import { logout, Session, User } from "../api/auth";
import { OpenTitleContext, TitleRef } from "../navigation";
import AddonsScreen from "./AddonsScreen";
import BoardScreen from "./BoardScreen";
import DetailsScreen from "./DetailsScreen";
import DiscoverScreen from "./DiscoverScreen";
import LibraryScreen from "./LibraryScreen";
import SearchScreen from "./SearchScreen";

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
type Page = keyof typeof PAGES | "search";

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState<Page>("board");
  const [title, setTitle] = useState<TitleRef | null>(null);
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");

  function search(event: FormEvent) {
    event.preventDefault();
    const term = input.trim();
    if (!term) return;
    setQuery(term);
    setPage("search");
    setTitle(null);
  }

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
            {(Object.keys(PAGES) as (keyof typeof PAGES)[]).map((key) => (
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
          <form className="search-form spacer" onSubmit={search} role="search">
            <input
              type="search"
              placeholder="Search"
              value={input}
              onChange={(e) => setInput(e.target.value)}
              aria-label="Search all addons"
            />
          </form>
          <span className="muted user-email">{user.email}</span>
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
          {page === "search" && <SearchScreen query={query} />}
        </main>
      </div>
    </OpenTitleContext.Provider>
  );
}
