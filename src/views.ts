import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { Track, Album, ViewMode } from "./types";
import {
  getTracks, ensureTracksLoaded, setTracks,
  getCurrentView, setCurrentView,
  getCurrentAlbumContext, setCurrentAlbumContext,
  getSearchQuery,
} from "./state";
import { isCurrentTrack, playTrack, formatDuration, getFormat } from "./player";
import { getIsPlaying } from "./state";
import { escapeHtml } from "./utils";
import { refreshSync } from "./sync";

const views = {
  tracks: () => document.getElementById("track-list"),
  albums: () => document.getElementById("album-grid"),
  details: () => document.getElementById("album-details-view"),
  sync: () => document.getElementById("sync-view"),
};

const navBtns = {
  tracks: () => document.getElementById("tracks-btn"),
  albums: () => document.getElementById("albums-btn"),
  sync: () => document.getElementById("sync-btn"),
};

export function switchView(mode: ViewMode, data?: Album) {
  setCurrentView(mode);

  document.querySelectorAll("nav button").forEach(btn => btn.classList.remove("active"));
  if (mode === 'tracks') navBtns.tracks()?.classList.add("active");
  if (mode === 'albums' || mode === 'album-details') navBtns.albums()?.classList.add("active");
  if (mode === 'sync') navBtns.sync()?.classList.add("active");

  Object.values(views).forEach(el => el()?.classList.add("hidden"));

  if (mode === 'tracks') {
    views.tracks()?.classList.remove("hidden");
    loadTracks();
  } else if (mode === 'albums') {
    views.albums()?.classList.remove("hidden");
    loadAlbums();
  } else if (mode === 'album-details') {
    views.details()?.classList.remove("hidden");
    if (data) renderAlbumDetails(data);
  } else if (mode === 'sync') {
    views.sync()?.classList.remove("hidden");
    updateStatusBar("VIEW: DEVICE SYNC", "");
    void refreshSync();
  }
}

export function updateStatusBar(viewLabel: string, statsLabel: string) {
  const lbl = document.getElementById("current-view-label");
  const stats = document.getElementById("library-stats");
  if (lbl) lbl.innerText = viewLabel;
  if (stats) stats.innerText = statsLabel;
}

const ROW_HEIGHT = 28;

export async function loadTracks() {
  await ensureTracksLoaded();
  const container = views.tracks();
  if (!container) return;

  const allTracks = getTracks();

  if (!container.querySelector(".vs-spacer")) {
    container.innerHTML = "";
    const spacer = document.createElement("div");
    spacer.className = "vs-spacer";
    container.appendChild(spacer);

    const content = document.createElement("div");
    content.className = "vs-content";
    container.appendChild(content);
  }

  if (!container.dataset.virtualized) {
    container.dataset.virtualized = "true";
    container.onscroll = () => renderVirtualTracks();
  }

  renderVirtualTracks();
  updateStatusBar("VIEW: TRACKS", `${allTracks.length} ITEMS`);
}

function renderVirtualTracks() {
  const container = views.tracks();
  if (!container) return;

  const searchQuery = getSearchQuery();
  const allTracks = getTracks();

  const filteredTracks = searchQuery
    ? allTracks.filter(t =>
      (t.title?.toLowerCase().includes(searchQuery)) ||
      (t.artist?.toLowerCase().includes(searchQuery)) ||
      (t.album?.toLowerCase().includes(searchQuery))
    )
    : allTracks;

  const scrollTop = container.scrollTop;
  const containerHeight = container.clientHeight;

  const startIndex = Math.floor(scrollTop / ROW_HEIGHT);
  const endIndex = Math.min(filteredTracks.length - 1, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT));

  let spacer = container.querySelector(".vs-spacer") as HTMLElement;
  let content = container.querySelector(".vs-content") as HTMLElement;
  if (!spacer || !content) return;

  spacer.style.height = `${filteredTracks.length * ROW_HEIGHT}px`;
  spacer.style.position = "relative";

  content.style.position = "absolute";
  content.style.top = "0";
  content.style.left = "0";
  content.style.right = "0";
  content.style.transform = `translateY(${startIndex * ROW_HEIGHT}px)`;
  content.style.paddingTop = "20px";
  content.style.paddingBottom = "20px";
  content.innerHTML = "";

  for (let i = startIndex; i <= endIndex; i++) {
    const track = filteredTracks[i];
    const playing = getIsPlaying() && isCurrentTrack(track);
    const div = document.createElement("div");
    div.className = "track-item" + (playing ? " playing" : "");
    div.style.height = `${ROW_HEIGHT}px`;
    div.innerHTML = `
      <span>${playing ? ">>" : (i + 1).toString().padStart(2, '0')}</span>
      <span>${escapeHtml(track.title || "Untitled")}</span>
      <span>${escapeHtml(track.artist || "Unknown")}</span>
      <span class="format-tag">${getFormat(track.path)}</span>
      <span>${formatDuration(track.duration || 0)}</span>
    `;
    div.onclick = () => playTrack(track, filteredTracks);
    content.appendChild(div);
  }

  updateStatusBar("VIEW: TRACKS", `${filteredTracks.length} ITEMS${searchQuery ? ' (FILTERED)' : ''}`);
}

