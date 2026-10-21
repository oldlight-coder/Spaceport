import { useEffect, useState } from "react";
import { logout, Session, User } from "../api/auth";
import { getSettings, saveSettings, Settings } from "../api/settings";
import Avatar from "../components/Avatar";
import SearchBox from "../components/SearchBox";
import { OpenAddonsContext, OpenTitleContext, TitleRef } from "../navigation";
import { useOnline } from "../online";
import { useForwardPlayerKeys } from "../playerKeys";
import { applyTheme, cacheAppearance } from "../theme";
import { NewEpisodesContext, useNewEpisodesLoader } from "../newEpisodes";
import BoardScreen from "./BoardScreen";
import DetailsScreen from "./DetailsScreen";
import DiscoverScreen from "./DiscoverScreen";
import LibraryScreen from "./LibraryScreen";
import SearchScreen from "./SearchScreen";
import SettingsScreen from "./SettingsScreen";
import { ListSkeleton } from "../components/Skeleton";

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
} as const;
type Page = keyof typeof PAGES | "search" | "settings";

export default function HomeScreen({ user, onSession }: Props) {
  const [busy, setBusy] = useState(false);
  const [page, setPage] = useState<Page>("board");
  const [title, setTitle] = useState<TitleRef | null>(null);
  const [query, setQuery] = useState("");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const online = useOnline();
  useForwardPlayerKeys();
  const newEpisodes = useNewEpisodesLoader();

  // The look follows the saved settings and is remembered for the next start's first paint.
  const appearance = settings?.appearance;
  useEffect(() => {
    if (!appearance) return;
    cacheAppearance(appearance);
    return applyTheme(appearance);
  }, [appearance?.mode, appearance?.accent]);

  useEffect(() => {
    getSettings()
      .then(setSettings)
      .catch((e) => setSettingsError(String(e)));
  }, []);

  async function updateSettings(next: Settings) {
    setSettings(await saveSettings(next));
  }

  function go(next: Page) {
    setPage(next);
    setTitle(null);
    // The player saves some choices (subtitle size) on its own; show what is stored now.
    if (next === "settings") {
      getSettings()
        .then(setSettings)
        .catch(() => {});
    }
  }

  function openAddons() {
    go("settings");
    // The section exists once the settings page has rendered.
    window.setTimeout(() => document.getElementById("settings-addons")?.scrollIntoView(), 0);
  }

  function search(term: string) {
    setQuery(term);
    go("search");
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
      <OpenAddonsContext.Provider value={openAddons}>
        <NewEpisodesContext.Provider value={newEpisodes}>
          <div className="app">
            <header className="topbar">
              <span className="brand">Spaceport</span>
              <nav className="tabs">
                {(Object.keys(PAGES) as (keyof typeof PAGES)[]).map((key) => (
                  <button
                    key={key}
                    className={key === page ? "tab active" : "tab"}
                    onClick={() => go(key)}
                  >
                    {PAGES[key]}
                  </button>
                ))}
              </nav>
              <SearchBox onSearch={search} />
              <button
                className={page === "settings" ? "settings-button active" : "settings-button"}
                onClick={() => go("settings")}
                aria-label="Settings"
                title="Settings"
              >
                <Avatar email={user.email} avatar={settings?.avatar ?? null} size={28} />
                <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
                  <path
                    fill="currentColor"
                    d="M19.4 13a7.5 7.5 0 0 0 0-2l2.1-1.6a.5.5 0 0 0 .1-.6l-2-3.5a.5.5 0 0 0-.6-.2l-2.5 1a7.3 7.3 0 0 0-1.7-1l-.4-2.6a.5.5 0 0 0-.5-.4h-4a.5.5 0 0 0-.5.4l-.4 2.6a7.3 7.3 0 0 0-1.7 1l-2.5-1a.5.5 0 0 0-.6.2l-2 3.5a.5.5 0 0 0 .1.6L4.6 11a7.5 7.5 0 0 0 0 2l-2.1 1.6a.5.5 0 0 0-.1.6l2 3.5c.1.2.4.3.6.2l2.5-1c.5.4 1.1.7 1.7 1l.4 2.6c0 .3.2.4.5.4h4c.3 0 .5-.2.5-.4l.4-2.6c.6-.3 1.2-.6 1.7-1l2.5 1c.2.1.5 0 .6-.2l2-3.5a.5.5 0 0 0-.1-.6L19.4 13ZM12 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7Z"
                  />
                </svg>
              </button>
            </header>
            {!online && (
              <div className="offline-banner" role="status">
                You are offline. Pages load again by themselves when the connection is back; cached
                torrents keep playing.
              </div>
            )}
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
              {page === "search" && <SearchScreen query={query} />}
              {page === "settings" &&
                (settings ? (
                  <SettingsScreen
                    email={user.email}
                    settings={settings}
                    onChange={updateSettings}
                    onSignOut={signOut}
                    signingOut={busy}
                  />
                ) : (
                  settingsError ? <p className="error">{settingsError}</p> : <ListSkeleton rows={4} />
                ))}
            </main>
          </div>
        </NewEpisodesContext.Provider>
      </OpenAddonsContext.Provider>
    </OpenTitleContext.Provider>
  );
}
