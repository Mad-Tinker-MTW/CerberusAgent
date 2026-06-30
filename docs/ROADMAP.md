# Cerberus Agent, Roadmap

---

## Stage 1: CLI Engine (complete)
Goal: serve a folder and get it onto a dossier.

- [x] Bun engine scaffold + config schema
- [x] Folder scan for audio files
- [x] ffprobe durations (best-effort)
- [x] Local static server with Range + CORS
- [x] cloudflared quick tunnel spawn + URL capture
- [x] Register tracks + tunnel URL to /api/agent/register
- [x] End-to-end verified (14 tracks -> dossier playback)

---

## Stage 2: Desktop Wizard (complete)
Goal: a non-technical artist can go live from one screen.

- [x] Framework decision: Tauri v2 over Electron
- [x] Rust backend: tiny_http Range server + cloudflared spawn + ffprobe + ureq register
- [x] React/TS wizard: folder picker, agent key, disclaimer, live status
- [x] Build clean (cargo + frontend)
- [ ] Desktop GUI run-through on a real install

---

## Stage 3: Named Token-Mode (complete)
Goal: a stable tunnel the platform provisions, not an ephemeral one.

- [x] Bun engine: cloudflared tunnel run --token when tunnelToken present
- [x] Register named-mode ({ named: true })
- [x] Tauri backend token-mode branch + frontend Streaming-token field
- [x] Config + README document tunnelToken
- [x] Live production verification (206 stream + R2 cache for f-de-la-paz)

---

## Stage 4: Distribution + Polish (planned)
Goal: ship it to artists and round off the edges.

- [ ] NSIS installer (bundle or document cloudflared)
- [ ] Desktop GUI run-through + screenshots
- [ ] Per-track ordering + featured selection in the GUI
- [ ] Auto-start on login
- [ ] In-app agent-key / streaming-token fetch (less copy-paste)
- [ ] Register CerberusAgent in TinkerOps
- [ ] Re-home Stage 1-3 build hours from CLS-PMD-003

---

## Stage 5: Type-aware Library + Auto-sync (L-048 Phase 3, complete)
Goal: mirror the platform's discography model and keep the dossier in sync live.
Built after Stage 3; Stage 4 distribution remains planned.

- [x] Recursive persona-aware scan (folder=persona, subfolder=release, root=direct single)
- [x] Embedded-tag auto-import via ffprobe (Album Artist/Album/Track/Composer/Title)
- [x] Video support (.mp4/.webm/.mov/.m4v/.mkv, media_kind=video)
- [x] Debounced recursive file watcher (re-scan + re-register, no restart)
- [x] Richer register payload (persona/release/releaseKind/mediaKind/trackNo/composer)
- [ ] Live run-through against a real per-persona library
