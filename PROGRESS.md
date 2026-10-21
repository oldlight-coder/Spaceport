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
- [x] Controls: play/pause, seek, volume, fullscreen, audio track, speed
- **Acceptance:** a debrid or direct 4K HDR file plays smoothly; killing a stream URL makes the
  player move to the next ranked stream.

## M5. Torrent streaming
- [x] librqbit engine; streams a magnet while downloading
- [x] Local stream server on 127.0.0.1 only; cleans up cache on exit or by size limit
- **Acceptance:** a magnet from a torrent addon starts playing within about 15 seconds on a healthy
  torrent; seeking works; nothing listens on a non-loopback address.

## M6. Subtitles, resume and progress
- [x] Subtitles from addons, embedded tracks and local files; delay and style controls
- [x] Resume position per episode; Continue Watching row
- [x] Mark watched / unwatched; progress synced back to Stremio
- [x] Next-episode countdown
- **Acceptance:** stopping halfway and reopening resumes at the same spot; the same progress shows
  in the official Stremio app.

## M7. AniSkip
- [x] Map title to MyAnimeList ID (Kitsu or AniList)
- [x] Fetch skip times from api.aniskip.com
- [x] Skip Intro / Skip Outro buttons and optional auto-skip
- **Acceptance:** a popular anime episode shows Skip Intro at the right time; auto-skip jumps past
  it when enabled.

## M8. Polish and release
- [x] Settings page (filters, languages, subtitle style, auto-skip, cache size)
- [x] Error states (no addons, no streams, offline)
- [x] README with build steps; final privacy check of all network calls
- [x] **Acceptance:** fresh Windows install from the NSIS installer works end to end: login,
  browse, pick, play, resume, skip intro. (Passed 2026-10-21.)

## M9. Conveniences
- [x] Remove "The picture stays on this computer" from Settings
- [x] Continue Watching: remove one title, clear all
- [x] Search history with single remove and Clear all
- [x] Library tabs: Watching, Movies, Series (and other types)
- [x] Mark titles as "Watching" from their page
- [x] Player keys work from any Spaceport window (J/K/L, arrows, volume)
- [x] New-episode badges on series posters and "New" tags on episodes
- [x] Library sort: recently watched (default), recently added, name
- **Acceptance:** each item works in the installed app.

## M10. Look and feel
- [x] Material You theme from an accent color, with custom colors in Settings
- [x] Light, dark and follow-system modes
- [x] Styled scrollbars and smooth scrolling
- [x] Skeleton loading placeholders
- **Acceptance:** every screen reads well in light and dark mode with a few accent colors.

## M11. Search assistant
- [x] Autocomplete: title suggestions under the search field while typing, arrow keys and Enter
- [x] Forgiving search: typos and partial words still find titles ("Best matches" row)
- **Acceptance:** misspelled titles such as "breakng bad" or "the mandolorian" are found.

## Decisions
(Record any spec changes agreed with the owner here.)
- 2026-10-20: Settings page brought forward from M8 at the owner's request. A cog button in the top
  bar (showing the profile picture) opens Settings with Account, Streams and Addons sections; the
  email and the Addons tab moved there. M8's settings item now adds the remaining settings
  (subtitle style, auto-skip, cache size) to this page.
- 2026-10-20: Profile picture added at the owner's request (not in SPEC). Chosen from a local
  image, shrunk to 128 px and stored only in the local settings file; never uploaded.
- 2026-10-21: At the owner's request, part of M6's subtitle item is brought into M4's controls:
  choosing embedded subtitle tracks and subtitles from addons (the protocol's `subtitles`
  resource) in the player. Local subtitle files and delay/style controls stay in M6. The player
  also gained a Source menu to switch between the ranked streams while watching.
