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
- [ ] Search across all addon catalogs
- **Acceptance:** installing an addon by URL shows its catalogs on Home; opening a series shows
  every season and episode; search returns results from more than one addon.

## M3. Stream collection and ranking
- [ ] Query all addons' `stream` resource in parallel with per-addon timeout
- [ ] Parser for resolution, HDR/DV, codec, audio, size, seeders, release group, cached flag
- [ ] Scoring rules (seeders, quality, size, language, cached) with unit tests
- [ ] User filters: max size, min quality, blocked codecs, preferred languages
- [ ] Manual picker with the ranked list
- **Acceptance:** for a popular movie the top pick is a well-seeded or cached stream within the
  filters; parser tests cover at least 30 real-world stream titles.

## M4. Player
- [ ] libmpv embedded in the window, hardware decoding on
- [ ] Plays direct and debrid links; auto-plays top stream, falls back to next on failure
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
  episodes grouped by season with specials last.
