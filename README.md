# Obel

A local-first music player built with Tauri 2, Rust and vanilla TypeScript. Manage your library, browse albums, and sync tracks to portable USB devices.

## Features

- **Library management** — add folders, scan for audio, edit metadata, delete albums.
- **Album browsing** — grid view grouped by album title, with compilations labeled "Various Artists".
- **Playback** — local playback via rodio, with disc (compact) mode, next/prev, seek, volume, and Discord Rich Presence.
- **Lyrics** — LRC lyrics support.
- **Sync** — copy only changed tracks to removable USB volumes (targets the Snowsky Echo Mini), regenerating an `Artist/Album/{track} - {title}.{ext}` structure with size-based dedup. Never deletes on the device.
- **YouTube download** — download audio from a YouTube URL.

## Tech stack

- [Tauri 2](https://tauri.app/) — desktop shell
- Rust — backend (playback via `rodio`, SQLite via `rusqlite`, tags via `lofty`, sync, YouTube)
- TypeScript + Vite — frontend
- Vanilla HTML/CSS with a TUI-styled theme

## Requirements

- Node.js + npm
- Rust toolchain (stable)
- Windows (sync uses removable-volume detection)

## Development

```bash
npm install
npm run tauri dev
```

Build a release bundle:

```bash
npm run tauri build
```

Frontend-only checks:

```bash
npx tsc --noEmit
npm run build
```

## Project layout

```
src-tauri/          Rust backend
  src/commands/     Tauri command handlers
  src/core/         playback, library, metadata, sync
  src/db/           SQLite schema + queries
src/                TypeScript frontend
src/main.ts         entry point
```

## License

[MIT](./LICENSE)
