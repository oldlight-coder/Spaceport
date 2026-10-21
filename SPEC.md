# Spaceport feature spec

Spaceport is a standalone desktop Stremio client for Windows. It is built from scratch and relies
only on the public Stremio protocol and open-source libraries. It has no servers, update channels
or hosted services of its own.

## 1. Stremio account and library
- Sign in with a Stremio account through Stremio's public API (`api.strem.io`).
- Sync the library, watch progress and installed addons both ways.
- Continue Watching row and per-episode resume position.
- Mark titles and episodes watched or unwatched.

## 2. Addons
- Install by manifest URL, remove, reorder, and open each addon's configure page.
- Follow the official Stremio addon protocol (`manifest`, `catalog`, `meta`, `stream`,
  `subtitles` resources). Reference: `Stremio/stremio-addon-sdk` on GitHub, `docs/` folder.
- Cinemeta as the default metadata source.

## 3. Browsing
- Home rows built from addon catalogs.
- Movies, Series and Anime pages with genre filters and infinite scroll.
- Details page: poster, backdrop, description, cast, seasons and episodes.
- Search across all addon catalogs.

## 4. Stream selection: Spaceport's own ranking
- Query every addon's `stream` resource in parallel, with a per-addon timeout.
- Parse stream names and titles into fields: resolution, HDR/DV, codec, audio, size, seeders,
  release group, cached-on-debrid.
- Score with configurable rules: seeders, quality, size limits, language, cached status.
- Auto-play the top stream. Fall back to the next one if it fails to start.
- Manual picker showing the ranked list.
- User filters: maximum size, minimum quality, blocked codecs, preferred audio and subtitle
  languages.

## 5. Playback
- mpv-based player (libmpv) with hardware decoding.
- Torrent streaming through a built-in engine and a local HTTP server bound to `127.0.0.1` only.
- Debrid direct links (Real-Debrid, TorBox, AllDebrid, Premiumize) via addons.
- Subtitles from addons, embedded tracks or local files, with delay and style controls.
- Audio track switching, playback speed, and a next-episode countdown.
- Reports progress back to Stremio.

## 6. AniSkip
- Map the title to a MyAnimeList ID (Kitsu or AniList public mapping APIs).
- Fetch skip times from `api.aniskip.com/v2/skip-times/{malId}/{episode}`.
- Skip Intro and Skip Outro buttons, plus an optional auto-skip setting.

## 7. Privacy and safety rules
- No Spaceport servers, accounts, telemetry or crash uploads.
- Network traffic only to Stremio, installed addons, debrid services, AniSkip and the ID-mapping
  APIs.
- No auto-updater. Updates come from rebuilding source.
- No plugin system that runs downloaded code. Only Stremio addons, which are HTTP APIs.
- Settings and tokens stored locally, with debrid keys in the Windows Credential Manager.

## 8. Additions agreed with the owner (2026-10-21)
- Continue Watching: remove one title or clear the whole row.
- Search history: recent searches, each removable, with a Clear all.
- Library in tabs: Watching, Movies and Series. Titles can be marked "Watching" from their
  page; the mark is kept on this computer. Sorted by recently watched, recently added or name.
- New episodes: series in the library, Continue Watching or marked Watching show a badge when
  an episode came out since they were last watched; the episode list tags the new ones.
- Search assistant: title suggestions while typing, and search that forgives typos and partial
  words, using only the installed addons' search.
- Player keys: J/Left back 10 s, K/Space pause, L/Right forward 10 s, Up/Down volume; they work
  whichever Spaceport window has focus.
- Look: Material You (Material 3) styling with a chosen accent color, light, dark or follow the
  system theme, styled scrollbars, smooth scrolling and skeleton loading placeholders.

## 9. Technology
| Part | Choice | Notes |
| --- | --- | --- |
| App shell | Tauri 2 | Small installer, uses Windows WebView2 |
| Interface | React + TypeScript (Vite) | |
| Player | libmpv (official shinchiro/mpv-winbuild-cmake builds, pinned SHA-256) | Embedded in the Tauri window |
| Torrents | librqbit (Rust) | Streams while downloading |
| Ranking | Own Rust module | Unit-tested scoring rules |
| Installer | Tauri NSIS bundle | |
