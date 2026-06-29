# Cerberus Agent

The Cerberus Live Studio self-host media agent. It serves an artist's local music folder over a local
HTTP server (Range + CORS) and runs a Cloudflare named tunnel that Cerberus provisioned for them, so
their tracks stream through the platform. The artist's machine is the storage; Cerberus stores only
the tunnel binding and the track list. Keep it running while you want media that is not yet cached to
be live.

Two forms:
- **`desktop/`**, the branded **Tauri v2** wizard app (Rust backend + React/TS frontend). The product
  artists download. Build: `cd desktop && bun install && bun run tauri build` (dev: `bun run tauri dev`).
  Framework rationale in `DECISION-framework.md`.
- **`src/agent.mjs`**, the **Bun CLI engine**, the reference implementation and a headless fallback.

## Prereqs
- [bun](https://bun.sh)
- [cloudflared](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/) on PATH
- ffprobe (FFmpeg) on PATH for durations (optional; skipped if absent)

## Use (CLI engine)
1. Copy the config and fill it in:
   ```
   cp cerberus-agent.config.example.json cerberus-agent.config.json
   ```
   - `slug`, your artist slug (`/artist/<slug>`)
   - `agentKey`, generated in your Cerberus account ("Set up streaming" / agent key)
   - `musicDir`, folder of audio files (.mp3/.wav/.flac/.m4a/.ogg/.aac)
   - `platformUrl`, the Cerberus platform base URL
   - `tunnelToken`, your streaming token from /account "Set up streaming" (enables the stable named tunnel)
2. Run it:
   ```
   bun run start
   ```
3. Leave the window open. Your tracks are live on your dossier. Ctrl+C to stop.

## How it streams
- **With `tunnelToken` (recommended):** the agent runs `cloudflared tunnel run --token`, a stable
  named tunnel Cerberus provisioned. It registers `{ named: true }`; the platform already knows your
  public host. Playback goes through `media.cerberuslive.studio/<slug>/<file>`, which R2-caches plays
  so hot tracks keep streaming even when your machine is off.
- **Without a token:** the agent falls back to an ephemeral `*.trycloudflare.com` quick tunnel and
  re-registers the new URL each start. Zero-config, but not stable.

## Repo
github.com/Mad-Tinker-MTW/CerberusAgent (private)

## State
v0.1.0, live in production for the first artist. CLI engine verified end to end; named token-mode
done. Pending: NSIS installer, desktop GUI run-through, per-track ordering. Full docs in `docs/`
(SPEC, VISION, ROADMAP, BUGS, CHANGELOG) and `docs/PMP/` (CBA-PMD-001..005).
