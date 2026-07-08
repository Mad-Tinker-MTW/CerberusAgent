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

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::prelude::*;
use lofty::tag::Tag;
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
    // Relative path to the embedded cover art extracted to a served sidecar, or None when the
    // file carries no art. The register route turns this into a gateway URL.
    cover: Option<String>,
    // Group "versions" release fields: the genre/version label and the member who performed it.
    // Read from the file's subtitle + performer tags (lofty), not the ffprobe standard set.
    #[serde(rename = "versionLabel")]
    version_label: Option<String>,
    performer: Option<String>,
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
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

const COVERS_DIR: &str = ".cerberus-covers";

/// Deterministic, filesystem-safe sidecar path for a track's extracted cover, under COVERS_DIR.
/// Same input always yields the same name, so a re-scan overwrites rather than piling up files.
fn cover_sidecar_name(rel_url: &str) -> String {
    let no_ext = rel_url.rsplit_once('.').map(|(a, _)| a).unwrap_or(rel_url);
    let safe: String = no_ext
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let t = safe.trim_matches('_');
    format!("{}/{}.jpg", COVERS_DIR, if t.is_empty() { "cover" } else { t })
}

/// Extract a file's embedded cover art (APIC / attached picture) to a served sidecar jpg via
/// ffmpeg, returning its relative path. Returns None when the file has no art, ffmpeg is
/// missing, or extraction fails (best-effort: a track without art just gets no cover).
fn extract_cover(full: &Path, root: &Path, rel_url: &str) -> Option<String> {
    let rel = cover_sidecar_name(rel_url);
    let out = root.join(&rel);
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-i"])
        .arg(full)
        .args(["-an", "-frames:v", "1"])
        .arg(&out)
        .status();
    let ok = matches!(status, Ok(s) if s.success())
        && out.metadata().map(|m| m.len() > 0).unwrap_or(false);
    if ok {
        Some(rel)
    } else {
        let _ = fs::remove_file(&out);
        None
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

/// Read the version subtitle (group "versions" releases), which our ffprobe pass doesn't cover.
/// Best-effort via lofty. The performer is derived from the artist/album_artist split in the scan.
fn read_version_label(path: &Path) -> Option<String> {
    let tagged = lofty::read_from_path(path).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    tag.get_string(&ItemKey::TrackSubtitle).map(|s| s.to_string())
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
        // Read embedded art + the version/performer fields (audio only).
        let cover = if media_kind == "audio" { extract_cover(full, root, &rel_url) } else { None };
        let version_label = if media_kind == "audio" { read_version_label(full) } else { None };
        // Performer = the track artist (TPE1) when it differs from the act (album_artist / TPE2), the
        // group-version case (album_artist = KWC, artist = the member). Otherwise there's no separate
        // performer credit to surface.
        let performer = match (tags.get("album_artist"), tags.get("artist")) {
            (Some(act), Some(art)) if act != art => Some(art.clone()),
            _ => None,
        };

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
            cover,
            version_label,
            performer,
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

// One track's edited fields from the Prep panel. Empty/None fields are left untouched on disk.
#[derive(serde::Deserialize)]
struct TagEdit {
    filename: String, // relative path with forward slashes, as returned by the scan
    title: Option<String>,
    persona: Option<String>, // -> album_artist (+ artist), so the scan seats it under this voice
    release: Option<String>, // -> album
    #[serde(rename = "trackNo")]
    track_no: Option<u32>,
    composer: Option<String>,
    #[serde(rename = "versionLabel")]
    version_label: Option<String>,
    performer: Option<String>,
}

fn nonempty(o: &Option<String>) -> Option<String> {
    o.as_ref().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Write one track's edited fields into its file tags via lofty. This is the Mp3tag-style
/// write-back: it makes the file itself the source of truth (portable, DistroKid-ready) so a
/// re-scan reads the corrected persona/release straight from the tags.
fn write_one_tag(path: &Path, e: &TagEdit) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(path).map_err(|err| format!("read {}: {err}", path.display()))?;
    if tagged.primary_tag_mut().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged.primary_tag_mut().ok_or("no writable tag")?;
    if let Some(v) = nonempty(&e.title) {
        tag.set_title(v);
    }
    // Standard TPE2/TPE1 convention: album_artist = the act/persona (the scan reads persona from
    // it first, overwriting any leaked "frankydlp"); artist = the specific performer of this track
    // when given (group-version case), else the persona itself.
    let persona = nonempty(&e.persona);
    let performer = nonempty(&e.performer);
    if let Some(p) = &persona {
        tag.insert_text(ItemKey::AlbumArtist, p.clone());
        tag.set_artist(performer.clone().unwrap_or_else(|| p.clone()));
    } else if let Some(pf) = &performer {
        tag.set_artist(pf.clone());
    }
    if let Some(v) = nonempty(&e.release) {
        tag.set_album(v);
    }
    if let Some(n) = e.track_no {
        if n > 0 {
            tag.set_track(n);
        }
    }
    if let Some(v) = nonempty(&e.composer) {
        tag.insert_text(ItemKey::Composer, v);
    }
    if let Some(v) = nonempty(&e.version_label) {
        tag.insert_text(ItemKey::TrackSubtitle, v);
    }
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(())
}

/// Write the Prep panel's edits back into the files under `music_dir`. Continues past a failed file
/// and reports how many were written; only errors when nothing could be written.
#[tauri::command]
fn write_tags(music_dir: String, edits: Vec<TagEdit>) -> Result<u32, String> {
    let root = PathBuf::from(&music_dir);
    let mut written = 0u32;
    let mut first_err: Option<String> = None;
    for e in &edits {
        let path = root.join(&e.filename);
        if !path.starts_with(&root) || !path.is_file() {
            first_err.get_or_insert_with(|| format!("skipped {}", e.filename));
            continue;
        }
        match write_one_tag(&path, e) {
            Ok(()) => written += 1,
            Err(msg) => {
                first_err.get_or_insert(msg);
            }
        }
    }
    if written == 0 {
        return Err(first_err.unwrap_or_else(|| "nothing to write".into()));
    }
    Ok(written)
}

fn mime_from_ext(path: &Path) -> Option<MimeType> {
    match ext_of(path).as_str() {
        "jpg" | "jpeg" => Some(MimeType::Jpeg),
        "png" => Some(MimeType::Png),
        "gif" => Some(MimeType::Gif),
        _ => None,
    }
}

/// Embed an image as a file's front-cover art via lofty. Making the art part of the file (rather
/// than a loose sidecar) keeps it portable and lets the existing extract-cover path serve it.
fn embed_cover(path: &Path, mime: MimeType, data: Vec<u8>) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if tagged.primary_tag_mut().is_none() {
        let tt = tagged.primary_tag_type();
        tagged.insert_tag(Tag::new(tt));
    }
    let tag = tagged.primary_tag_mut().ok_or("no writable tag")?;
    let pic = Picture::new_unchecked(PictureType::CoverFront, Some(mime), None, data);
    tag.set_picture(0, pic);
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

/// Set the cover art on one or more tracks (a release's tracks, or a single). The image is embedded
/// into each file; a re-scan then extracts it through the normal cover pipeline.
#[tauri::command]
fn set_cover(music_dir: String, filenames: Vec<String>, image_path: String) -> Result<u32, String> {
    let root = PathBuf::from(&music_dir);
    let img = PathBuf::from(&image_path);
    let mime = mime_from_ext(&img).ok_or("Cover must be a jpg, png, or gif")?;
    let data = fs::read(&img).map_err(|e| format!("read image: {e}"))?;
    let mut n = 0u32;
    let mut first_err: Option<String> = None;
    for f in &filenames {
        let p = root.join(f);
        if !p.starts_with(&root) || !p.is_file() {
            first_err.get_or_insert_with(|| format!("skipped {f}"));
            continue;
        }
        match embed_cover(&p, mime.clone(), data.clone()) {
            Ok(()) => n += 1,
            Err(e) => {
                first_err.get_or_insert(e);
            }
        }
    }
    if n == 0 {
        return Err(first_err.unwrap_or_else(|| "no files".into()));
    }
    Ok(n)
}

#[derive(Serialize)]
struct CoverOption {
    name: String,
    path: String,
}

/// List candidate cover images for the library picker: the "Album Covers" subfolder (where the
/// artist keeps release art named by title) plus any images at the top level. Only formats set_cover
/// can embed (jpg/png/gif).
#[tauri::command]
fn list_covers(music_dir: String) -> Result<Vec<CoverOption>, String> {
    let root = PathBuf::from(&music_dir);
    let mut out: Vec<CoverOption> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in [root.join("Album Covers"), root.clone()] {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() && matches!(ext_of(&p).as_str(), "png" | "jpg" | "jpeg" | "gif") {
                let path = p.to_string_lossy().to_string();
                if seen.insert(path.clone()) {
                    let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("cover").to_string();
                    out.push(CoverOption { name, path });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

/// Publish the current on-disk catalog to the artist's dossier: re-scan the folder (so what's served
/// matches the saved tags exactly) and register it. Named-mode, so the platform derives the liveness
/// marker from the provisioned media origin. Returns the number of tracks published.
#[tauri::command]
fn serve_catalog(music_dir: String, agent_key: String, platform_url: String) -> Result<usize, String> {
    let root = PathBuf::from(&music_dir);
    if !root.is_dir() {
        return Err("Music folder not found".into());
    }
    let tracks = build_tracks(&root);
    if tracks.is_empty() {
        return Err("No tracks to serve".into());
    }
    post_register(&platform_url, &agent_key, &tracks, json!({ "named": true }))
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
        let mut cmd = Command::new("cloudflared");
        cmd.args(["tunnel", "run", "--token", token])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // cloudflared runs as a background service; on Windows a plain spawn pops a console
        // window a non-technical artist could close and kill their own stream. Hide it.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("cloudflared not found: {e}"))?;
        thread::sleep(Duration::from_secs(4));
        (child, json!({ "named": true }), None::<String>)
    } else {
        let mut cmd = Command::new("cloudflared");
        cmd.args(["tunnel", "--url", &format!("http://localhost:{port}")])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let mut child = cmd
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

/// Location of the persisted agent identity: %APPDATA%\Cerberus\agent.json.
/// Isolated from webview localStorage so a compromised page can't read the keys.
fn config_path() -> Result<PathBuf, String> {
    let appdata = std::env::var("APPDATA").map_err(|_| "APPDATA not set".to_string())?;
    let dir = PathBuf::from(appdata).join("Cerberus");
    fs::create_dir_all(&dir).map_err(|e| format!("could not create config dir: {e}"))?;
    Ok(dir.join("agent.json"))
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Default)]
struct AgentConfig {
    slug: String,
    #[serde(rename = "agentKey")]
    agent_key: String,
    #[serde(rename = "tunnelToken")]
    tunnel_token: String,
    #[serde(rename = "mediaOrigin")]
    media_origin: String,
    #[serde(rename = "platformUrl")]
    platform_url: String,
    #[serde(default, rename = "musicDir")]
    music_dir: String,
}

#[tauri::command]
fn save_config(config: AgentConfig) -> Result<(), String> {
    let path = config_path()?;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    fs::write(&tmp, bytes).map_err(|e| format!("write failed: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename failed: {e}"))?;
    Ok(())
}

#[tauri::command]
fn load_config() -> Option<AgentConfig> {
    let path = config_path().ok()?;
    if !path.exists() {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    serde_json::from_slice::<AgentConfig>(&bytes).ok()
}

#[tauri::command]
fn clear_config() -> Result<(), String> {
    let path = config_path()?;
    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("delete failed: {e}"))?;
    }
    Ok(())
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

        // Dummy files carry no embedded art, so extraction is a clean no-op (best-effort None).
        assert!(direct.cover.is_none(), "no embedded art -> no cover, no crash");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn write_tags_roundtrip() {
        let tmp = std::env::temp_dir().join("cerb_write_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let f = tmp.join("song.mp3");
        // Generate a real 1s mp3 so lofty has a valid container to tag.
        let made = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=1"])
            .arg(&f)
            .status();
        if made.map(|s| !s.success()).unwrap_or(true) {
            eprintln!("ffmpeg unavailable; skipping write_tags_roundtrip");
            return;
        }
        let edit = TagEdit {
            filename: "song.mp3".into(),
            title: Some("Highway con Sexy".into()),
            persona: Some("Kings Without Crowns".into()),
            release: Some("Highway con Sexy".into()),
            track_no: Some(1),
            composer: Some("Francisco De La Paz".into()),
            version_label: Some("Reggaeton".into()),
            performer: Some("El Rey".into()),
        };
        write_one_tag(&f, &edit).unwrap();

        // Re-read straight from the file and confirm the tags stuck, including version + performer.
        let tagged = lofty::read_from_path(&f).unwrap();
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag()).unwrap();
        assert_eq!(tag.title().as_deref(), Some("Highway con Sexy"));
        assert_eq!(tag.album().as_deref(), Some("Highway con Sexy"));
        assert_eq!(tag.track(), Some(1));
        // album_artist = the act; artist = the version's performer; subtitle = the version.
        assert_eq!(tag.get_string(&ItemKey::AlbumArtist), Some("Kings Without Crowns"));
        assert_eq!(tag.artist().as_deref(), Some("El Rey"));
        assert_eq!(tag.get_string(&ItemKey::TrackSubtitle), Some("Reggaeton"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn set_cover_embeds_picture() {
        let tmp = std::env::temp_dir().join("cerb_cover_test");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        let mp3 = tmp.join("song.mp3");
        let jpg = tmp.join("art.jpg");
        let mk_mp3 = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=1"])
            .arg(&mp3)
            .status();
        let mk_jpg = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-f", "lavfi", "-i", "color=c=red:s=64x64:d=1", "-frames:v", "1"])
            .arg(&jpg)
            .status();
        let ok = mk_mp3.map(|s| s.success()).unwrap_or(false) && mk_jpg.map(|s| s.success()).unwrap_or(false);
        if !ok {
            eprintln!("ffmpeg unavailable; skipping set_cover_embeds_picture");
            return;
        }
        let data = fs::read(&jpg).unwrap();
        embed_cover(&mp3, MimeType::Jpeg, data).unwrap();
        let tagged = lofty::read_from_path(&mp3).unwrap();
        let tag = tagged.primary_tag().or_else(|| tagged.first_tag()).unwrap();
        assert!(!tag.pictures().is_empty(), "front cover embedded");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn cover_sidecar_name_safe_and_deterministic() {
        let n = cover_sidecar_name("Styrling Shadow/Fallen Brother.mp3");
        assert_eq!(n, ".cerberus-covers/styrling_shadow_fallen_brother.jpg");
        // Stable across calls, and always inside the covers dir with a .jpg extension.
        assert_eq!(n, cover_sidecar_name("Styrling Shadow/Fallen Brother.mp3"));
        assert!(cover_sidecar_name("weird/../name!.flac").starts_with(".cerberus-covers/"));
        assert!(cover_sidecar_name("x.mp3").ends_with(".jpg"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AgentState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_folder,
            write_tags,
            set_cover,
            list_covers,
            serve_catalog,
            start_agent,
            stop_agent,
            agent_status,
            save_config,
            load_config,
            clear_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
