# Agent GUI framework — decision

**Decision: Tauri v2** for the branded wizard app. (2026-06-28)

The plan docs (ULTRAPLAN / WBS 1.3.1) originally named Electron, but the MTW global
standard is Tauri v2, and on assessment Electron adds no benefit this app needs:

| Need | Tauri v2 | Electron |
|---|---|---|
| Serve local folder w/ Range | Rust (axum/hyper), native, tiny | Node http/express |
| Spawn cloudflared | Tauri sidecar / shell command | child_process |
| ffprobe durations | Command | child_process |
| Register (HTTP POST) | reqwest | fetch |
| Wizard GUI | webview + React/TS/Tailwind | Chromium + React |
| Footprint (long-running bg utility) | lean native webview (~few MB) | bundles Chromium + Node (~100MB+) |

The only Electron edge is reusing the existing Node engine (`src/agent.mjs`) as-is.
That code is small and ports cleanly to Rust, and a self-host agent that an artist
leaves running in the background should be light, so the footprint wins.

**Plan:**
- The Rust backend reimplements the proven engine: static file server (Range + CORS),
  cloudflared spawn + URL capture, ffprobe durations, register POST. Frontend is a
  React/TS/Tailwind wizard (Cerberus-branded): sign-in / paste agent key, pick music
  folder, "keep this running" disclaimer, Start → shows live status + the tunnel URL.
- `src/agent.mjs` stays as the reference implementation and a working CLI fallback.

**Tunnel model (per operator):** the wizard sets up the artist's own tunnel; the tunnel
URL is registered to the dossier and visible to the artist (in /account) and to the
admin (artist_profiles.tunnel_url; surfaced in the admin dashboard) so it can be
troubleshooted or a replacement tunnel provisioned. Quick tunnel for zero-config start;
named tunnel (stable hostname) as the upgrade.