export async function loadAlbums() {
  await ensureTracksLoaded();
  const container = views.albums();
  if (!container) return;
  container.innerHTML = "";

  const allTracks = getTracks();
  // Group albums by title only, so compilation/soundtrack albums with multiple
  // per-track artists (e.g. "Various Artists") collapse into a single album card
  // instead of one card per artist.
  const albumIndex: Record<string, { name: string; artist: string; tracks: Track[]; cover: string | null }> = {};
  const albumKeyOf = (a?: string | null) => ((a || "Unknown Album").trim().toLowerCase());

  for (const t of allTracks) {
    const key = albumKeyOf(t.album);
    if (!albumIndex[key]) {
      albumIndex[key] = {
        name: t.album || "Unknown Album",
        artist: t.artist || "Unknown",
        tracks: [],
        cover: null,
      };
    }
    albumIndex[key].tracks.push(t);
    if (!albumIndex[key].cover && t.cover_art) albumIndex[key].cover = t.cover_art;
  }

  const albumEntries = Object.values(albumIndex);
  if (albumEntries.length === 0) {
    container.innerHTML = `<div class="no-albums">[ NO ALBUMS ]</div>`;
    return;
  }

  // Recompute each album's displayed artist: join distinct artists, or
  // "Various Artists" when a compilation has multiple different artists.
  albumEntries.forEach(album => {
    const artists = [...new Set(album.tracks.map(t => t.artist || "Unknown").filter(Boolean))];
    album.artist = artists.length > 1
      ? "Various Artists"
      : (artists[0] || "Unknown");
  });

  updateStatusBar("VIEW: ALBUMS", `${albumEntries.length} ALBUMS`);

  albumEntries.forEach(album => {
    const card = document.createElement("div");
    card.className = "album-card";
    card.innerHTML = `
      <div class="album-cover">
        ${album.cover ? `<img src="${convertFileSrc(album.cover)}" loading="lazy" decoding="async" />` : `<span class="album-cover-placeholder">[ ]</span>`}
      </div>
      <div class="album-info">
        <div class="album-title">${escapeHtml(album.name)}</div>
        <div class="album-artist">${escapeHtml(album.artist)}</div>
      </div>
    `;
    card.onclick = () => switchView('album-details', album);
    container.appendChild(card);
  });
}

