# Cerberus Agent, Specification

## What It Is
The self-host media tool for Cerberus Live Studio. It serves an artist's local music folder over a
local HTTP server (Range + CORS), runs a Cloudflare named tunnel from a platform-issued token, and
registers the track list to the artist's dossier. Ships as a Tauri v2 desktop wizard and a Bun CLI
engine. Public streaming, caching, and routing are the platform's media gateway, not the agent.

## Stack

| Layer | Technology |
|---|---|
| Desktop shell | Tauri v2 (Rust backend, webview frontend) |
| Desktop backend | Rust: tiny_http (Range server), std::process (cloudflared, ffprobe), ureq (register) |
| Desktop frontend | React + TypeScript + Vite + Tailwind |
| CLI engine | Bun (Node-style ESM, `src/agent.mjs`) |
| Tunnel | cloudflared (named token mode; quick-tunnel fallback) |
| Durations | ffprobe (FFmpeg), optional |
| Package manager | bun |

## Architecture
The artist's machine is the origin. The agent serves files locally on `:8787` with byte-range
support, and runs cloudflared so the Cerberus media gateway can reach it.

- **Token mode (production):** config has a `tunnelToken` (issued by the platform's
  /account "Set up streaming"). The agent runs `cloudflared tunnel run --token <token>`, which
  connects a stable named tunnel terminating at the hidden host `t-<slug>.cerberuslive.studio`.
  It registers `{ named: true }`; the platform derives the public host from the stored media_origin.
- **Quick-tunnel mode (fallback):** no token. The agent opens an ephemeral `*.trycloudflare.com`
  tunnel and registers that URL. Re-registers each start because the URL changes.

Playback path: browser -> `media.cerberuslive.studio/<slug>/<file>` (platform gateway) -> R2 cache
or the artist's tunnel origin -> the agent's local server.

## Modules

| Module | Responsibility |
|---|---|
| `src/agent.mjs` | Bun CLI engine: scan, serve (Range+CORS), tunnel (token or quick), register |
| `desktop/src-tauri` | Rust backend: same engine reimplemented (tiny_http, cloudflared spawn, ffprobe, ureq) |
| `desktop/src` | React/TS wizard: platform URL, agent key, streaming token, folder picker, live status |
| `cerberus-agent.config.json` | Local config (gitignored): slug, agentKey, musicDir, platformUrl, port, tunnelToken |

## Configuration

| Key | Meaning |
|---|---|
| `slug` | Artist slug (the `/artist/<slug>` part of the dossier) |
| `agentKey` | Per-artist register token (from /account) |
| `musicDir` | Folder of audio files to serve |
| `platformUrl` | Cerberus platform base URL |
| `port` | Local server port (default 8787) |
| `tunnelToken` | Streaming token from "Set up streaming" (enables named token mode) |

## Known Limitations
- The artist must keep the agent running for media that is not yet cached in the platform's R2.
- `cloudflared` must be on PATH (installer bundling is a Stage 4 task).
- The desktop GUI compiles clean but has not been run through on a packaged install.
- No per-track ordering or featured selection yet (the first track is featured); comes with the GUI polish.
- No transcoding: files are served as-is.
