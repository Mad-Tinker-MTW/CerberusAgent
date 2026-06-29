# Changelog — Cerberus Agent

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
