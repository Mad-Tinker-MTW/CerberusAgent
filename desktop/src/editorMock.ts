import type { EditorTrack, CoverOption } from "./Editor";

// Browser-dev fixture for the cover library picker: the owner's 4 EP covers, named by release so
// the title-match highlights the right one.
export const MOCK_COVERS: CoverOption[] = [
  { name: "a-soldiers-ghost", path: "X:/Music/Album Covers/a-soldiers-ghost.png" },
  { name: "without-confession", path: "X:/Music/Album Covers/without-confession.png" },
  { name: "the-price-of-desire", path: "X:/Music/Album Covers/the-price-of-desire.png" },
  { name: "highway-con-sexy", path: "X:/Music/Album Covers/highway-con-sexy.png" },
];

// Browser-dev fixture: mirrors the owner's real catalog shape so the 3-panel editor renders
// (and grouping is exercised) without the Tauri backend. Used only when not running in Tauri.
export const MOCK_LIBRARY: EditorTrack[] = [
  // Styrling Shadow — A Soldier's Ghost (EP)
  t("Blade Before the Dawn", "Styrling Shadow/A Soldiers Ghost/01 Blade Before the Dawn.mp3", "Styrling Shadow", "A Soldier's Ghost", "ep", 1),
  t("Ashes of Mercy", "Styrling Shadow/A Soldiers Ghost/02 Ashes of Mercy.mp3", "Styrling Shadow", "A Soldier's Ghost", "ep", 2),
  t("Fallen Brother", "Styrling Shadow/A Soldiers Ghost/03 Fallen Brother.mp3", "Styrling Shadow", "A Soldier's Ghost", "ep", 3),
  // Bianca Ravina — Without Confession (EP)
  t("Without Confession", "Bianca Ravina/Without Confession/01 Without Confession.mp3", "Bianca Ravina", "Without Confession", "ep", 1),
  t("Velvet Communion", "Bianca Ravina/Without Confession/02 Velvet Communion.mp3", "Bianca Ravina", "Without Confession", "ep", 2),
  // Kings Without Crowns — Highway con Sexy (EP, group versions)
  ver("Highway con Sexy — Country", "Kings without Crowns/Highway con Sexy/Country.mp3", "Kings Without Crowns", "Highway con Sexy", 1, "Country", "El Vaquero"),
  ver("Highway con Sexy — Reggaeton", "Kings without Crowns/Highway con Sexy/Reggaeton.mp3", "Kings Without Crowns", "Highway con Sexy", 2, "Reggaeton", "El Rey"),
  // A misfiled track: artist tag leaked, no release -> lands as a loose single needing work
  t("Codebreaker Queen", "Singles/Codebreaker Queen.mp3", null, null, null, null),
];

function t(
  title: string,
  filename: string,
  persona: string | null,
  release: string | null,
  releaseKind: string | null,
  trackNo: number | null
): EditorTrack {
  return {
    title,
    filename,
    duration: "3:40",
    persona,
    release,
    releaseKind,
    trackNo,
    composer: null,
    mediaKind: "audio",
    featured: false,
    cover: null,
    versionLabel: null,
    performer: null,
  };
}

// A group-version track: an EP track that also carries a version label + performer.
function ver(
  title: string,
  filename: string,
  persona: string,
  release: string,
  trackNo: number,
  versionLabel: string,
  performer: string
): EditorTrack {
  return { ...t(title, filename, persona, release, "ep", trackNo), versionLabel, performer };
}
