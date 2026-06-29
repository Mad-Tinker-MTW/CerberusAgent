# Risk Register
**Cerberus Agent, Cerberus Live Studio self-host media agent**
Document ID: CBA-PMD-003
Version: 1.0
Date: 2026-06-29

---

## Risk Scale
**Probability:** Low (1) / Medium (2) / High (3)
**Impact:** Low (1) / Medium (2) / High (3)
**Score:** P x I

---

## Risk Log

### R-001: Artist Machine Offline for Uncached Media
**Probability:** High (3) | **Impact:** Medium (2) | **Score:** 6

The agent must be running for media to stream the first time. If the artist's machine is off and a
track has never been played, the gateway returns 502.

**Mitigation:** The platform gateway R2-caches on first play, so hot tracks survive an offline
machine. The wizard shows a clear must-stay-running disclaimer. The dossier degrades gracefully
(no broken player) when nothing is cached.

**Contingency:** An admin-hosted always-on R2 tier (platform roadmap) for artists who cannot keep a
machine running.

---

### R-002: cloudflared Install Friction
**Probability:** Medium (2) | **Impact:** Medium (2) | **Score:** 4

The agent shells out to `cloudflared`, which must be on PATH. A non-technical artist may not have it.

**Mitigation:** Document the prerequisite in README. The Tauri build can bundle cloudflared as a
sidecar in a later stage so the artist installs nothing extra.

**Contingency:** The installer (Stage 4) ships cloudflared alongside the app.

---

### R-003: Streaming Token Handling
**Probability:** Medium (2) | **Impact:** Medium (2) | **Score:** 4

Token-mode runs a Cerberus-provisioned tunnel run token on the artist's machine. It is a credential;
if leaked, it lets someone run that specific tunnel.

**Mitigation:** The real config (`cerberus-agent.config.json`) is gitignored. The desktop app stores
it in localStorage on the artist's machine, never committed. Tokens are scoped to one tunnel and
revocable by deleting/rotating the tunnel from the Cerberus account.

**Contingency:** Add token rotation to /account so a compromised token can be replaced without a new
dossier.

---

### R-004: Desktop GUI Not Yet Run Through
**Probability:** Medium (2) | **Impact:** Medium (2) | **Score:** 4

The Tauri app compiles clean (cargo + frontend) and the CLI engine is verified end to end, but the
packaged desktop window has not been run through on a real install.

**Mitigation:** The Rust backend reimplements the proven CLI engine logic. CLI is the verified
fallback. GUI run-through is an explicit Stage 4 task (1.4.2).

**Contingency:** Ship the CLI engine as the supported path until the GUI is verified.

---

## Risk Summary

| ID | Risk | Score | Status |
|---|---|---|---|
| R-001 | Artist machine offline for uncached media | 6 | Active |
| R-002 | cloudflared install friction | 4 | Monitored |
| R-003 | Streaming token handling | 4 | Monitored |
| R-004 | Desktop GUI not yet run through | 4 | Active |

---

# Stakeholder Register

---

## Stakeholders

| ID | Stakeholder | Role | Influence | Interest | Strategy |
|---|---|---|---|---|---|
| STK-001 | Francisco De La Paz | Owner, Developer, End User | High | High | Lead |
| STK-002 | Cerberus Live Studio | Parent platform | High | High | Collaborate |
| STK-003 | Mad Tinker's Workshop | Validation environment | High | High | Collaborate |
| STK-004 | 4Kings Enterprises | Parent org | High | Low | Inform |
| STK-005 | Underground artists | End users | Low | High | Inform |

---

### STK-002: Cerberus Live Studio
The platform provisions each agent's named tunnel (lib/cf-tunnel.ts, /api/agent/provision), receives
the agent's track registration (/api/agent/register), and serves media through its gateway worker
(media.cerberuslive.studio + R2 cache). The agent and platform must agree on the register contract
and the token-mode handshake. Platform decisions (host pattern, gateway behavior) directly shape the
agent.

**Communication:** Coordinated within the Cerberus Live Studio build; the agent's register contract
is defined by the platform's `/api/agent/register` route.

### STK-005: Underground Artists
The end users who download the agent to put their media live while keeping their files. They are
non-technical; the wizard must require no Cloudflare knowledge and minimal setup.

**Communication:** Through the /account "Set up streaming" flow and the wizard's on-screen guidance.
