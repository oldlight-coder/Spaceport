# Spaceport

Standalone desktop Stremio client for Windows, built with Tauri 2, React and TypeScript.
See `SPEC.md` for the feature list and `PROGRESS.md` for the current state.

## Build (Windows)

Prerequisites: VS 2022 Build Tools (Desktop C++), Rust (MSVC toolchain), Node 22 + pnpm,
7-Zip on PATH, CMake.

```
pnpm install
pnpm tauri build
```

The installer is written to
`src-tauri/target/release/bundle/nsis/Spaceport_<version>_x64-setup.exe`.

`pnpm run setup` (fetches libmpv with a pinned SHA-256) is added in milestone M4 and will be
required before `pnpm tauri build` from then on.

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
