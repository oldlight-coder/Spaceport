# Spaceport

Standalone desktop Stremio client for Windows, built with Tauri 2, React and TypeScript.
See `SPEC.md` for the feature list and `PROGRESS.md` for the current state.

## Build (Windows)

Prerequisites: VS 2022 Build Tools (Desktop C++), Rust (MSVC toolchain), Node 22 + pnpm,
7-Zip on PATH, CMake.

```
pnpm install
pnpm run setup
pnpm tauri build
```

The installer is written to
`src-tauri/target/release/bundle/nsis/Spaceport_<version>_x64-setup.exe`.

`pnpm run setup` downloads the libmpv developer build named in `scripts/libmpv.json` from the
official [shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake) releases,
refuses it unless its SHA-256 matches the pinned one, extracts `libmpv-2.dll` with 7-Zip and
installs it into `src-tauri/lib` only if the DLL's own SHA-256 (also pinned) matches. The
installer ships that DLL next to the app; `pnpm tauri build` checks it against the pin first
and stops if it differs. Run setup again after a pin change; it does nothing when the pinned
DLL is already in place.

To pin a build (first time, or to update): `pnpm run setup -- --pin` takes the newest release, or
`pnpm run setup -- --pin <tag>` a specific one. It checks the download against the SHA-256 GitHub
publishes for the asset and writes its name and hash, and the DLL's hash, to
`scripts/libmpv.json`. Commit that file.

The shinchiro builds are GPL-licensed; Spaceport loads the DLL at runtime and is meant for personal
use.

## Development

```
pnpm tauri dev                                       # run the app with hot reload
pnpm typecheck                                       # tsc --noEmit
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml
```

The Stremio auth key is stored in the Windows Credential Manager (entry `Spaceport`). On
non-Windows dev builds an in-memory store is used, so sign-in does not survive a restart there.

Preferences (languages, stream filters, profile picture, theme and accent colour) are saved in `settings.json` in the app
config folder (`%APPDATA%\com.spaceport.client` on Windows). It holds no secrets. The position of
every video you watch is kept next to it in `resume.json`, with the stream it was playing so a
resumed video starts on the same source (named by torrent hash or addon and file name, never by
link). The last position per title is also saved in your Stremio library, as the official apps do.

Torrents are downloaded to the app cache folder (`%LOCALAPPDATA%\com.spaceport.client\torrents` on
Windows). Torrents you are not watching are deleted once the cache passes the size set in
Settings (10 GB by default), and the whole folder is deleted when Spaceport closes. The player
reads torrents through a stream server bound to 127.0.0.1 only.

## Network and privacy

Spaceport has no accounts, servers, telemetry, crash reports or auto-updater of its own. It
contacts only:

- `api.strem.io`: sign-in, your addon collection, library and watch progress.
- Cinemeta (`v3-cinemeta.strem.io`), the default catalog and metadata addon.
- The addons you installed: their manifests, catalogs, metadata, streams and subtitles.
- What those addons point to: poster and background images, stream links (for debrid addons,
  the debrid service's download servers), subtitle files, and for torrents the trackers listed
  with the stream and peers found through them or through DHT (Settings > Torrents). DHT
  starts from librqbit's two bootstrap nodes, `dht.transmissionbt.com` and
  `dht.libtorrent.org`.
- For anime skip times: Kitsu (`kitsu.app`) or AniList (`graphql.anilist.co`) to find the
  MyAnimeList id, and `api.aniskip.com`.
- An addon's configure page opens in your own browser, not in Spaceport.

Addon requests may follow a redirect only within the same host or to another public https
host (never to this computer or the local network), and Stremio account requests only within
`strem.io`; no Referer header is sent. Addon configure pages open only when they are web
links. Torrents whose file names could point outside the download folder are refused.

Nothing downloads or runs code at runtime: mpv starts with `config=no`, `load-scripts=no` and
`ytdl=no`. The only listening sockets are the torrent stream server on 127.0.0.1 and, while DHT
is on, DHT's UDP socket; the engine accepts no incoming peer connections and uses no UPnP or
local peer discovery. Images load without a referrer. Request errors never include URLs, since
addon and debrid links can carry API keys, and Spaceport never writes addon, stream or debrid
links to disk. (Poster links of titles marked Watching are kept, to show them offline.)

