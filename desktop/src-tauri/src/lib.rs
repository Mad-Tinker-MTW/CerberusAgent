// Cerberus Agent — Tauri backend.
// Serves the artist's local music/video folder (Range + CORS, nested paths), opens a cloudflared
// tunnel, and registers the scanned catalog to the artist's dossier. The machine is the storage;
// Cerberus only stores the tunnel marker.
//
// L-048 scan model (recursive + persona-aware, ported from src/agent.mjs):
//   musicDir/<file>                      -> direct single (no persona, no release)
//   musicDir/<persona>/<file>            -> persona single
//   musicDir/<persona>/<release>/<file>  -> release under persona
// Embedded tags (ffprobe) win over folder names: album_artist/artist -> persona, album -> release,
// track -> track no, composer -> creator, title -> title. Untagged originals fall back to folders.
// A recursive file watcher re-scans + re-registers on change (debounced).

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use serde::Serialize;
use serde_json::json;
use tauri::State;

const AUDIO_EXTS: &[&str] = &["mp3", "wav", "flac", "m4a", "ogg", "aac"];
const VIDEO_EXTS: &[&str] = &["mp4", "webm", "mov", "m4v", "mkv"];

#[derive(Serialize, Clone)]
struct Track {
    title: String,
    filename: String, // relative URL path with forward slashes (nested)
    duration: Option<String>,
    persona: Option<String>,
    release: Option<String>,
    #[serde(rename = "mediaKind")]
    media_kind: String, // "audio" | "video"
    #[serde(rename = "trackNo")]
    track_no: Option<u32>,
    composer: Option<String>,
    #[serde(rename = "releaseKind")]
    release_kind: Option<String>,
    featured: bool,
}

#[derive(Serialize, Clone, Default)]
struct Status {
    running: bool,
    tunnel_url: Option<String>,
    track_count: usize,
    message: String,
}

#[derive(Default)]
struct AgentState {
    running: Arc<AtomicBool>,
    cf: Mutex<Option<Child>>,
    status: Mutex<Status>,
}

fn ext_of(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default()
}

fn is_media(ext: &str) -> bool {
    AUDIO_EXTS.contains(&ext) || VIDEO_EXTS.contains(&ext)
}

fn mime_for(path: &Path) -> &'static str {
    match ext_of(path).as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" => "audio/mp4",
        "ogg" => "audio/ogg",
        "aac" => "audio/aac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "m4v" => "video/x-m4v",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    }
}

/// ffprobe duration + tags (best-effort). Returns (duration "m:ss", lowercased tag map).
fn probe_meta(path: &Path) -> (Option<String>, HashMap<String, String>) {
    let mut tags = HashMap::new();
    let mut duration = None;
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:format_tags=artist,album,album_artist,track,composer,title,genre",
            "-of",
            "json",
        ])
        .arg(path)
        .output();
    if let Ok(o) = out {
        if let Ok(j) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
            let fmt = &j["format"];
            if let Some(s) = fmt["duration"].as_str().and_then(|s| s.parse::<f64>().ok()) {
                let m = (s / 60.0).floor() as u64;
                let sec = (s % 60.0).floor() as u64;
                duration = Some(format!("{}:{:02}", m, sec));
            }
            if let Some(obj) = fmt["tags"].as_object() {
                for (k, v) in obj {
                    if let Some(vs) = v.as_str() {
                        tags.insert(k.to_lowercase(), vs.to_string());
                    }
                }
            }
        }
    }
    (duration, tags)
}

/// Recursively collect media files under `dir`.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.is_file() && is_media(&ext_of(&p)) {
                out.push(p);
            }
        }
    }
}