- 2026-10-21: Torrent engine and DHT (owner's choice): librqbit accepts no incoming peer
  connections and uses no UPnP or local peer discovery. DHT is on by default, with a Settings
  switch (Torrents > Find peers with DHT) for trackers-only mode. DHT's UDP socket is the only
  thing that listens on the network, so M5's check reads "nothing but DHT listens on a
  non-loopback address" (and nothing at all with DHT off). The stream server is 127.0.0.1 only.
- 2026-10-21: Privacy check (owner's choice): hosts an installed addon points to (images, stream
  links and debrid download servers, subtitle files, torrent trackers and peers) count as part of
  that addon. They are listed in the README's "Network and privacy" section; no restriction added.
- 2026-10-21: New features requested by the owner after M8 became M9 to M12 and SPEC section 8.
  M8's installer acceptance test is still to be run; it can be run on a later build.
- 2026-10-21: Chromecast dropped (owner's choice): it needed mDNS on the LAN and, for torrents, a
  stream server reachable from the LAN, both against the hard rules.
- 2026-10-21: Player skip keys stay at 10 s for J/L and the arrows (owner's choice).
- 2026-10-21: Android: the owner wants Spaceport to run on Android. It is a large port (player,
  secret storage and windows are Windows-specific), so it comes last as M12 with its own scope.
- 2026-10-21: Ratings beyond IMDb and the Android version are deferred (owner's choice); they
  moved out of the milestone list to "Deferred".
- 2026-10-21: Ratings beyond IMDb (Rotten Tomatoes) and the Android version are dropped from the
  plan (owner's choice); removed from SPEC and from the deferred list.
- 2026-10-21: Pre-merge review (owner's request): 39 findings from four reviews, 35 fixed, 4 not:
  stream URLs from the UI are not tied to addon answers (needs code already running in the
  window, which the CSP prevents), the possible fullscreen freeze was unconfirmed (lock now
  released early anyway), simultaneous addon edits (the UI blocks them; search now uses the
  cached list) and stale settings copies (the settings page now reloads when opened).

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
- 2026-10-21: Controls feedback from owner: skips are both 10 s; Audio, Subtitles (off / in file /
  addons), Source and Speed menus; preferred audio languages passed to mpv (alang); the seek bar
  holds the chosen point and shows "Seeking…" until playback gets there, and warns when a stream
  cannot seek beyond what has loaded. Addon subtitles fetched in parallel (stremio/subtitles.rs).
  (87624cb)
- 2026-10-21: M4 controls confirmed by owner (audio/subtitle/source menus, seeking). (e7f1dcc)
- 2026-10-21: M4 accepted by owner; moving on to M5.
- 2026-10-21: M5 torrent engine implemented (not yet ticked: needs a Windows check). librqbit
  starts on the first torrent play; torrents get a random-token URL on the 127.0.0.1 stream
  server, which adds the magnet (addon trackers included), picks the file, downloads only it
  and serves range requests. End-to-end test with a local seeder passed (IPv4-patched copy here,
  as this sandbox has no IPv6); it skips itself where IPv6 sockets are unavailable.
- 2026-10-21: M5 torrent playback confirmed by owner on Windows (Torrentio without debrid). (3516e08)
- 2026-10-21: M5 cache cleanup: the torrent cache is wiped on start and exit; a size limit in
  Settings (10 GB default) deletes idle torrents least recently watched first, never the one
  playing (open stream or used in the last 2 min); torrents nobody watches are paused when the
  player closes and resume when reopened. Stream server stays 127.0.0.1-only. M5 code complete.
- 2026-10-21: M5 accepted by owner on Windows (torrent start, seeking, listeners, cache cleanup).
  Besides DHT, librqbit's UDP tracker client socket is bound on all interfaces; it only talks to
  trackers Spaceport contacts.
- 2026-10-21: M6 subtitles implemented (not yet ticked: needs a Windows check). Load a subtitle file
  from the computer (file picker), delay -/+0.1 s (also Z/X) and reset, size -/+ in the player;
  Settings > Subtitles for size, colour, outline or box, position (applied live and on start);
  preferred subtitle languages auto-select a track in the file when the audio differs.
- 2026-10-21: M6 subtitles confirmed by owner on Windows. (3f4b009)
- 2026-10-21: M6 resume and Continue Watching implemented (not yet ticked: needs a Windows check).
  Progress is saved every 30 s, on pause, at the end and on close: locally per video
  (resume.json) and in the title's Stremio library item (video_id, timeOffset/duration in ms,
  watch time; new titles as temporary entries like the official apps). Playback resumes at the
  newer of the two with a Start over option; Continue Watching follows Stremio's rules.
- 2026-10-21: M6 resume and Continue Watching confirmed by owner, including progress in the official
  Stremio app. (c7a15a5)
- 2026-10-21: M6 watched marks implemented (not yet ticked: needs a Windows check). Episodes, seasons
  and whole titles can be marked watched/unwatched; stored in the library item like Stremio
  (flaggedWatched, and state.watched as anchor:length:base64(zlib(bits)) in the addon's video
  order, shifted by the anchor when videos change). Videos played to 90% are marked automatically.
- 2026-10-21: M6 watched marks confirmed by owner, matching the official app. (3449b4d)
- 2026-10-21: M6 next-episode countdown implemented (not yet ticked: needs a Windows check). 30 s
  before the end a card counts down from 10 (Play now / Cancel); the next episode in season order
  (specials only to specials) starts in the same player, preferring the same bingeGroup. M6 code
  complete.
- 2026-10-21: M6 next-episode countdown confirmed; M6 accepted by owner on Windows. (50a8a25)
- 2026-10-21: M7 AniSkip implemented (not yet ticked: needs a Windows check; Kitsu, AniList and
  AniSkip are unreachable from the build sandbox). MAL id from kitsu:/mal:/anilist: ids or, for
  IMDb series with an Animation genre, an AniList search that must match the title (and season);
  skip times fetched with the episode length; Skip Intro/Outro/Recap button and an auto-skip
  switch in Settings > Anime. aniskip.rs parsing and matching have unit tests. M7 code complete.
- 2026-10-21: AniSkip fix after "no button for Frieren": requests now send a User-Agent, AniList
  is also searched with the name before ":", and when no skip times load the player top bar says
  where the lookup stopped ("AniSkip: ..."). Tests for both cases.
- 2026-10-21: Kitsu answered 406 to the MyAnimeList lookup; it now gets JSON:API's Accept header.
- 2026-10-21: Resume keeps the source: the stream playing (after a manual pick, fallback or Source
  menu switch) is saved with the position in resume.json by a key (torrent hash and file, or addon
  and file name; never the link) and goes first when that video is played again.
- 2026-10-21: M7 accepted by the owner on Windows (Frieren shows Skip Intro). M7 ticked.
- 2026-10-21: M8 Settings page ticked: filters, languages, subtitle style, auto-skip and cache
  size were built and tested with the earlier milestones.
- 2026-10-21: M8 error states implemented (not yet ticked: needs a Windows check): offline banner,
  failed pages and sign-in retry when the connection returns, network errors say "no connection"
  or "timed out", empty catalogs/search/streams offer "Manage addons", and Play explains why no
  stream started (no stream addons, none answered, none found, all hidden by filters).
- 2026-10-21: M8 error states accepted by the owner. Privacy check done: every hard-coded host is
  on the allowed list, no logging, no updater or telemetry, mpv loads no scripts, secrets only in
  Credential Manager, no links on disk. README gained a "Network and privacy" section.
- 2026-10-21: M9 implemented (not yet ticked: needs a Windows check): picture note removed from
  Settings; Continue Watching cards get a remove button and the row a Clear all (rewinds the
  title in Stremio, like the official Dismiss, and forgets local positions); recent searches
  under the search field with remove and Clear all (stored on this computer); Library grouped
  into Movies, Series and other types; player keys also work while the main window has focus
  and ignore Caps Lock/Shift.
- 2026-10-21: Library groups became tabs (Watching, Movies, Series, other types; last tab
  remembered), and titles can be marked "Watching" on their page (owner's request). Marks are
  kept on this computer only: Stremio's library has no such field.
- 2026-10-21: Watching mark is a bookmark (button and a ribbon on every poster of a marked title, in all rows and the Library); Library tabs put Series before Movies.
- 2026-10-21: M9 items accepted by the owner on Windows and ticked.
- 2026-10-21: New episodes (owner's request, added to M9; not yet ticked): an episode is new when
  it came out after the series was last watched (Stremio's lastWatched, or the Watching mark's
  time) and is not marked watched. Checked for library, Continue Watching and Watching series;
  details cached 3 h. Posters show "New episode"/"N new episodes", episode lists a "New" tag.
- 2026-10-21: Library sort menu (owner's request): recently watched (Stremio's lastWatched; never
  watched after, newest added first), recently added (_ctime; for Watching marks the mark time)
  and name. The choice is remembered.
- 2026-10-21: New-episode badges and library sort accepted by the owner; M9 complete.
- 2026-10-21: M10 implemented (not yet ticked: needs a Windows check): Settings > Appearance with
  theme (Follow Windows, Light, Dark) and accent colour (presets or any colour). The Material 3
  palette is built from the accent with @material/material-color-utilities (Apache-2.0, no
  network use) and applied as CSS variables; the player controls use the accent but stay dark.
  Pill buttons, filled cards, tonal chips and tabs, hover/press state layers, thin themed
  scrollbars, smooth scrolling (off with reduced motion), shimmer skeletons on every loading
  screen and a spinner at start-up.
- 2026-10-21: Catalogs that answer with a single "Error…" placeholder title (TMDB addon on failure) are treated as empty, so their rows no longer show an X card.
- 2026-10-21: M10 accepted by the owner on Windows; ticked.
- 2026-10-21: M11 search assistant (owner's request; not yet ticked): fuzzy.rs scores titles
  against the query (accents, punctuation, typos with swaps, unfinished last word) and makes
  looser forms of the query (words cut to four letters, longest word alone) for addons' exact
  search; search.rs merges and ranks. search_assist asks every search catalog (looser forms
  always for the search page, for autocomplete only when the query finds under 3), caching
  answers for 10 minutes. Suggestions under the search field, "Best matches" row on the page.
- 2026-10-21: M11 accepted by the owner on Windows; ticked.
- 2026-10-21: M8 acceptance passed on a fresh Windows install from the NSIS installer; M8 ticked. All milestones complete.
- 2026-10-21: Pre-merge review fixes: torrents (unsafe file names refused, hash-named folders,
  streams freed when the player hangs up, whole-start timeout, in-use marking, season packs),
  player (own-session teardown, cleanup on failed start, background subtitle loads, save on
  exit), backend (redirect and Referer limits, web-only configure links, account caches cleared,
  session kept on other errors, safe resume.json, ranking title words, sizes), frontend (search
  Enter crash, error screen, play/load races, stable player keys, size field, settings merges),
  build (libmpv DLL hash pinned and checked before bundling). 197 tests pass.
