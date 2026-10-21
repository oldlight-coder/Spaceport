# Spaceport progress

Tick items as they are finished. A milestone is complete only when all of its acceptance checks
pass on Windows. The owner tests each milestone before the next one starts.

## M1. App shell and Stremio login
- [x] Tauri 2 + React + TypeScript project named Spaceport, builds an NSIS installer
- [x] Stremio login (email/password) and logout; auth key stored in Windows Credential Manager
- [x] Read the user's library and installed addons from Stremio
- **Acceptance:** installer runs; app opens to a login screen; after login the library titles are
  listed; restarting the app keeps you logged in.

## M2. Addons and browsing
- [x] Addon list: install by manifest URL, remove, reorder, open configure page; synced to Stremio
- [x] Home rows from addon catalogs (Cinemeta by default)
- [x] Movies / Series / Anime pages with genre filter and infinite scroll
- [x] Details page: poster, backdrop, description, cast, seasons, episodes
- [x] Search across all addon catalogs
- **Acceptance:** installing an addon by URL shows its catalogs on Home; opening a series shows
  every season and episode; search returns results from more than one addon.

## M3. Stream collection and ranking
- [x] Query all addons' `stream` resource in parallel with per-addon timeout
- [x] Parser for resolution, HDR/DV, codec, audio, size, seeders, release group, cached flag
- [x] Scoring rules (seeders, quality, size, language, cached) with unit tests
- [x] User filters: max size, min quality, blocked codecs, preferred languages
- [x] Manual picker with the ranked list
- **Acceptance:** for a popular movie the top pick is a well-seeded or cached stream within the
  filters; parser tests cover at least 30 real-world stream titles.

## M4. Player
- [x] libmpv embedded in the window, hardware decoding on
- [x] Plays direct and debrid links; auto-plays top stream, falls back to next on failure
- [ ] Controls: play/pause, seek, volume, fullscreen, audio track, speed
- **Acceptance:** a debrid or direct 4K HDR file plays smoothly; killing a stream URL makes the
  player move to the next ranked stream.

## M5. Torrent streaming
- [ ] librqbit engine; streams a magnet while downloading
- [ ] Local stream server on 127.0.0.1 only; cleans up cache on exit or by size limit
- **Acceptance:** a magnet from a torrent addon starts playing within about 15 seconds on a healthy
  torrent; seeking works; nothing listens on a non-loopback address.

## M6. Subtitles, resume and progress
- [ ] Subtitles from addons, embedded tracks and local files; delay and style controls
- [ ] Resume position per episode; Continue Watching row
- [ ] Mark watched / unwatched; progress synced back to Stremio
- [ ] Next-episode countdown
- **Acceptance:** stopping halfway and reopening resumes at the same spot; the same progress shows
  in the official Stremio app.

## M7. AniSkip
- [ ] Map title to MyAnimeList ID (Kitsu or AniList)
- [ ] Fetch skip times from api.aniskip.com
- [ ] Skip Intro / Skip Outro buttons and optional auto-skip
- **Acceptance:** a popular anime episode shows Skip Intro at the right time; auto-skip jumps past
  it when enabled.

## M8. Polish and release
- [ ] Settings page (filters, languages, subtitle style, auto-skip, cache size)
- [ ] Error states (no addons, no streams, offline)
- [ ] README with build steps; final privacy check of all network calls
- **Acceptance:** fresh Windows install from the NSIS installer works end to end: login, browse,
  pick, play, resume, skip intro.

## Decisions
(Record any spec changes agreed with the owner here.)
- 2026-10-20: Settings page brought forward from M8 at the owner's request. A cog button in the top
  bar (showing the profile picture) opens Settings with Account, Streams and Addons sections; the
  email and the Addons tab moved there. M8's settings item now adds the remaining settings
  (subtitle style, auto-skip, cache size) to this page.
- 2026-10-20: Profile picture added at the owner's request (not in SPEC). Chosen from a local
  image, shrunk to 128 px and stored only in the local settings file; never uploaded.

## Log
(One line per finished item: date, item, commit.)
- 2026-10-20: M1 project scaffold (Tauri 2 + React + TS, NSIS target). Typecheck, cargo test,
  clippy and a release build pass on Linux; NSIS installer still to be built and checked on Windows. (9e88864)
- 2026-10-20: M1 Stremio login/logout; auth key in Windows Credential Manager via keyring,
  session restored on start. Unit tests use a mock API; live api.strem.io untested (blocked here). (4e1f13e)
- 2026-10-20: M1 library (datastoreGet, removed/temp hidden) and installed addons
  (addonCollectionGet) shown after login. M1 code complete. (07adee2)
