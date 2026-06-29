# Project Schedule
**Cerberus Agent, Cerberus Live Studio self-host media agent**
Document ID: CBA-PMD-005
Version: 1.0
Date: 2026-06-29
Project Manager: Francisco De La Paz

---

## Summary Timeline

| Stage | Start | End | Duration | Hours | Status |
|---|---|---|---|---|---|
| Stage 1: CLI Engine | 2026-06-28 | 2026-06-28 | 1 day | 4.0 hrs | Complete |
| Stage 2: Desktop Wizard | 2026-06-28 | 2026-06-28 | 1 day | 4.5 hrs | Complete |
| Stage 3: Named Token-Mode | 2026-06-29 | 2026-06-29 | 1 day | 2.5 hrs | Complete |
| Stage 4: Distribution + Polish | TBD | TBD | est. | 10 hrs | Not started |
| **Total** | | | | **~21 hrs** | |

Stage 1-3 hours (11.0h) are currently logged in the Cerberus Live Studio WBS (CLS-PMD-003, Stage 3)
and cross-referenced here; re-home at next /audit-project to count once.

---

## Stage 1: CLI Engine, Complete (2026-06-28)

| Date | Milestone |
|---|---|
| 2026-06-28 | Bun engine: folder scan, Range server, cloudflared quick tunnel, register (commit 788d431) |
| 2026-06-28 | End-to-end verified: X:\Music -> 14 tracks -> dossier playback |

---

## Stage 2: Desktop Wizard, Complete (2026-06-28)

| Date | Milestone |
|---|---|
| 2026-06-28 | Framework decision: Tauri v2 over Electron (commit d69ab12) |
| 2026-06-28 | Rust backend (tiny_http + cloudflared + ffprobe + ureq) + React wizard; build clean (commit 2bea671) |

---

## Stage 3: Named Token-Mode, Complete (2026-06-29)

| Date | Milestone |
|---|---|
| 2026-06-29 | Bun engine + Tauri token-mode: cloudflared tunnel run --token, register named (commit 212907b) |
| 2026-06-29 | Live production verification: f-de-la-paz provisioned, 206 stream through gateway, R2 cache warm |
| 2026-06-29 | GitHub repo created (private), all commits pushed |

---

## Stage 4: Distribution + Polish, Not started

| Block | Target Tasks |
|---|---|
| Packaging | NSIS installer (bundle or document cloudflared); auto-start on login |
| GUI | Desktop run-through + screenshots; per-track ordering + featured selection |
| Convenience | In-app agent-key / streaming-token fetch (less copy-paste) |
| Ops | Register in TinkerOps; re-home build hours from CLS-PMD-003; Stage 4 validation |

**Stage 4 Gate: TBD** (scheduled when the platform's public-domain cutover is decided; the agent
is functionally complete for the current artist roster).

---

## Notes

The agent was built in three same-week stages alongside the Cerberus Live Studio platform's media
work, which is why Stage 1-3 hours sit in CLS-PMD-003. It is now functionally complete and live in
production for the first artist. Stage 4 is distribution polish, not core capability, and is paced
behind the platform's go-live priorities.
