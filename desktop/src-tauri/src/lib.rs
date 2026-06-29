// Cerberus Agent — Tauri backend.
// Serves the artist's local music folder (Range + CORS), opens a cloudflared
// quick tunnel, and registers the tunnel URL + track list to the artist's dossier.
// The machine is the storage; Cerberus only stores the tunnel URL.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

const AUDIO_EXTS: &[&str] = &["mp3", "wav", "flac", "m4a", "ogg", "aac"];

#[derive(Serialize, Clone)]
struct Track {
    title: String,
    filename: String,
    duration: Option<String>,
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

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("m4a") => "audio/mp4",
        Some("ogg") => "audio/ogg",
        Some("aac") => "audio/aac",
        _ => "application/octet-stream",
    }
}

fn ffprobe_duration(path: &Path) -> Option<String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nokey=1"])
        .arg(path)
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    let secs: f64 = s.trim().parse().ok()?;
    let m = (secs / 60.0).floor() as u64;
    let sec = (secs % 60.0).floor() as u64;
    Some(format!("{}:{:02}", m, sec))
}

fn list_tracks(dir: &Path) -> Result<Vec<Track>, String> {
    if !dir.is_dir() {
        return Err("Not a folder".into());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| AUDIO_EXTS.contains(&x.to_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .collect();
    files.sort();
    let mut tracks = Vec::new();
    for (i, f) in files.iter().enumerate() {
        let title = f.file_stem().and_then(|s| s.to_str()).unwrap_or("track").to_string();
        let filename = f.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        tracks.push(Track { title, filename, duration: ffprobe_duration(f), featured: i == 0 });
    }
    Ok(tracks)
}

#[tauri::command]
fn scan_folder(path: String) -> Result<Vec<Track>, String> {
    list_tracks(&PathBuf::from(path))
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

// Local static server (Range + CORS). Runs until `running` flips false.
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
    state: State<'_, AgentState>,
) -> Result<Status, String> {
    let port = port.unwrap_or(8787);
    let root = PathBuf::from(&music_dir);
    let tracks = list_tracks(&root)?;
    if tracks.is_empty() {
        return Err("No audio files in that folder".into());
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

    *state.cf.lock().unwrap() = Some(child);

    let body = serde_json::json!({ "tunnelUrl": tunnel, "tracks": tracks });
    let url = format!("{}/api/agent/register", platform_url.trim_end_matches('/'));
    let message = match ureq::post(&url)
        .set("Authorization", &format!("Bearer {}", agent_key))
        .send_json(body)
    {
        Ok(_) => format!("{} tracks live", tracks.len()),
        Err(e) => format!("Serving, but register failed: {e}"),
    };

    let status = Status { running: true, tunnel_url: Some(tunnel), track_count: tracks.len(), message };
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
