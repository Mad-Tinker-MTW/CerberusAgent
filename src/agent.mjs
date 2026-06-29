// Cerberus Live Studio — self-host media agent (engine core).
//
// Serves an artist's local music folder over a local HTTP server (with Range so
// audio scrubs), opens a cloudflared quick tunnel to it, and registers the public
// tunnel URL + track list to the artist's Cerberus dossier. The artist's machine
// is the storage; Cerberus only stores the tunnel URL. Keep this running while you
// want your media live (this is the "must stay on" part).
//
// This is the engine. The branded wizard GUI (Tauri/Electron) wraps this same core.
//
// Usage:  bun run src/agent.mjs [path/to/config.json]
// Config: { "slug", "agentKey", "musicDir", "platformUrl", "port"?, "tunnelToken"? }
//   tunnelToken (from /account "Set up streaming") runs a stable named Cerberus tunnel.
//   Without it the agent falls back to an ephemeral quick tunnel.

import { readdirSync, statSync, existsSync } from "node:fs";
import { resolve, join, extname, basename, sep } from "node:path";
import { spawn } from "node:child_process";

const AUDIO = new Set([".mp3", ".wav", ".flac", ".m4a", ".ogg", ".aac"]);
const MIME = {
  ".mp3": "audio/mpeg", ".wav": "audio/wav", ".flac": "audio/flac",
  ".m4a": "audio/mp4", ".ogg": "audio/ogg", ".aac": "audio/aac",
};

const cfgPath = resolve(process.argv[2] ?? "cerberus-agent.config.json");
if (!existsSync(cfgPath)) {
  console.error(`Config not found: ${cfgPath}`);
  console.error(`Copy cerberus-agent.config.example.json and fill it in.`);
  process.exit(1);
}
const cfg = JSON.parse(await Bun.file(cfgPath).text());
const { slug, agentKey, musicDir, platformUrl } = cfg;
const port = cfg.port ?? 8787;
const root = resolve(musicDir);
for (const [k, v] of Object.entries({ slug, agentKey, musicDir, platformUrl })) {
  if (!v) { console.error(`config.${k} is required`); process.exit(1); }
}
if (!existsSync(root)) { console.error(`musicDir not found: ${root}`); process.exit(1); }

const log = (m) => console.log(`[cerberus-agent] ${m}`);

// --- ffprobe duration (best-effort; "m:ss") -------------------------------
function probeDuration(file) {
  return new Promise((res) => {
    const p = spawn("ffprobe", [
      "-v", "error", "-show_entries", "format=duration",
      "-of", "default=nw=1:nokey=1", file,
    ]);
    let out = "";
    p.stdout.on("data", (d) => (out += d));
    p.on("error", () => res(null));
    p.on("close", () => {
      const s = parseFloat(out.trim());
      if (!isFinite(s)) return res(null);
      res(`${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`);
    });
  });
}

// --- scan folder ----------------------------------------------------------
log(`scanning ${root}`);
const files = readdirSync(root)
  .filter((f) => AUDIO.has(extname(f).toLowerCase()) && statSync(join(root, f)).isFile())
  .sort();
if (files.length === 0) { console.error("No audio files found."); process.exit(1); }

const tracks = [];
for (let i = 0; i < files.length; i++) {
  const f = files[i];
  const duration = await probeDuration(join(root, f));
  tracks.push({ title: basename(f, extname(f)), filename: f, duration, featured: i === 0 });
}
log(`${tracks.length} tracks (featured: ${tracks[0].title})`);

// --- local static server with Range + CORS --------------------------------
const server = Bun.serve({
  port,
  async fetch(req) {
    const name = decodeURIComponent(new URL(req.url).pathname.slice(1));
    if (!name) return new Response("Cerberus agent", { status: 200 });
    const full = resolve(root, name);
    // No path traversal outside the music folder.
    if (!full.startsWith(root + sep) && full !== root) return new Response("forbidden", { status: 403 });
    const file = Bun.file(full);
    if (!(await file.exists())) return new Response("not found", { status: 404 });
    const type = MIME[extname(full).toLowerCase()] ?? "application/octet-stream";
    const size = file.size;
    const cors = { "Access-Control-Allow-Origin": "*", "Accept-Ranges": "bytes", "Content-Type": type };
    const range = req.headers.get("range");
    if (range) {
      const m = /bytes=(\d+)-(\d*)/.exec(range);
      const start = parseInt(m[1], 10);
      const end = m[2] ? parseInt(m[2], 10) : size - 1;
      return new Response(file.slice(start, end + 1), {
        status: 206,
        headers: { ...cors, "Content-Range": `bytes ${start}-${end}/${size}`, "Content-Length": String(end - start + 1) },
      });
    }
    return new Response(file, { headers: { ...cors, "Content-Length": String(size) } });
  },
});
log(`serving on http://localhost:${port}`);

// --- register with the platform -------------------------------------------
// `extra` is either { named: true } (token mode: the platform derives the public host from the
// provisioned media_origin) or { tunnelUrl } (legacy quick tunnel: report the trycloudflare URL).
async function register(extra) {
  try {
    const res = await fetch(`${platformUrl.replace(/\/$/, "")}/api/agent/register`, {
      method: "POST",
      headers: { Authorization: `Bearer ${agentKey}`, "Content-Type": "application/json" },
      body: JSON.stringify({ tracks, ...extra }),
    });
    const data = await res.json().catch(() => ({}));
    if (res.ok) log(`registered: ${data.tracks} tracks live on ${platformUrl}/artist/${slug}`);
    else log(`register failed (${res.status}): ${data.error ?? ""}`);
  } catch (e) {
    log(`register error: ${e}`);
  }
}

// --- cloudflared tunnel ----------------------------------------------------
// Named token mode (cfg.tunnelToken) is the production path: a stable Cerberus-provisioned
// tunnel terminating at the hidden t-<slug>.cerberuslive.studio host. Quick-tunnel mode stays
// as a fallback for unprovisioned/dev use.
let cf;
let registered = false;
if (cfg.tunnelToken) {
  log("starting named cloudflared tunnel (token mode)...");
  cf = spawn("cloudflared", ["tunnel", "run", "--token", cfg.tunnelToken]);
  const onData = (buf) => {
    if (!registered && /Registered tunnel connection/i.test(buf.toString())) {
      registered = true;
      log("tunnel connection registered");
      register({ named: true });
    }
  };
  cf.stdout.on("data", onData);
  cf.stderr.on("data", onData);
  // Fallback: register even if the connection log line never matches.
  setTimeout(() => { if (!registered) { registered = true; register({ named: true }); } }, 10000);
} else {
  log("opening cloudflared quick tunnel...");
  cf = spawn("cloudflared", ["tunnel", "--url", `http://localhost:${port}`]);
  let tunnelUrl = null;
  const scan = (buf) => {
    const m = /https:\/\/[a-z0-9-]+\.trycloudflare\.com/i.exec(buf.toString());
    if (m && !tunnelUrl) { tunnelUrl = m[0]; log(`tunnel: ${tunnelUrl}`); register({ tunnelUrl }); }
  };
  cf.stdout.on("data", scan);
  cf.stderr.on("data", scan); // cloudflared prints the URL to stderr
}

log("agent running. Keep this window open to stay live. Ctrl+C to stop.");
process.on("SIGINT", () => { log("stopping..."); cf.kill(); server.stop(); process.exit(0); });
