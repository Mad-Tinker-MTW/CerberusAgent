import { useEffect, useMemo, useState } from "react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { MOCK_LIBRARY, MOCK_COVERS } from "./editorMock";
import "./Editor.css";

export type CoverOption = { name: string; path: string };

/** Slug for title <-> cover-filename matching (e.g. "A Soldier's Ghost" -> "a-soldiers-ghost").
 *  Apostrophes are dropped (not treated as separators) so "Soldier's" matches a "soldiers" file. */
function coverSlug(s: string): string {
  return s.toLowerCase().replace(/['’]/g, "").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

// The 3-panel prep editor: Library (scanned tracks) -> Prep (edit tags + art) -> Serve (grouped
// preview of what will publish). Slice 1: read the real scan, edit into an in-memory draft, and
// preview the grouped result. Persisting drafts to file tags + the Serve push land in later slices.

export type EditorTrack = {
  title: string;
  filename: string;
  duration: string | null;
  persona: string | null;
  release: string | null;
  releaseKind: string | null; // album | ep | single
  trackNo: number | null;
  composer: string | null;
  mediaKind: string; // audio | video
  featured: boolean;
  cover: string | null;
  versionLabel: string | null; // group "versions" release: the genre/version of this track
  performer: string | null; // which group member performed this version
};

// The subset the Prep panel edits; merged over the scanned track to form the effective view.
type Draft = Partial<
  Pick<EditorTrack, "title" | "persona" | "release" | "releaseKind" | "trackNo" | "composer" | "versionLabel" | "performer">
>;

const IN_TAURI = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const KINDS = ["album", "ep", "single"] as const;

export function Editor({
  musicDir,
  artistName,
  agentKey,
  platformUrl,
  onExit,
}: {
  musicDir: string;
  artistName: string;
  agentKey: string;
  platformUrl: string;
  onExit: () => void;
}) {
  const [tracks, setTracks] = useState<EditorTrack[]>([]);
  const [drafts, setDrafts] = useState<Record<string, Draft>>({});
  const [selected, setSelected] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [saveMsg, setSaveMsg] = useState("");
  const [covers, setCovers] = useState<CoverOption[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const rows = IN_TAURI ? await invoke<EditorTrack[]>("scan_folder", { path: musicDir }) : MOCK_LIBRARY;
        setTracks(rows);
        setSelected(rows[0]?.filename ?? null);
      } catch (e) {
        setError(String(e));
        setTracks([]);
      }
      try {
        setCovers(IN_TAURI ? await invoke<CoverOption[]>("list_covers", { musicDir }) : MOCK_COVERS);
      } catch {
        setCovers([]);
      }
      setLoading(false);
    })();
  }, [musicDir]);

  // Effective track = scanned values with the draft laid on top.
  const effective = useMemo(() => {
    const map: Record<string, EditorTrack> = {};
    for (const t of tracks) map[t.filename] = { ...t, ...drafts[t.filename] };
    return map;
  }, [tracks, drafts]);

  const eff = (f: string) => effective[f];
  const sel = selected ? eff(selected) : null;

  function patch(field: keyof Draft, value: string | number | null) {
    if (!selected) return;
    setSaveMsg("");
    setDrafts((d) => ({ ...d, [selected]: { ...d[selected], [field]: value } }));
  }

  // Write the edited tracks' effective values back into their file tags, then re-scan so the
  // library reflects what's now on disk.
  async function saveTags() {
    const edits = tracks
      .filter((t) => drafts[t.filename])
      .map((t) => {
        const e = eff(t.filename);
        return { filename: t.filename, title: e.title, persona: e.persona, release: e.release, trackNo: e.trackNo, composer: e.composer, versionLabel: e.versionLabel, performer: e.performer };
      });
    if (!edits.length) return;
    if (!IN_TAURI) {
      setSaveMsg(`Preview mode: would write tags to ${edits.length} file${edits.length > 1 ? "s" : ""}.`);
      return;
    }
    setError("");
    try {
      const n = await invoke<number>("write_tags", { musicDir, edits });
      const rows = await invoke<EditorTrack[]>("scan_folder", { path: musicDir });
      setTracks(rows);
      setDrafts({});
      setSaveMsg(`Saved tags to ${n} file${n > 1 ? "s" : ""}.`);
    } catch (e) {
      setError(String(e));
    }
  }

  // The tracks a cover applies to: a whole release shares one cover; a loose single is just itself.
  function coverTargets(): string[] {
    if (!sel || !selected) return [];
    return sel.release
      ? tracks.filter((t) => { const e = eff(t.filename); return e.release === sel.release && e.persona === sel.persona; }).map((t) => t.filename)
      : [selected];
  }

  // Embed an image (from the library or a browsed file) into the target tracks, then re-scan.
  async function applyCover(imagePath: string) {
    setPickerOpen(false);
    if (!IN_TAURI) {
      setSaveMsg("Preview mode: the cover is embedded into your files in the app.");
      return;
    }
    setError("");
    setSaveMsg("Setting cover…");
    try {
      const n = await invoke<number>("set_cover", { musicDir, filenames: coverTargets(), imagePath });
      const rows = await invoke<EditorTrack[]>("scan_folder", { path: musicDir });
      setTracks(rows);
      setSaveMsg(`Cover set on ${n} track${n === 1 ? "" : "s"}.`);
    } catch (e) {
      setError(String(e));
      setSaveMsg("");
    }
  }

  // Browse the filesystem for an image outside the library.
  async function browseCover() {
    if (!IN_TAURI) {
      setSaveMsg("Preview mode: Browse opens a file dialog in the app.");
      setPickerOpen(false);
      return;
    }
    const img = await openDialog({ title: "Pick cover art", filters: [{ name: "Image", extensions: ["png", "jpg", "jpeg", "gif"] }] });
    if (typeof img === "string") await applyCover(img);
  }

  // Publish the saved catalog to the dossier. Requires no unsaved edits, so what publishes matches
  // the files on disk (Serve re-scans server-side).
  async function serveCatalog() {
    if (dirty) {
      setSaveMsg("Save your tag edits first, then serve.");
      return;
    }
    const releaseCount = groups.reduce((n, g) => n + g.releases.length, 0);
    if (!IN_TAURI) {
      setSaveMsg(`Preview mode: would publish ${releaseCount} release${releaseCount === 1 ? "" : "s"} to your dossier.`);
      return;
    }
    setError("");
    setSaveMsg("Publishing…");
    try {
      const n = await invoke<number>("serve_catalog", { musicDir, agentKey, platformUrl });
      setSaveMsg(`Published ${n} track${n === 1 ? "" : "s"} to your dossier.`);
    } catch (e) {
      setError(String(e));
      setSaveMsg("");
    }
  }

  // Group the effective tracks into release cards, split by type for the Serve preview.
  const groups = useMemo(() => buildGroups(tracks.map((t) => eff(t.filename)), artistName), [effective, tracks, artistName]);
  const dirty = Object.keys(drafts).length > 0;

  // Library covers, with the one whose filename matches the release (or single title) sorted first.
  const matchSlug = sel ? coverSlug(sel.release || sel.title) : "";
  const sortedCovers = useMemo(() => {
    const scored = covers.map((c) => ({ c, match: coverSlug(c.name) === matchSlug }));
    scored.sort((a, b) => (a.match === b.match ? 0 : a.match ? -1 : 1));
    return scored;
  }, [covers, matchSlug]);

  return (
    <div className="ed">
      <header className="ed-top">
        <button className="btn ghost sm" onClick={onExit}>&larr; Back</button>
        <h2 className="ed-title">Organize library</h2>
        <div className="ed-actions">
          {saveMsg && <span className="ed-savemsg">{saveMsg}</span>}
          <button className="btn ghost sm" disabled={!dirty} onClick={saveTags} title="Write the edited tags back into your files">
            Save tags
          </button>
          <button className="btn go sm" onClick={serveCatalog} disabled={loading || tracks.length === 0} title="Publish the saved catalog to your dossier">
            Serve
          </button>
        </div>
      </header>

      {error && <p className="ed-err">{error}</p>}

      <div className="ed-cols">
        <section className="ed-pane">
          <div className="ed-pane-h"><span className="step">1</span> Library <span className="muted">{tracks.length}</span></div>
          {loading ? (
            <p className="muted pad">Scanning&hellip;</p>
          ) : (
            <ul className="ed-list">
              {tracks.map((raw) => {
                const t = eff(raw.filename);
                const filed = Boolean(t.persona);
                return (
                  <li
                    key={raw.filename}
                    className={`ed-item ${selected === raw.filename ? "on" : ""}`}
                    onClick={() => setSelected(raw.filename)}
                  >
                    <span className={`dot ${filed ? "ok" : "warn"}`} />
                    <span className="ed-item-main">
                      <span className="ed-item-title">{t.title}</span>
                      <span className="muted sm">{t.persona ? `${t.persona}${t.release ? ` · ${t.release}` : ""}` : "needs a voice"}</span>
                    </span>
                    {drafts[raw.filename] && <span className="pip" title="edited" />}
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="ed-pane">
          <div className="ed-pane-h"><span className="step">2</span> Prep</div>
          {!sel ? (
            <p className="muted pad">Pick a track.</p>
          ) : (
            <div className="ed-form">
              <div className="cover-area">
                <button type="button" className="cover-drop" onClick={() => setPickerOpen((o) => !o)} title="Pick cover art">
                  {sel.cover ? (
                    <span className="cover-set"><span className="cover-check">✓</span> cover set{sel.release ? " · release" : ""}<br /><span className="muted sm">change</span></span>
                  ) : (
                    <span>+ add cover art{sel.release ? <><br /><span className="muted sm">whole release</span></> : null}</span>
                  )}
                </button>
                {pickerOpen && (
                  <div className="cover-picker">
                    {sortedCovers.length === 0 ? (
                      <p className="muted sm pad">No images in your Album Covers folder.</p>
                    ) : (
                      <div className="cover-grid">
                        {sortedCovers.map(({ c, match }) => (
                          <button key={c.path} type="button" className={`cover-tile ${match ? "match" : ""}`} onClick={() => applyCover(c.path)} title={c.name}>
                            {IN_TAURI ? <img src={convertFileSrc(c.path)} alt="" /> : <span className="cover-ph">img</span>}
                            <span className="cover-name">{c.name}{match ? " ✓" : ""}</span>
                          </button>
                        ))}
                      </div>
                    )}
                    <div className="cover-picker-actions">
                      <button type="button" className="btn ghost sm" onClick={browseCover}>Browse…</button>
                      <button type="button" className="btn ghost sm" onClick={() => setPickerOpen(false)}>Cancel</button>
                    </div>
                  </div>
                )}
              </div>
              <Field label="Title"><input value={sel.title} onChange={(e) => patch("title", e.target.value)} /></Field>
              <Field label="Voice / artist" hint={sel.persona ? "AI voice" : "empty = your own voice (direct)"}>
                <input value={sel.persona ?? ""} placeholder={artistName} onChange={(e) => patch("persona", e.target.value || null)} />
              </Field>
              <div className="ed-row2">
                <Field label="Release"><input value={sel.release ?? ""} placeholder="single" onChange={(e) => patch("release", e.target.value || null)} /></Field>
                <Field label="Type">
                  <select value={sel.releaseKind ?? ""} onChange={(e) => patch("releaseKind", e.target.value || null)}>
                    <option value="">auto</option>
                    {KINDS.map((k) => <option key={k} value={k}>{k}</option>)}
                  </select>
                </Field>
              </div>
              <div className="ed-row2">
                <Field label="Track #"><input type="number" min={1} value={sel.trackNo ?? ""} onChange={(e) => patch("trackNo", e.target.value ? Number(e.target.value) : null)} /></Field>
                <Field label="AI-assisted" hint="a voice persona = AI-assisted"><input value={sel.persona ? "Yes" : "No"} readOnly /></Field>
              </div>
              <Field label="Composer / contributing"><input value={sel.composer ?? ""} placeholder={artistName} onChange={(e) => patch("composer", e.target.value || null)} /></Field>
              <div className="ed-row2">
                <Field label="Version" hint="group release"><input value={sel.versionLabel ?? ""} placeholder="e.g. Reggaeton" onChange={(e) => patch("versionLabel", e.target.value || null)} /></Field>
                <Field label="Performer" hint="which member"><input value={sel.performer ?? ""} placeholder="member" onChange={(e) => patch("performer", e.target.value || null)} /></Field>
              </div>
            </div>
          )}
        </section>

        <section className="ed-pane">
          <div className="ed-pane-h"><span className="step">3</span> Ready to serve</div>
          <div className="ed-serve">
            {groups.map((g) => (
              <div key={g.heading}>
                <p className="ed-serve-h">{g.heading}</p>
                {g.releases.map((r) => (
                  <div key={r.key} className="ed-rel">
                    <div className="ed-rel-art">{r.cover ? "art" : "♪"}</div>
                    <div className="ed-rel-main">
                      <span className="ed-item-title">{r.title}</span>
                      <span className="muted sm">{r.persona} · {r.kind}{r.tracks.length > 1 ? ` · ${r.tracks.length} tracks` : ""}</span>
                    </div>
                  </div>
                ))}
              </div>
            ))}
            {groups.length === 0 && <p className="muted pad">Nothing filed yet.</p>}
          </div>
        </section>
      </div>
      {!IN_TAURI && <p className="ed-note">Preview mode (sample library). Running in the app scans your real folder.</p>}
    </div>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="ed-field">
      <span>{label}{hint && <em> — {hint}</em>}</span>
      {children}
    </label>
  );
}

type RelCard = { key: string; title: string; persona: string; kind: string; tracks: EditorTrack[]; cover: string | null };

// Build release cards from effective tracks and split them under Albums / EPs / Singles headings.
// A track with a release joins that release; a track with none is a single (its own card).
function buildGroups(tracks: EditorTrack[], artistName: string): { heading: string; releases: RelCard[] }[] {
  const byRelease = new Map<string, RelCard>();
  const singles: RelCard[] = [];
  for (const t of tracks) {
    const persona = t.persona || artistName;
    if (t.release) {
      const key = `${persona}|${t.release}`;
      let r = byRelease.get(key);
      if (!r) {
        r = { key, title: t.release, persona, kind: t.releaseKind || "ep", tracks: [], cover: t.cover };
        byRelease.set(key, r);
      }
      r.tracks.push(t);
      if (!r.cover && t.cover) r.cover = t.cover;
    } else {
      singles.push({ key: `s:${t.filename}`, title: t.title, persona, kind: "single", tracks: [t], cover: t.cover });
    }
  }
  const rels = [...byRelease.values()];
  for (const r of rels) r.tracks.sort((a, b) => (a.trackNo ?? 0) - (b.trackNo ?? 0));
  const out: { heading: string; releases: RelCard[] }[] = [];
  const albums = rels.filter((r) => r.kind === "album");
  const eps = rels.filter((r) => r.kind === "ep");
  const allSingles = [...rels.filter((r) => r.kind === "single"), ...singles];
  if (albums.length) out.push({ heading: "Albums", releases: albums });
  if (eps.length) out.push({ heading: "EPs", releases: eps });
  if (allSingles.length) out.push({ heading: "Singles", releases: allSingles });
  return out;
}
