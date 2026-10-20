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
refuses it unless its SHA-256 matches the pinned one, and extracts `libmpv-2.dll` into
`src-tauri/lib` with 7-Zip. The installer ships that DLL next to the app. Run it again after a
pin change; it does nothing when the pinned file is already in place.

To pin a build (first time, or to update): `pnpm run setup -- --pin` takes the newest release, or
`pnpm run setup -- --pin <tag>` a specific one. It checks the download against the SHA-256 GitHub
publishes for the asset and writes name and hash to `scripts/libmpv.json`. Commit that file.

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

Preferences (languages, stream filters, profile picture) are saved in `settings.json` in the app
config folder (`%APPDATA%\com.spaceport.client` on Windows). It holds no secrets.