export function renderAlbumDetails(album: Album) {
  setCurrentAlbumContext(album);
  const container = views.details();
  if (!container) return;

  const scrollTop = container.scrollTop;
  updateStatusBar("VIEW: ARCHIVE", `ALBUM: ${album.name.toUpperCase()}`);

  let albumYear: string | null = null;
  try {
    const years = album.tracks.map(t => t.year).filter((y): y is number => y !== null);
    if (years.length > 0) {
      const counts: Record<string, number> = {};
      years.forEach(y => { const k = String(y); counts[k] = (counts[k] || 0) + 1; });
      albumYear = Object.keys(counts).sort((a, b) => counts[b] - counts[a])[0];
    }
  } catch { albumYear = null; }

  const totalDuration = album.tracks.reduce((acc, t) => acc + (t.duration || 0), 0);

  container.innerHTML = `
    <div class="details-header">
      <div class="details-cover">
         ${album.cover ? `<img src="${convertFileSrc(album.cover)}" />` : `<div class="details-cover-placeholder">▒</div>`}
      </div>
      <div class="details-meta">
        <h2 class="details-title">${escapeHtml(album.name)}</h2>
        <h3 class="details-artist">${escapeHtml(album.artist)}</h3>
        <div class="details-meta-info">
           YEAR: ${albumYear || 'Unknown'} <br>
           TRACKS: ${album.tracks.length} <br>
           TOTAL TIME: ${formatDuration(totalDuration)}
        </div>
        <div class="action-bar">
           <button id="ad-play" class="tui-btn success action">[ PLAY ALBUM ]</button>
           <button id="ad-edit" class="tui-btn action">[ EDIT DATA ]</button>
           <button id="ad-delete" class="tui-btn danger action">[ DELETE ]</button>
           <button id="ad-back" class="tui-btn action">[ &lt; BACK ]</button>
        </div>
      </div>
    </div>
    <div class="tui-header">[ TRACKLIST ]</div>
    <div class="track-list" id="ad-tracklist"></div>
  `;

  container.querySelector("#ad-play")?.addEventListener("click", () => playTrack(album.tracks[0], album.tracks));
  container.querySelector("#ad-back")?.addEventListener("click", () => switchView("albums"));
  container.querySelector("#ad-edit")?.addEventListener("click", () => openEditModal(album));
  container.querySelector("#ad-delete")?.addEventListener("click", () => deleteAlbum(album));

  const list = container.querySelector("#ad-tracklist");
  if (list) {
    album.tracks.forEach((t, i) => {
      const playing = getIsPlaying() && isCurrentTrack(t);
      const row = document.createElement("div");
      row.className = "track-item" + (playing ? " playing" : "");
      row.innerHTML = `
         <span class="track-idx">${playing ? ">>" : (i + 1).toString().padStart(2, '0') + "."}</span>
         <span>${escapeHtml(t.title || "Untitled")}</span>
         <span>${escapeHtml(t.artist || "Unknown")}</span>
         <span>${formatDuration(t.duration || 0)}</span>
       `;
      row.onclick = () => playTrack(t, album.tracks);
      list.appendChild(row);
    });
  }

  container.scrollTop = scrollTop;
}

export async function deleteAlbum(album: Album) {
  if (!confirm(`DELETE ALBUM: "${album.name}"?\nThis will remove it from the library database.`)) return;
  try {
    await invoke("delete_album", { albumName: album.name });
    setTracks([]);
    switchView("albums");
  } catch (e) {
    console.error("Failed to delete album:", e);
    alert("Failed to delete album: " + e);
  }
}

function openEditModal(album: Album) {
  const modal = document.getElementById("edit-modal");
  if (!modal) return;
  (document.getElementById("edit-original-album") as HTMLInputElement).value = album.name;
  (document.getElementById("edit-album-title") as HTMLInputElement).value = album.name;
  (document.getElementById("edit-album-artist") as HTMLInputElement).value = album.artist;
  modal.classList.remove("hidden");
}

export function closeEditModal() {
  document.getElementById("edit-modal")?.classList.add("hidden");
}

export async function saveAlbumMetadata() {
  try {
    const originalName = (document.getElementById("edit-original-album") as HTMLInputElement).value;
    const newName = (document.getElementById("edit-album-title") as HTMLInputElement).value;
    const newArtist = (document.getElementById("edit-album-artist") as HTMLInputElement).value;

    if (!newName) return alert("Album Name is required");

    await invoke("update_album_metadata", { oldName: originalName, newName, newArtist });

    setTracks([]);
    await loadTracks();
    closeEditModal();
    alert(`UPDATED: ${originalName} -> ${newName}`);

    if (getCurrentView() === 'album-details') {
      const updatedAlbum: Album = {
        name: newName,
        artist: newArtist,
        tracks: getTracks().filter(t => t.album === newName),
        cover: getCurrentAlbumContext()?.cover || null,
      };
      renderAlbumDetails(updatedAlbum);
    } else {
      loadAlbums();
      if (getCurrentView() === 'tracks') loadTracks();
    }
  } catch (e) {
    console.error("Error saving metadata:", e);
    alert("Error saving metadata. Check console.");
  }
}
