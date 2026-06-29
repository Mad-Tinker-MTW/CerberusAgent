# Cerberus Agent, Known Issues

---

## Open

**Desktop GUI not yet run through**
The Tauri app compiles clean (cargo + frontend tsc/vite) but the packaged desktop window has not been
run through on a real install. The CLI engine is the verified path until then. Fix: Stage 4 (1.4.2),
GUI run-through + screenshots.

**cloudflared must be on PATH**
The agent shells out to `cloudflared`; a non-technical artist may not have it installed. Fix: Stage 4
installer bundles cloudflared as a sidecar so the artist installs nothing extra.

**Must stay running for uncached media**
Media that has never been played is not yet in the platform's R2 cache, so it needs the agent live.
Mitigated by the gateway's read-through cache (hot tracks survive offline) and a clear disclaimer.
A platform-side admin-hosted R2 tier is the longer-term answer for always-on artists.

---

## Closed

- Account-less quick tunnels do not serve reliably / three-level media host had no TLS cert (2026-06-29):
  resolved at the platform level by moving to Cerberus-provisioned named tunnels terminating at hidden
  two-level origins (`t-<slug>.cerberuslive.studio`) behind a gateway worker with an R2 cache. The agent
  gained named token-mode (`cloudflared tunnel run --token`) to match. Verified live for f-de-la-paz.
