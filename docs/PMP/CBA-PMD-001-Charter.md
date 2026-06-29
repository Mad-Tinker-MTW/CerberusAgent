# Project Charter
**Cerberus Agent, Cerberus Live Studio self-host media agent**
Document ID: CBA-PMD-001
Version: 1.0
Date: 2026-06-29
Project Manager: Francisco De La Paz

---

## Project Overview

Cerberus Agent is the self-host media tool for Cerberus Live Studio. It runs on an artist's own
machine, serves their local music folder over a small HTTP server with byte-range support, and runs
a Cloudflare named tunnel that the Cerberus platform provisioned for them. The artist's machine is
the storage; Cerberus stores only the tunnel binding and the track list. The platform's media
gateway (media.cerberuslive.studio) streams from the agent and caches in R2, so hot tracks keep
playing even when the artist's machine is offline.

It ships in two forms: a branded Tauri v2 desktop wizard (the product artists download) and a Bun
CLI engine (the reference implementation and a headless fallback). Both register tracks to the same
platform endpoint.

---

## Business Need

Cerberus Live Studio is first-party media only: artists keep ownership of their files instead of
uploading to a third party or linking off-platform. That requires a tool the artist installs once,
that needs no Cloudflare account or networking knowledge, and that streams reliably through Cerberus.
Cerberus Agent is that tool. Without it, the platform has no media.

---

## Objectives

1. Serve an artist's local audio with range support and register it to their dossier, complete (2026-06-28)
2. Ship a branded one-screen desktop wizard (Tauri v2) so a non-technical artist can go live, built (2026-06-28)
3. Run a stable named tunnel from a Cerberus-provisioned token (no per-artist Cloudflare account), complete (2026-06-29)
4. Verify the full path live in production (provision, stream, cache), complete (2026-06-29)
5. Package for distribution (NSIS installer) and finish per-track controls, pending

---

## Scope

### In Scope
- Local static media server with Range + CORS (.mp3/.wav/.flac/.m4a/.ogg/.aac)
- Folder scan + ffprobe durations
- Named cloudflared tunnel via a platform-issued run token (token mode)
- Quick-tunnel fallback for zero-config / unprovisioned use
- Track registration to the Cerberus platform (`/api/agent/register`)
- Tauri v2 desktop wizard (folder picker, agent key + streaming token, must-stay-running disclaimer, live status)
- Bun CLI engine as reference + headless fallback

### Out of Scope
- Media transcoding or editing (the agent serves files as-is)
- The R2 read-through cache and public routing (that is the platform's media gateway, not the agent)
- Per-artist Cloudflare accounts (rejected; Cerberus provisions the tunnel centrally)
- Mobile or web build (this is a desktop background utility)

---

## Deliverables

| Deliverable | Target Date |
|---|---|
| Stage 1: CLI engine (serve + tunnel + register) | Complete (2026-06-28) |
| Stage 2: Tauri v2 desktop wizard | Complete (2026-06-28) |
| Stage 3: Named token-mode (stable tunnel) | Complete (2026-06-29) |
| Stage 4: Distribution + polish (installer, GUI run-through, per-track controls) | Pending |

---

## Milestones

| Milestone | Date |
|---|---|
| CLI engine verified (X:\Music -> 14 tracks -> dossier playback) | 2026-06-28 |
| Tauri v2 framework decision | 2026-06-28 |
| Desktop wizard built (cargo + frontend clean) | 2026-06-28 |
| Named token-mode (Bun + Tauri) | 2026-06-29 |
| Live production verification (provision + 206 stream + R2 cache) | 2026-06-29 |
| GitHub repo created (private) | 2026-06-29 |
| NSIS installer + GUI run-through | TBD Stage 4 |

---

## Budget Summary

| Category | Amount |
|---|---|
| Labor, Stage 1 CLI engine (4 hrs at $85/hr) | $340 |
| Labor, Stage 2 desktop wizard (4.5 hrs at $85/hr) | $382.50 |
| Labor, Stage 3 named token-mode (2.5 hrs at $85/hr) | $212.50 |
| Labor, Stage 4 distribution + polish (est. 10 hrs at $85/hr) | $850 |
| Tools and hosting | $0 (uses the Cerberus Cloudflare account) |
| **Total** | **~$1,785** |

Note: Stage 1-3 build hours (commits 788d431 / 2bea671 / 212907b) are currently logged in the
Cerberus Live Studio WBS (CLS-PMD-003, Stage 3), as the agent was built as a companion of the
platform. They are cross-referenced here and should be re-homed to this WBS at the next
/audit-project so hours are not double-counted.

---

## Stakeholders

| Name | Role | Interest |
|---|---|---|
| Francisco De La Paz | Project Sponsor, Project Manager, Lead Developer, End User, QA, Deployment Engineer | Full ownership |
| Cerberus Live Studio | Parent platform (consumes the agent) | First-party media depends on it |
| Mad Tinker's Workshop | Validation environment | First artist (mad-tinker / f-de-la-paz) |
| 4Kings Enterprises | Parent organization | Product portfolio value |
| Underground artists | End users | Keep ownership of their media |

---

## Risks (Summary)

| Risk | Level |
|---|---|
| Artist must keep the agent running for live (uncached) media | Medium |
| cloudflared not on PATH / install friction for non-technical artists | Medium |
| Streaming tunnel token is a credential handed to the artist's machine | Medium |
| Desktop GUI not yet run through on a real install | Medium |

---

## Authorization

Project authorized under 4Kings Enterprises as a component of Cerberus Live Studio.
Project Manager: Francisco De La Paz
Date: 2026-06-29
