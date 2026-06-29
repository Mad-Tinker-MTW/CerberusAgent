# Cerberus Agent, Vision

## The Problem
Cerberus Live Studio is first-party media only: artists keep ownership of their files instead of
uploading to a third party or pointing fans off-platform to Spotify or SoundCloud. But "keep your
files" usually means "be your own sysadmin." Artists are not networking people. They will not stand
up a server, configure a tunnel, or manage a Cloudflare account. Without a tool that makes self-hosting
a one-screen action, the first-party promise is empty.

## The Product
Cerberus Agent is that tool: an artist downloads it, signs in, points it at a music folder, and clicks
go. Their machine becomes the storage; Cerberus handles the public side. The artist never sees
Cloudflare. Behind the one screen, the agent serves their folder with range support and runs a
named tunnel that Cerberus provisioned for them, then tells the platform what tracks are live.

The platform's media gateway caches plays in R2, so once a track is warm it keeps streaming even when
the artist's machine is off. The artist gets ownership without uptime anxiety.

## Where It Goes
- **Now:** Tauri v2 wizard + Bun CLI engine, named token-mode, live in production for the first artist.
- **Next:** an installer that bundles cloudflared so the artist installs nothing else; per-track
  ordering and featured selection in the GUI; auto-start so "keep it running" is automatic.
- **Ceiling:** the agent disappears into the background. The artist installs once, never thinks about
  it again, and their catalog is always live through Cerberus. Optionally pairs with a platform-side
  admin-hosted R2 tier for artists who would rather not run anything at all.

## Principles
- **The artist owns the files.** Cerberus stores the tunnel binding and track list, never the media.
- **No Cloudflare knowledge required.** The platform provisions the tunnel; the agent just runs the token.
- **Light by default.** A background utility should be a few MB, not a bundled browser (the reason it is
  Tauri, not Electron).
- **The CLI is the source of truth.** The desktop app reimplements the proven engine; the CLI stays as
  the verified fallback.

## Emergent Scope (chaos dancing)
The agent was planned around a quick tunnel (zero-config, random URL each run) with a named tunnel as
"a later upgrade." Reality forced the upgrade early: account-less quick tunnels did not serve reliably,
and a three-level media host had no TLS cert. That pushed the whole design to named tunnels provisioned
centrally by Cerberus plus a gateway worker with an R2 cache. The agent's job narrowed and hardened:
serve locally, run the token, register. The clever part (hidden two-level origins, caching, routing)
moved to the platform, which is where it belongs.

## Origin
Born from Cerberus Live Studio's first-party media requirement. It only exists to feed the platform,
and its contracts (register, provision, token handshake) are defined by the platform.
