# Changelog — Cerberus Agent

## [0.4.0] — 2026-07-07

The Agent became a real, downloadable, no-paste client.

### Added
- Release CI (`.github/workflows/release.yml`): a `v*` tag builds the NSIS installer via
  tauri-action and publishes it as a GitHub Release asset. First artifact:
  `Cerberus.Agent_0.4.0_x64-setup.exe` (tag v0.4.0). This is the downloadable on-ramp the
  platform lacked ("i cant get the app" on 7/1).

### Verified
- No-paste device-authorization onboarding proven end to end on real hardware: install ->
  "Get a new code" -> enter at cerberuslive.studio/device -> approve -> the Agent self-configures
  (no key/token paste) -> folder pick -> served 21 tracks + full discography to the artist page.
  (Required deploying the platform device routes + fixing their CORS; see CLS 0.12.0.)

### Known
- The spawned cloudflared tunnel opens a visible console window (Command::new without
  CREATE_NO_WINDOW). Must stay open while serving, but should be hidden in a polished build.
- Device-auth uses browser `fetch()` from the Tauri webview, so the platform routes must send
  CORS (fixed platform-side this session).

## [0.3.0] — 2026-06-30

Desktop backend reaches parity with the engine. The 0.2.0 rework landed only in the Bun engine
(`src/agent.mjs`); the Tauri desktop app's Rust backend (`desktop/src-tauri/src/lib.rs`) was still
running the original flat, audio-only, top-level scan — so the desktop app missed music in
subfolders, ignored video, imported no tags, and never re-synced. This release ports the full
L-048 scan model to Rust.

### Fixed
- **Desktop flat-scan bug**: `list_tracks` (top-level `read_dir`, audio-only, duration-only, flat
  payload) replaced with the recursive persona-aware `build_tracks`. The desktop app now picks up
  per-persona subfolders and nested releases that previously registered nothing.

### Added (desktop, porting the engine's 0.2.0 behavior)
- Recursive folder walk (persona = folder, release = sub-folder, root file = direct single).
- Embedded-tag auto-import via ffprobe (album_artist/artist -> persona, album -> release, track ->
  track no, composer, title); tags win over folder names.
- Video support (.mp4/.webm/.mov/.m4v/.mkv) with correct MIME + `mediaKind=video`.
- Release-kind heuristic (single/ep/album by track count) and featured = first audio track.
- Recursive file watcher (`notify` crate): debounced ~2s re-scan + re-register on change, so the
  desktop app keeps the dossier current without a restart.
- Rich register payload matching the engine (persona / release / releaseKind / mediaKind / trackNo /
  composer), and nested-path serving through the existing Range+CORS server.

### Verified
- `cargo check` clean; a unit test (`recursive_persona_release_scan`) covers recursion,
  folder-derived persona/release, video detection, the release-kind heuristic, and direct singles.
- REMAINING: desktop GUI run-through (launch the built app, click the wizard, confirm a real
  multi-subfolder library registers end-to-end). Needs the operator at the machine (L-045).

## [0.2.0] — 2026-06-29

Type-aware library + auto-sync (L-048 Phase 3). The engine now mirrors the platform's new
discography model and keeps the dossier current without a restart.

### Added
- **Recursive, persona-aware scan**: the agent walks the music folder (was a flat top-level scan).
  Folder = persona, sub-folder = release; a file at the root is a direct single. Fixes the bug where
  per-persona subfolders registered 0 tracks.
- **Embedded-tag auto-import** (via ffprobe): Album Artist / Artist -> persona, Album -> release,
  Track -> track number, Composer -> the human creator, Title -> title. Tags win over folder names;
  folder structure is the fallback for untagged originals.
- **Video support**: .mp4 / .webm / .mov / .m4v / .mkv served with correct MIME and registered as
  media_kind=video (the platform's video lane / Live Sets tab).
- **File watcher**: a debounced recursive fs.watch re-scans and re-registers on any change, so
  adding or editing files updates the dossier live (no restart).
- **Richer register payload**: each track now carries persona / release / releaseKind / mediaKind /
  trackNo / composer, which the platform reconciles (find-or-create personas/releases, replace only
  agent-managed tracks, preserve artist-edited dedications).

### Notes
- Built + parsed clean; the register reconcile is verified against the platform D1. The live
  filesystem scan + watcher has not yet been run against a real per-persona library (next run-through).

## [0.1.0] — 2026-06-29

First release line of the Cerberus Live Studio self-host media agent. Built across three same-week
stages alongside the platform's media layer; now live in production for the first artist.

### Added
- **CLI engine** (`src/agent.mjs`): folder scan, local static server with Range + CORS, ffprobe
  durations, cloudflared quick tunnel, and registration to `/api/agent/register` (commit 788d431).
- **Tauri v2 desktop wizard**: Rust backend (tiny_http Range server, cloudflared spawn, ffprobe, ureq
  register) + React/TS wizard (folder picker, agent key, must-stay-running disclaimer, live status).
  Framework decision Tauri over Electron documented (commits d69ab12, 2bea671).
- **Named token-mode**: with a streaming `tunnelToken` the agent runs `cloudflared tunnel run --token`
  (stable named tunnel) instead of an ephemeral quick tunnel, and registers `{ named: true }` so the
  platform derives the public host. Bun engine + Tauri desktop (Streaming-token field) (commit 212907b).

### Verified
- CLI engine end to end (X:\Music -> 14 tracks -> dossier playback).
- Live production: f-de-la-paz provisioned, agent streamed, 206 range responses through
  media.cerberuslive.studio with the R2 cache warming.

### Infrastructure
- Private GitHub repo created: github.com/Mad-Tinker-MTW/CerberusAgent.
