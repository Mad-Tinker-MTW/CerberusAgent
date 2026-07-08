import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Editor } from "./Editor";
import "./App.css";

type Track = { title: string; filename: string; duration: string | null; featured: boolean };
type Status = { running: boolean; tunnel_url: string | null; track_count: number; message: string };

type AgentConfig = {
  slug: string;
  agentKey: string;
  tunnelToken: string;
  mediaOrigin: string;
  platformUrl: string;
  musicDir: string;
};

type DeviceStart = {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete: string;
  expires_in: number;
  interval: number;
};

type PollResp =
  | { status: "authorization_pending" | "slow_down" | "access_denied" | "expired_token" }
  | {
      status: "success";
      platformUrl: string;
      slug: string;
      agentKey: string;
      tunnelToken: string;
      mediaOrigin: string;
    }
  | { status: "error"; detail?: string };

const DEFAULT_PLATFORM = "https://cerberuslive.studio";

// True only inside the Tauri app. In a plain browser (dev preview) the Rust `invoke` backend is
// absent, so the UI falls back to sample data — lets the whole frontend be previewed without Tauri.
const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export default function App() {
  // 'loading' -> we're checking APPDATA for stored config
  // 'device'  -> no config, running the device-authorization flow
  // 'setup'   -> config loaded, artist picks folder + goes live
  const [phase, setPhase] = useState<"loading" | "device" | "setup">("loading");
  const [config, setConfig] = useState<AgentConfig | null>(null);
  const [status, setStatus] = useState<Status | null>(null);
  const [error, setError] = useState("");

  // Device-flow state
  const [platformUrl, setPlatformUrl] = useState(DEFAULT_PLATFORM);
  const [grant, setGrant] = useState<DeviceStart | null>(null);
  const [deviceMsg, setDeviceMsg] = useState<string>("");

  // Setup-phase state
  const [musicDir, setMusicDir] = useState<string>("");
  const [tracks, setTracks] = useState<Track[]>([]);
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [showEditor, setShowEditor] = useState(false);

  const pollTimer = useRef<number | null>(null);
  const grantExpiresAt = useRef<number | null>(null);

  const cancelPolling = useCallback(() => {
    if (pollTimer.current) {
      window.clearTimeout(pollTimer.current);
      pollTimer.current = null;
    }
  }, []);

  const startDeviceFlow = useCallback(
    async (platform: string) => {
      cancelPolling();
      setError("");
      setDeviceMsg("Getting a code from Cerberus...");
      try {
        const res = await fetch(`${platform.replace(/\/$/, "")}/api/auth/device`, { method: "POST" });
        if (!res.ok) throw new Error(`start ${res.status}`);
        const g = (await res.json()) as DeviceStart;
        setGrant(g);
        grantExpiresAt.current = Date.now() + g.expires_in * 1000;
        setDeviceMsg("Waiting for you to approve in your browser...");
        schedulePoll(platform, g, g.interval);
      } catch (e) {
        setError(`Could not reach ${platform}: ${e instanceof Error ? e.message : String(e)}`);
        setDeviceMsg("");
      }
    },
    // schedulePoll is defined below via closure; only platform + grant matter.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [cancelPolling]
  );

  const schedulePoll = useCallback(
    (platform: string, g: DeviceStart, intervalSec: number) => {
      cancelPolling();
      pollTimer.current = window.setTimeout(async () => {
        if (grantExpiresAt.current && Date.now() > grantExpiresAt.current) {
          setDeviceMsg("Code expired. Request a new one.");
          setGrant(null);
          return;
        }
        try {
          const res = await fetch(`${platform.replace(/\/$/, "")}/api/auth/device/token`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ device_code: g.device_code }),
          });
          const data = (await res.json()) as PollResp;
          if (data.status === "success") {
            const cfg: AgentConfig = {
              slug: data.slug,
              agentKey: data.agentKey,
              tunnelToken: data.tunnelToken,
              mediaOrigin: data.mediaOrigin,
              platformUrl: data.platformUrl,
              musicDir: "",
            };
            await invoke("save_config", { config: cfg });
            setConfig(cfg);
            setPhase("setup");
            return;
          }
          if (data.status === "access_denied") {
            setDeviceMsg("Request denied in the browser. Start over to try again.");
            setGrant(null);
            return;
          }
          if (data.status === "expired_token") {
            setDeviceMsg("Code expired. Request a new one.");
            setGrant(null);
            return;
          }
          if (data.status === "slow_down") {
            schedulePoll(platform, g, intervalSec + 5);
            return;
          }
          // authorization_pending or unexpected -> keep waiting
          schedulePoll(platform, g, intervalSec);
        } catch {
          schedulePoll(platform, g, intervalSec);
        }
      }, intervalSec * 1000);
    },
    [cancelPolling]
  );

  // Boot: load config if it exists, else start device flow.
  useEffect(() => {
    (async () => {
      // Browser dev-preview: no Tauri backend, so land in setup with a sample config.
      if (!IN_TAURI) {
        const mock: AgentConfig = { slug: "mad-tinker", agentKey: "", tunnelToken: "", mediaOrigin: "", platformUrl: DEFAULT_PLATFORM, musicDir: "X:\\Music" };
        setConfig(mock);
        setMusicDir(mock.musicDir);
        setPhase("setup");
        return;
      }
      try {
        const cfg = (await invoke("load_config")) as AgentConfig | null;
        if (cfg && cfg.slug && cfg.agentKey) {
          setConfig(cfg);
          setMusicDir(cfg.musicDir || "");
          setPlatformUrl(cfg.platformUrl || DEFAULT_PLATFORM);
          setPhase("setup");
          const s = await invoke<Status>("agent_status").catch(() => null);
          if (s) setStatus(s);
        } else {
          setPhase("device");
          await startDeviceFlow(DEFAULT_PLATFORM);
        }
      } catch (e) {
        setPhase("device");
        setError(String(e));
      }
    })();
    return () => cancelPolling();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function pickFolder() {
    const dir = await openDialog({ directory: true, title: "Pick your music folder" });
    if (typeof dir === "string") {
      setMusicDir(dir);
      await scan(dir);
      if (config) await invoke("save_config", { config: { ...config, musicDir: dir } });
    }
  }

  async function scan(dir: string) {
    setError("");
    try {
      setTracks(await invoke<Track[]>("scan_folder", { path: dir }));
    } catch (e) {
      setError(String(e));
      setTracks([]);
    }
  }

  async function goLive() {
    if (!config) return;
    if (!musicDir.trim()) return setError("Pick your music folder.");
    if (!ack) return setError("Confirm you'll keep the agent running.");
    setBusy(true);
    setError("");
    try {
      const s = await invoke<Status>("start_agent", {
        musicDir,
        agentKey: config.agentKey,
        platformUrl: config.platformUrl,
        port: 8787,
        tunnelToken: config.tunnelToken || null,
      });
      setStatus(s);
    } catch (e) {
      setError(String(e));
    }
    setBusy(false);
  }

  async function stop() {
    setBusy(true);
    setStatus(await invoke<Status>("stop_agent"));
    setBusy(false);
  }

  async function signOut() {
    cancelPolling();
    await invoke("clear_config").catch(() => {});
    setConfig(null);
    setStatus(null);
    setMusicDir("");
    setTracks([]);
    setAck(false);
    setPhase("device");
    await startDeviceFlow(platformUrl);
  }

  const live = status?.running;

  if (showEditor && config) {
    return <Editor musicDir={musicDir} artistName={config.slug} onExit={() => setShowEditor(false)} />;
  }

  return (
    <main className="wrap">
      <h1 className="brand">
        CERBERUS <span>AGENT</span>
      </h1>

      {phase === "loading" && <p className="sub">Loading...</p>}

      {phase === "device" && (
        <>
          <p className="sub">
            Enter this code at{" "}
            <code>{platformUrl.replace(/^https?:\/\//, "")}/device</code> to link this agent
            to your Cerberus account.
          </p>

          {grant ? (
            <section className="card live">
              <div className="dot-row"><span className="dot on" /> {deviceMsg}</div>
              <p style={{ fontSize: 28, fontWeight: 700, letterSpacing: 4, margin: "12px 0", textAlign: "center", fontFamily: "monospace" }}>
                {grant.user_code}
              </p>
              <div className="fld" style={{ marginTop: 4 }}>
                <button
                  className="btn go"
                  type="button"
                  onClick={() => openUrl(grant.verification_uri_complete).catch(() => {})}
                >
                  Open verification page
                </button>
              </div>
              <p className="msg" style={{ marginTop: 12, textAlign: "center" }}>
                Or type the code at{" "}
                <code>{grant.verification_uri.replace(/^https?:\/\//, "")}</code> on any device.
              </p>
            </section>
          ) : (
            <>
              <p className="err">{deviceMsg || error}</p>
              <button className="btn go" onClick={() => startDeviceFlow(platformUrl)}>
                Get a new code
              </button>
            </>
          )}

          <details style={{ marginTop: 12 }}>
            <summary style={{ cursor: "pointer", color: "var(--muted)", fontSize: 12 }}>
              Advanced: change platform URL
            </summary>
            <div className="fld" style={{ marginTop: 8 }}>
              <input value={platformUrl} onChange={(e) => setPlatformUrl(e.target.value)} />
              <button className="btn ghost" onClick={() => startDeviceFlow(platformUrl)}>
                Restart with this URL
              </button>
            </div>
          </details>
        </>
      )}

      {phase === "setup" && config && (
        <>
          <p className="sub">
            Signed in as <b>{config.slug}</b>. Your media stays on this machine and streams
            through Cerberus while this agent is running.
          </p>

          {live ? (
            <section className="card live">
              <div className="dot-row"><span className="dot on" /> Live</div>
              <p className="msg">{status?.message}</p>
              {status?.tunnel_url && (
                <p className="tunnel">Tunnel: <code>{status.tunnel_url}</code></p>
              )}
              <button className="btn stop" onClick={stop} disabled={busy}>Stop serving</button>
            </section>
          ) : (
            <>
              <label className="fld">
                <span>Music folder</span>
                <div className="row">
                  <input value={musicDir} readOnly placeholder="none selected" />
                  <button className="btn ghost" onClick={pickFolder}>Browse</button>
                </div>
              </label>

              {tracks.length > 0 && (
                <div className="card">
                  <div className="card-h">
                    {tracks.length} tracks <span className="muted">· featured: {tracks[0].title}</span>
                  </div>
                  <ul className="tracks">
                    {tracks.map((t, i) => (
                      <li key={i}><span>{t.title}</span><span className="muted">{t.duration ?? ""}</span></li>
                    ))}
                  </ul>
                </div>
              )}

              <label className="ack">
                <input type="checkbox" checked={ack} onChange={(e) => setAck(e.target.checked)} />
                <span>I understand my media is live only while this app and my computer stay on.</span>
              </label>

              <button className="btn go" onClick={goLive} disabled={busy}>{busy ? "Starting..." : "Go live"}</button>
            </>
          )}

          <button
            className="btn ghost"
            style={{ marginTop: 8 }}
            onClick={() => setShowEditor(true)}
            disabled={busy}
          >
            Organize library
          </button>

          <button
            className="btn ghost"
            style={{ marginTop: 16, alignSelf: "flex-start" }}
            onClick={signOut}
            disabled={busy}
          >
            Sign out
          </button>
        </>
      )}

      {error && <p className="err">{error}</p>}
    </main>
  );
}
