# Changelog — Cerberus Agent

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
