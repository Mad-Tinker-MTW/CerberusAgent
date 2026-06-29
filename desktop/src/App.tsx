import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";

type Track = { title: string; filename: string; duration: string | null; featured: boolean };
type Status = { running: boolean; tunnel_url: string | null; track_count: number; message: string };

const DEFAULT_PLATFORM = "https://cerberuslive-web.frankydlp.workers.dev";

const load = (k: string, d: string) => localStorage.getItem(k) ?? d;

export default function App() {
  const [platformUrl, setPlatformUrl] = useState(() => load("cb_platform", DEFAULT_PLATFORM));
  const [agentKey, setAgentKey] = useState(() => load("cb_key", ""));
  const [musicDir, setMusicDir] = useState(() => load("cb_dir", ""));
  const [tunnelToken, setTunnelToken] = useState(() => load("cb_token", ""));
  const [tracks, setTracks] = useState<Track[]>([]);
  const [ack, setAck] = useState(false);
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    localStorage.setItem("cb_platform", platformUrl);
    localStorage.setItem("cb_key", agentKey);
    localStorage.setItem("cb_dir", musicDir);
    localStorage.setItem("cb_token", tunnelToken);
  }, [platformUrl, agentKey, musicDir, tunnelToken]);

  useEffect(() => {
    invoke<Status>("agent_status").then(setStatus).catch(() => {});
  }, []);

  async function pickFolder() {
    const dir = await open({ directory: true, title: "Pick your music folder" });
    if (typeof dir === "string") {
      setMusicDir(dir);
      await scan(dir);
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

  async function start() {
    if (!agentKey.trim()) return setError("Paste your agent key from your Cerberus account.");
    if (!musicDir.trim()) return setError("Pick your music folder.");
    if (!ack) return setError("Confirm you'll keep the agent running.");
    setBusy(true);
    setError("");
    try {
      const s = await invoke<Status>("start_agent", {
        musicDir,
        agentKey: agentKey.trim(),
        platformUrl: platformUrl.trim(),
        port: 8787,
        tunnelToken: tunnelToken.trim() || null,
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

  const live = status?.running;

  return (
    <main className="wrap">
      <h1 className="brand">CERBERUS <span>AGENT</span></h1>
      <p className="sub">
        Your media stays on this machine and streams through Cerberus. Keep this running
        while you want your tracks live.
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
            <span>Cerberus platform</span>
            <input value={platformUrl} onChange={(e) => setPlatformUrl(e.target.value)} />
          </label>
          <label className="fld">
            <span>Agent key <em>(from your Cerberus account)</em></span>
            <input value={agentKey} onChange={(e) => setAgentKey(e.target.value)} placeholder="paste your key" />
          </label>
          <label className="fld">
            <span>Streaming token <em>(from "Set up streaming"; optional)</em></span>
            <input value={tunnelToken} onChange={(e) => setTunnelToken(e.target.value)} placeholder="paste your streaming token" />
          </label>
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

          <button className="btn go" onClick={start} disabled={busy}>{busy ? "Starting..." : "Go live"}</button>
        </>
      )}

      {error && <p className="err">{error}</p>}
    </main>
  );
}