/// First trimmed non-empty candidate (tags win over folder names; caller orders them).
fn first_nonempty(cands: &[Option<&String>]) -> Option<String> {
    for c in cands {
        if let Some(s) = c {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

fn parse_track_no(raw: Option<&String>) -> Option<u32> {
    raw.and_then(|s| s.split('/').next()).and_then(|s| s.trim().parse::<u32>().ok())
}

/// Build the catalog from the current folder state. Mirrors src/agent.mjs buildTracks().
fn build_tracks(root: &Path) -> Vec<Track> {
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();

    let mut base: Vec<Track> = Vec::new();
    for full in &files {
        let ext = ext_of(full);
        let rel = full.strip_prefix(root).unwrap_or(full);
        // forward-slash relative URL, e.g. "persona/release/song.mp3"
        let rel_url = rel
            .iter()
            .map(|c| c.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/");
        // folders above the file: parts[0] = persona, parts[1] = release
        let parts: Vec<String> = rel
            .parent()
            .map(|p| p.iter().map(|c| c.to_string_lossy().to_string()).collect())
            .unwrap_or_default();

        let (duration, tags) = probe_meta(full);
        let persona = first_nonempty(&[tags.get("album_artist"), tags.get("artist"), parts.first()]);
        let release = first_nonempty(&[tags.get("album"), parts.get(1)]);
        let title = first_nonempty(&[tags.get("title")])
            .unwrap_or_else(|| full.file_stem().and_then(|s| s.to_str()).unwrap_or("track").to_string());
        let composer = first_nonempty(&[tags.get("composer")]);
        let media_kind = if VIDEO_EXTS.contains(&ext.as_str()) { "video" } else { "audio" }.to_string();

        base.push(Track {
            title,
            filename: rel_url,
            duration,
            persona,
            release,
            media_kind,
            track_no: parse_track_no(tags.get("track")),
            composer,
            release_kind: None,
            featured: false,
        });
    }

    // Release-kind heuristic from track count per (persona|release).
    let mut counts: HashMap<String, u32> = HashMap::new();
    for t in &base {
        if let Some(rel) = &t.release {
            let key = format!("{} {}", t.persona.clone().unwrap_or_default(), rel);
            *counts.entry(key).or_insert(0) += 1;
        }
    }
    let mut featured_set = false;
    for t in &mut base {
        if let Some(rel) = &t.release {
            let key = format!("{} {}", t.persona.clone().unwrap_or_default(), rel);
            let n = *counts.get(&key).unwrap_or(&1);
            t.release_kind = Some(if n == 1 { "single" } else if n <= 5 { "ep" } else { "album" }.to_string());
        }
        if !featured_set && t.media_kind == "audio" {
            t.featured = true;
            featured_set = true;
        }
    }
    base
}

#[tauri::command]
fn scan_folder(path: String) -> Result<Vec<Track>, String> {
    let root = PathBuf::from(path);
    if !root.is_dir() {
        return Err("Not a folder".into());
    }
    let tracks = build_tracks(&root);
    if tracks.is_empty() {
        return Err("No audio/video files in that folder".into());
    }
    Ok(tracks)
}

fn parse_range(h: &str) -> Option<(u64, u64)> {
    let s = h.strip_prefix("bytes=")?;
    let mut parts = s.split('-');
    let start: u64 = parts.next()?.trim().parse().ok()?;
    let end = parts.next().and_then(|e| e.trim().parse::<u64>().ok()).unwrap_or(u64::MAX);
    Some((start, end))
}

fn extract_tunnel(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest.find(".trycloudflare.com")? + ".trycloudflare.com".len();
    Some(rest[..end].to_string())
}

/// POST the catalog to the platform's register endpoint. `extra` carries { named: true } (token
/// mode) or { tunnelUrl } (quick tunnel).
fn post_register(
    platform_url: &str,
    agent_key: &str,
    tracks: &[Track],
    extra: serde_json::Value,
) -> Result<usize, String> {
    let url = format!("{}/api/agent/register", platform_url.trim_end_matches('/'));
    let mut body = json!({ "tracks": tracks });
    if let (Some(map), Some(extra_map)) = (body.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_map {
            map.insert(k.clone(), v.clone());
        }
    }
    ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", agent_key))
        .send_json(body)
        .map(|_| tracks.len())
        .map_err(|e| e.to_string())
}

// Local static server (Range + CORS, nested paths). Runs until `running` flips false.
fn run_server(root: PathBuf, port: u16, running: Arc<AtomicBool>) -> Result<(), String> {
    let server = tiny_http::Server::http(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    while running.load(Ordering::Relaxed) {
        let req = match server.recv_timeout(Duration::from_millis(400)) {
            Ok(Some(r)) => r,
            Ok(None) => continue,
            Err(_) => break,
        };
        let cors = tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap();
        let name = urlencoding::decode(req.url().trim_start_matches('/'))
            .map(|c| c.into_owned())
            .unwrap_or_default();
        if name.is_empty() {
            let _ = req.respond(tiny_http::Response::from_string("Cerberus agent").with_header(cors));
            continue;
        }
        let full = root.join(&name);
        if !(full.starts_with(&root) && full.is_file()) {
            let _ = req.respond(tiny_http::Response::from_string("not found").with_status_code(404));
            continue;
        }
        let ctype = tiny_http::Header::from_bytes(&b"Content-Type"[..], mime_for(&full).as_bytes()).unwrap();
        let ranges = tiny_http::Header::from_bytes(&b"Accept-Ranges"[..], &b"bytes"[..]).unwrap();
        let size = full.metadata().map(|m| m.len()).unwrap_or(0);
        let range_hdr = req.headers().iter().find(|h| h.field.equiv("Range")).map(|h| h.value.as_str().to_string());

        let mut file = match File::open(&full) {
            Ok(f) => f,
            Err(_) => {
                let _ = req.respond(tiny_http::Response::from_string("error").with_status_code(500));
                continue;
            }
        };

        if let Some((s, e)) = range_hdr.as_deref().and_then(parse_range).filter(|_| size > 0) {
            let start = s;
            let end = e.min(size - 1);
            if start <= end {
                let len = end - start + 1;
                let mut buf = vec![0u8; len as usize];
                if file.seek(SeekFrom::Start(start)).is_ok() && file.read_exact(&mut buf).is_ok() {
                    let cr = tiny_http::Header::from_bytes(
                        &b"Content-Range"[..],
                        format!("bytes {}-{}/{}", start, end, size).as_bytes(),
                    )
                    .unwrap();
                    let resp = tiny_http::Response::from_data(buf)
                        .with_status_code(206)
                        .with_header(cors)
                        .with_header(ctype)
                        .with_header(ranges)
                        .with_header(cr);
                    let _ = req.respond(resp);
                    continue;
                }
            }
        }
        let resp = tiny_http::Response::from_file(file).with_header(cors).with_header(ctype).with_header(ranges);
        let _ = req.respond(resp);
    }
    Ok(())
}

/// Watch the library recursively; on change, debounce ~2s then re-scan + re-register. Exits when
/// `running` flips false. Mirrors the agent.mjs watcher.
fn run_watcher(
    root: PathBuf,
    platform_url: String,
    agent_key: String,
    extra: serde_json::Value,
    running: Arc<AtomicBool>,
) {
    let (tx, rx) = mpsc::channel();
    let mut watcher = match notify::recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => w,
        Err(_) => return,
    };
    if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
        return;
    }
    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(_) => {
                // Debounce: let a burst of edits settle, drain, then sync once.
                thread::sleep(Duration::from_secs(2));
                while rx.try_recv().is_ok() {}
                if !running.load(Ordering::Relaxed) {
                    break;
                }
                let tracks = build_tracks(&root);
                if !tracks.is_empty() {
                    let _ = post_register(&platform_url, &agent_key, &tracks, extra.clone());
                }
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn spawn_url_reader<R: Read + Send + 'static>(reader: R, tx: mpsc::Sender<String>) {
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            if let Some(u) = extract_tunnel(&line) {
                let _ = tx.send(u);
                break;
            }
        }
    });
}

fn stop_internal(state: &State<'_, AgentState>) {
    state.running.store(false, Ordering::Relaxed);
    if let Some(mut child) = state.cf.lock().unwrap().take() {
        let _ = child.kill();
    }
    *state.status.lock().unwrap() = Status::default();
}

#[tauri::command]
fn start_agent(
    music_dir: String,
    agent_key: String,
    platform_url: String,
    port: Option<u16>,
    tunnel_token: Option<String>,
    state: State<'_, AgentState>,
) -> Result<Status, String> {
    let port = port.unwrap_or(8787);
    let root = PathBuf::from(&music_dir);
    let tracks = build_tracks(&root);
    if tracks.is_empty() {
        return Err("No audio/video files in that folder".into());
    }

    stop_internal(&state);
    state.running.store(true, Ordering::Relaxed);

    {
        let running = state.running.clone();
        let root = root.clone();
        thread::spawn(move || {
            if let Err(e) = run_server(root, port, running) {
                eprintln!("server error: {e}");
            }
        });
    }

    // Named token mode (provisioned streaming) is the production path; quick tunnel is the
    // unprovisioned fallback. In named mode the platform derives the public host from the stored
    // media_origin, so there is no trycloudflare URL to parse.
    let named = tunnel_token.as_deref().map(|t| !t.is_empty()).unwrap_or(false);

    let (child, extra, tunnel_marker) = if named {
        let token = tunnel_token.as_deref().unwrap();
        let child = Command::new("cloudflared")
            .args(["tunnel", "run", "--token", token])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cloudflared not found: {e}"))?;
        thread::sleep(Duration::from_secs(4));
        (child, json!({ "named": true }), None::<String>)
    } else {
        let mut child = Command::new("cloudflared")
            .args(["tunnel", "--url", &format!("http://localhost:{port}")])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cloudflared not found: {e}"))?;
        let (tx, rx) = mpsc::channel::<String>();
        if let Some(out) = child.stdout.take() {
            spawn_url_reader(out, tx.clone());
        }
        if let Some(err) = child.stderr.take() {
            spawn_url_reader(err, tx.clone());
        }
        let tunnel = rx
            .recv_timeout(Duration::from_secs(40))
            .map_err(|_| "Tunnel did not come up in time".to_string())?;
        (child, json!({ "tunnelUrl": tunnel }), Some(tunnel))
    };

    *state.cf.lock().unwrap() = Some(child);

    let message = match post_register(&platform_url, &agent_key, &tracks, extra.clone()) {
        Ok(n) => format!("{} tracks live", n),
        Err(e) => format!("Serving, but register failed: {e}"),
    };

    // Auto re-sync the catalog when the folder changes.
    {
        let running = state.running.clone();
        let root = root.clone();
        let platform_url = platform_url.clone();
        let agent_key = agent_key.clone();
        let extra = extra.clone();
        thread::spawn(move || run_watcher(root, platform_url, agent_key, extra, running));
    }

    let status = Status { running: true, tunnel_url: tunnel_marker, track_count: tracks.len(), message };
    *state.status.lock().unwrap() = status.clone();
    Ok(status)
}

#[tauri::command]
fn stop_agent(state: State<'_, AgentState>) -> Status {
    stop_internal(&state);
    Status::default()
}

#[tauri::command]
fn agent_status(state: State<'_, AgentState>) -> Status {
    state.status.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Verifies the L-048 scan: recursion into subfolders, persona/release from folder names
    // (dummy files have no ffprobe tags, so the folder-name fallback is exercised), video
    // detection, the release-kind heuristic, and direct-single (no persona/release).
    #[test]
    fn recursive_persona_release_scan() {
        let tmp = std::env::temp_dir().join("cerb_scan_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("PersonaA").join("AlbumX")).unwrap();
        fs::create_dir_all(tmp.join("PersonaB")).unwrap();
        fs::write(tmp.join("direct.mp3"), b"x").unwrap();
        fs::write(tmp.join("PersonaA").join("AlbumX").join("song1.mp3"), b"x").unwrap();
        fs::write(tmp.join("PersonaA").join("AlbumX").join("song2.mp3"), b"x").unwrap();
        fs::write(tmp.join("PersonaA").join("AlbumX").join("clip.mp4"), b"x").unwrap();
        fs::write(tmp.join("PersonaB").join("single.mp3"), b"x").unwrap();
        fs::write(tmp.join("notes.txt"), b"x").unwrap(); // non-media, ignored

        let tracks = build_tracks(&tmp);
        assert_eq!(tracks.len(), 5, "recursive scan finds all 5 media files, ignores .txt");

        let direct = tracks.iter().find(|t| t.filename == "direct.mp3").unwrap();
        assert!(direct.persona.is_none() && direct.release.is_none(), "top-level = direct single");

        let song1 = tracks.iter().find(|t| t.filename == "PersonaA/AlbumX/song1.mp3").unwrap();
        assert_eq!(song1.persona.as_deref(), Some("PersonaA"));
        assert_eq!(song1.release.as_deref(), Some("AlbumX"));
        assert_eq!(song1.release_kind.as_deref(), Some("ep"), "3 tracks in AlbumX -> ep");

        let clip = tracks.iter().find(|t| t.filename == "PersonaA/AlbumX/clip.mp4").unwrap();
        assert_eq!(clip.media_kind, "video", "mp4 detected as video");

        let _ = fs::remove_dir_all(&tmp);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AgentState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![scan_folder, start_agent, stop_agent, agent_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
