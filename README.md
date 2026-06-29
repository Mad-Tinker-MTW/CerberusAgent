# Cerberus Agent

The Cerberus Live Studio self-host media agent. It serves an artist's local music
folder through a Cloudflare quick tunnel and registers the public URL + track list
to their dossier. The artist's machine is the storage; Cerberus stores only the
tunnel URL. Keep it running while you want your media live.

Two forms:
- **`desktop/`** — the branded **Tauri v2** wizard app (Rust backend reimplements the
  engine; React/TS frontend). This is the product artists download. Build:
  `cd desktop && bun install && bun run tauri build` (dev: `bun run tauri dev`).
  Framework rationale in `DECISION-framework.md`.
- **`src/agent.mjs`** — the original Bun engine, kept as a reference implementation and a
  working CLI fallback.

The notes below cover the CLI engine.

## Prereqs
- [bun](https://bun.sh)
- [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) on PATH
- ffprobe (FFmpeg) on PATH for track durations (optional; durations are skipped if absent)

## Use
1. Copy the config and fill it in:
   ```
   cp cerberus-agent.config.example.json cerberus-agent.config.json
   ```
   - `slug` — your artist slug (the `/artist/<slug>` part of your dossier)
   - `agentKey` — generated in your Cerberus account ("Connect your agent")
   - `musicDir` — folder of audio files to serve (.mp3/.wav/.flac/.m4a/.ogg/.aac)
   - `platformUrl` — the Cerberus platform base URL
2. Run it:
   ```
   bun run start
   ```
3. Leave the window open. Your tracks are live on your dossier. Ctrl+C to stop.

## Notes
- Quick tunnels get a random `*.trycloudflare.com` URL that changes each run, so the
  agent re-registers the new URL on every start. A named tunnel (stable URL) is a
  later upgrade.
- The first track is marked featured. Per-track ordering/featured selection comes
  with the GUI.
