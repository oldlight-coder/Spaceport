# Spaceport: instructions for Claude

Spaceport is a personal, standalone Stremio client for Windows (and, from M15, Android). Read `SPEC.md` for what to build
and `PROGRESS.md` for where the work stands. Both files are the source of truth; this conversation
is not.

## How to work
- At the start of every session: read `SPEC.md` and `PROGRESS.md`, then continue with the first
  unchecked item in `PROGRESS.md`.
- Work one milestone at a time. A milestone is done only when every acceptance check under it
  passes. Do not start the next milestone with a failing check.
- After finishing an item: tick it in `PROGRESS.md`, add a one-line note under "Log", and commit.
- If a spec item turns out impractical, stop and ask the owner. Do not silently drop or swap it.
  Record the decision in `PROGRESS.md` under "Decisions".
- Keep features out that are not in `SPEC.md`.

## Hard rules
- Write all code from scratch. Do not copy code or assets from other Stremio client projects;
  use only the official Stremio documentation and open-source libraries added as dependencies.
- No auto-updater, telemetry, analytics, crash upload, accounts or servers of our own.
- Only these network destinations: `api.strem.io` and Stremio's own endpoints, addon URLs the user
  installed, debrid APIs, `api.aniskip.com`, Kitsu/AniList ID mapping, Cinemeta.
- No code that downloads and executes scripts or binaries at runtime.
- Local servers (torrent stream server) bind to `127.0.0.1` only.
- Build-time downloads (libmpv, etc.) come from the official upstream project and are checked
  against a pinned SHA-256.
- Secrets (Stremio auth key, debrid keys) go in the Windows Credential Manager (Android: the
  Android Keystore), never in plain files or logs.

## Checks before every commit
- `pnpm typecheck` (tsc --noEmit) passes.
- `cargo test --manifest-path src-tauri/Cargo.toml` passes.
- `cargo clippy --manifest-path src-tauri/Cargo.toml` has no new warnings.
- New ranking or parsing logic has unit tests.

## Build (Windows)
- Prerequisites: VS 2022 Build Tools (Desktop C++), Rust (MSVC), Node 22 + pnpm, 7-Zip on PATH,
  CMake.
- `pnpm install`, `pnpm run setup` (fetches libmpv into `src-tauri/lib`, checked against the
  archive and DLL SHA-256s in `scripts/libmpv.json`), `pnpm tauri build` (refuses to bundle a
  DLL that does not match the pin).
- To move to a newer libmpv: `pnpm run setup -- --pin`, which checks the download against the
  digest GitHub publishes for the release asset, records the archive and DLL hashes in
  `scripts/libmpv.json`; commit it.
- Installer: `src-tauri/target/release/bundle/nsis/Spaceport_<version>_x64-setup.exe`.
- Keep this section and `README.md` accurate whenever setup changes.
