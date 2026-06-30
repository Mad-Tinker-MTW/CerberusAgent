# Work Breakdown Structure
**Cerberus Agent, Cerberus Live Studio self-host media agent**
Document ID: CBA-PMD-002
Version: 1.0
Date: 2026-06-29
Project Manager: Francisco De La Paz

---

#### 1.1 Stage 1: CLI Engine, Complete

| ID | Task | Status |
|---|---|---|
| 1.1.1 | Bun engine scaffold + config schema (slug, agentKey, musicDir, platformUrl, port) | Complete |
| 1.1.2 | Folder scan for audio files (.mp3/.wav/.flac/.m4a/.ogg/.aac) | Complete |
| 1.1.3 | ffprobe durations (best-effort, skipped if absent) | Complete |
| 1.1.4 | Local static server with Range + CORS (audio scrubbing) | Complete |
| 1.1.5 | cloudflared quick tunnel spawn + URL capture | Complete |
| 1.1.6 | Register tunnel URL + track list to /api/agent/register | Complete |
| 1.1.7 | First track marked featured | Complete |
| 1.1.8 | End-to-end verify (X:\Music -> 14 tracks -> dossier playback) | Complete |

---

#### 1.2 Stage 2: Desktop Wizard (Tauri v2), Complete

| ID | Task | Status |
|---|---|---|
| 1.2.1 | Framework decision: Tauri v2 over Electron (footprint) | Complete |
| 1.2.2 | Rust backend: tiny_http Range + CORS server | Complete |
| 1.2.3 | Rust backend: cloudflared spawn + URL capture | Complete |
| 1.2.4 | Rust backend: ffprobe durations + ureq register POST | Complete |
| 1.2.5 | React/TS wizard: folder picker, agent key, must-stay-running disclaimer | Complete |
| 1.2.6 | Live status view (running, tunnel, track count, message) | Complete |
| 1.2.7 | Build clean (cargo build + frontend tsc/vite) | Complete |
| 1.2.8 | Desktop GUI run-through on a real install | Pending |

---

#### 1.3 Stage 3: Named Token-Mode, Complete

| ID | Task | Status |
|---|---|---|
| 1.3.1 | Bun engine: cloudflared tunnel run --token when tunnelToken present | Complete |
| 1.3.2 | Bun engine: register named-mode ({ named: true }); platform derives the public host | Complete |
| 1.3.3 | Tauri backend: token-mode branch (null stdio, settle, register named) | Complete |
| 1.3.4 | Tauri frontend: Streaming-token field (persisted) | Complete |
| 1.3.5 | Config + README document tunnelToken | Complete |
| 1.3.6 | Live production verification (provision + 206 stream + R2 cache for f-de-la-paz) | Complete |

---

#### 1.4 Stage 4: Distribution + Polish, Pending

| ID | Task | Status |
|---|---|---|
| 1.4.1 | NSIS installer for Windows distribution | Pending |
| 1.4.2 | Desktop GUI run-through + screenshots | Pending |
| 1.4.3 | Per-track ordering + featured selection in the GUI | Pending |
| 1.4.4 | Auto-start on login (optional, keep-running aid) | Pending |
| 1.4.5 | In-app agent-key / streaming-token fetch (reduce copy-paste) | Pending |
| 1.4.6 | Register CerberusAgent in TinkerOps dashboard | Pending |
| 1.4.7 | Stage 4 validation + CHANGELOG update | Pending |

---

#### 1.5 Stage 5: Type-aware Library + Auto-sync (L-048 Phase 3), Complete

| ID | Task | Status |
|---|---|---|
| 1.5.1 | Recursive persona-aware scan (folder=persona, subfolder=release, root file=direct single; fixes the flat-scan 0-tracks bug) | Complete |
| 1.5.2 | Embedded-tag auto-import via ffprobe (Album Artist/Artist -> persona, Album -> release, Track -> number, Composer, Title; tags win, folders fallback) | Complete |
| 1.5.3 | Video support (.mp4/.webm/.mov/.m4v/.mkv MIME, registered media_kind=video) | Complete |
| 1.5.4 | Debounced recursive file watcher: re-scan + re-register on change (no restart) | Complete |
| 1.5.5 | Richer register payload (persona/release/releaseKind/mediaKind/trackNo/composer) for the platform reconcile | Complete |
| 1.5.6 | Live run-through against a real per-persona library | Pending |

---

#### 1.6 Project Management (ongoing)

| ID | Task | Status |
|---|---|---|
| 1.6.1 | Maintain BUGS.md | Ongoing |
| 1.6.2 | Maintain CHANGELOG.md | Ongoing |
| 1.6.3 | Update ROADMAP.md checkboxes per stage | Ongoing |
| 1.6.4 | GitHub commits per session | Ongoing |
| 1.6.5 | Re-home Stage 1-3 build hours from CLS-PMD-003 to this WBS at next /audit-project | Pending |

---

## Actual Hours Log

Stage 1-3 build hours were logged in the Cerberus Live Studio WBS (CLS-PMD-003, Stage 3) while the
agent was a companion of the platform. They are cross-referenced below and should be re-homed here
at the next /audit-project (task 1.5.5) so they are counted once. No hours are re-logged in this
file yet to avoid double-counting.

| Date | Work Package | Role | Hours | Logged in |
|---|---|---|---|---|
| 2026-06-28 | CLI engine (serve + tunnel + register) | Lead Developer | 4.0 | CLS-PMD-003 |
| 2026-06-28 | Desktop wizard (Tauri v2, Rust + React) | Lead Developer | 4.5 | CLS-PMD-003 |
| 2026-06-29 | Named token-mode (engine + desktop) | Lead Developer | 2.5 | CLS-PMD-003 |
| 2026-06-29 | Type-aware library + auto-sync (L-048 P3): recursive scan, ffprobe tags, video, watcher | Lead Developer | 2.0 | CLS-PMD-003 (1.3.15) |
| **Subtotal (cross-referenced, not re-counted)** | | | **13.0** | |