- 2026-10-20: M1 accepted by owner on Windows (installer, login, library, stays signed in).
- 2026-10-20: M2 addon management: install by URL (stremio:// accepted, manifest validated),
  remove (protected kept), reorder, configure page in browser; every change written back with
  addonCollectionSet. (5c48b1f) Owner confirmed it works on Windows.
- 2026-10-20: M2 Home rows: one row per browsable catalog (no required extras) of installed addons,
  Cinemeta used when none has catalogs; rows load in parallel with a 15 s per-addon timeout and
  only installed addons (or Cinemeta) can be contacted. (d2ce0da)
- 2026-10-20: M2 Movies/Series/Anime pages: catalog picker per type, genre filter from catalog
  extras (new and legacy forms), infinite scroll with `skip`, stops on empty or repeated page. (03e7ea2) Owner confirmed it works.
- 2026-10-20: M2 details page: meta from the first addon declaring `meta` for the type/id prefix
  (source addon first, Cinemeta fallback); cast/directors/genres from links or legacy fields;
  episodes grouped by season with specials last. (d9858e7)
- 2026-10-20: Details page remembers the last opened season per title (owner request). (47a13a2)
- 2026-10-20: M2 search: top-bar search queries every catalog that accepts `search` (current and
  legacy manifest forms) in parallel, one row per catalog, empty rows hidden. M2 code complete. (083a9b1)
- 2026-10-20: M2 accepted by owner on Windows (addon catalogs on Home, all seasons/episodes, search
  results from more than one addon).
- 2026-10-20: M3 stream collection: every addon declaring `stream` for the type/id is asked at once
  (15 s per addon); per-addon outcome shown; streams panel opens from an episode or a movie. (9303070)
- 2026-10-20: M3 stream parser (src-tauri/src/ranking/parse.rs): resolution, HDR/DV, codec, bit depth,
  audio/channels, size, seeders, group, source, debrid cached flag, languages (incl. flag emoji).
  36 tests over 39 titles; 8 are real Torrentio/TorBox results from the owner, the rest follow
  common addon formats. Parsed tags shown in the streams panel. (1f10578)
- 2026-10-20: M3 scoring (ranking/score.rs): cached/uncached debrid, torrent seeders (log scale, dead
  torrents sink), resolution up to a preferred cap, HDR/DV, source, CAM penalty, audio, preferred
  languages, soft size limit and tiny-file penalty; every point has a reason. 16 tests. Streams panel
  is ranked best first with the score and its reasons. Rules use defaults until settings exist. (1e8cc38)
- 2026-10-20: M3 user filters (ranking/filters.rs) for max size, min quality, blocked codecs, plus
  audio/subtitle language order and preferred resolution, saved in settings.json in the app config
  folder. Filtered streams are listed separately with the reason. Settings page with account,
  profile picture, streams and addons. (2d7f266)
- 2026-10-20: M3 manual picker: ranked list with quality chips (counts per resolution), addon
  choice, Play per stream (disabled for web/YouTube), hidden streams pickable too. The pick is
  handed to the details page, which will start the player in M4. M3 code complete. (0e6525b)
- 2026-10-20: M3 accepted by owner on Windows.
- 2026-10-20: M4 libmpv player implemented (not yet ticked: needs a first run on Windows). libmpv is
  loaded at runtime from the bundled libmpv-2.dll and draws into a child window over the web
  view with hwdec=auto-safe; mpv's own controls until our controls item. `pnpm run setup` pins
  and verifies the shinchiro build (scripts/libmpv.json).
- 2026-10-20: First Windows run failed to load libmpv-2.dll (LoadLibraryExW). Pinned build's imports
  checked: all are Windows system DLLs except vulkan-1.dll (installed by GPU drivers). Load errors
  now give the path, the Windows reason and name a missing Vulkan runtime; libmpv's folder is
  searched for its dependencies.
- 2026-10-20: `tauri dev` did not copy libmpv-2.dll beside the dev executable; debug builds now also
  load it from src-tauri/lib, the error lists every path tried, and a failed load is retried.
- 2026-10-20: M4 libmpv embedded with hwdec=auto-safe; owner confirmed video plays on Windows. (fb1a62b)
- 2026-10-20: M4 auto-play and fallback implemented (not yet ticked: needs a Windows check). Episode
  click / Play starts the best direct or debrid stream; the player moves down the ranked list when
  a stream errors, does not start within 30 s, or ends >2 min before its duration, resuming at the
  same position. Manual picks fall back to the rest of the list. player/queue.rs, 11 tests.
- 2026-10-21: M4 auto-play and fallback confirmed by owner on Windows (built on a second PC). (18fd4a8)
- 2026-10-21: M4 player controls implemented (not yet ticked: needs a Windows check). A transparent
  window owned by the main window sits over the video with title, seek bar, play/pause, ±10/30 s,
  volume/mute, audio track, speed and fullscreen; keyboard shortcuts; auto-hide. mpv's own
  controls and key bindings are off. player/state.rs parses mpv's track list (tests).
