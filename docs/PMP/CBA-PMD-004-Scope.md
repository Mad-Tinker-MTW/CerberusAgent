# Scope Statement
**Cerberus Agent, Cerberus Live Studio self-host media agent**
Document ID: CBA-PMD-004
Version: 1.0
Date: 2026-06-29
Project Manager: Francisco De La Paz

---

## Project Description

Cerberus Agent is the artist-side companion to Cerberus Live Studio. It serves an artist's local
music folder over a local HTTP server (Range + CORS), runs a Cloudflare named tunnel from a
platform-issued token, and registers the track list to the artist's dossier. It ships as a Tauri v2
desktop wizard (the downloadable product) and a Bun CLI engine (reference + headless fallback). The
public streaming, caching, and routing are handled by the platform's media gateway, not the agent.

---

## Product Scope

### Stage 1: CLI Engine, Complete

**Media serving**
- Folder scan for audio files (.mp3/.wav/.flac/.m4a/.ogg/.aac)
- Static server with HTTP Range support (audio scrubbing) and CORS
- ffprobe durations, best-effort (skipped if ffprobe absent)
- First track marked featured

**Tunnel + registration**
- cloudflared quick tunnel spawn + trycloudflare URL capture
- POST track list + tunnel URL to `/api/agent/register` (Bearer agent key)
- Re-register on each start (quick-tunnel URL changes per run)

### Stage 2: Desktop Wizard (Tauri v2), Complete

- Rust backend reimplements the engine: tiny_http Range server, cloudflared spawn, ffprobe, ureq register
- React/TS/Tailwind wizard: platform URL, agent key, folder picker, must-stay-running disclaimer
- Live status: running state, tunnel, track count, message
- Cerberus-branded single-screen flow

### Stage 3: Named Token-Mode, Complete

- When a streaming `tunnelToken` is present, run `cloudflared tunnel run --token` (stable named tunnel)
  instead of an ephemeral quick tunnel
- Register `{ named: true }`; the platform derives the public host from the provisioned media_origin
- Desktop Streaming-token field (persisted); config + README document the field
- Verified live in production: f-de-la-paz provisioned, agent streamed, 206 through the gateway, R2 cache warm

---

## Not In Scope

- Media transcoding, normalization, or editing (files served as-is)
- The R2 read-through cache, public host, and TLS (platform media gateway, not the agent)
- Per-artist Cloudflare accounts (rejected; the platform provisions tunnels centrally)
- Mobile or web builds (desktop background utility only)
- Upload-to-cloud media hosting (that is the platform's separate admin-hosted R2 tier)

---

## Deliverable Acceptance Criteria

### Stage 1: CLI Engine (complete)
- A configured engine serves a folder and registers tracks that appear on the dossier
- Range requests return 206 so audio scrubs
- Verified: X:\Music -> 14 tracks -> dossier playback

### Stage 2: Desktop Wizard (complete except GUI run-through)
- cargo build + frontend build clean
- Wizard collects platform URL, agent key, folder; shows live status
- GUI run-through on a real install: Stage 4 (1.4.2)

### Stage 3: Named Token-Mode (complete)
- With a token, the agent runs a stable named tunnel and registers named-mode
- Tracks stream through media.cerberuslive.studio with 206 + R2 caching
- Verified live for f-de-la-paz

### Stage 4: Distribution + Polish (pending)
- NSIS installer produces a working Windows install (cloudflared bundled or documented)
- Per-track ordering + featured selection in the GUI
- GUI run-through documented with screenshots

---

## Constraints

- Solo developer, no external team
- Desktop background utility: must stay light (the Tauri-over-Electron decision)
- Depends on the Cerberus platform's register + provision contracts
- `wrangler dev` / workerd does not run on the Windows dev box; the platform side verifies at deploy
- Uses the Cerberus Cloudflare account (no separate hosting cost)

---

## Assumptions

- The artist keeps the agent running while they want live (uncached) media
- cloudflared is installed (or bundled by the installer in Stage 4)
- The Cerberus platform issues the streaming token via /account "Set up streaming"
- The register contract (`/api/agent/register`) is stable and owned by the platform
