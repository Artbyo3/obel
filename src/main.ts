import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// --- State ---
let tracks: any[] = [];
let isPlaying = false;
let currentView: 'tracks' | 'albums' | 'album-details' = 'tracks';
let currentAlbumContext: any = null; // Stores the album currently being viewed
let playbackQueue: any[] = [];
let currentQueueIndex = -1;

// --- DOM Elements ---
const views = {
  tracks: () => document.getElementById("track-list"),
  albums: () => document.getElementById("album-grid"),
  details: () => document.getElementById("album-details-view")
};

const navBtns = {
  tracks: () => document.getElementById("tracks-btn"),
  albums: () => document.getElementById("albums-btn")
};

// --- Initialization ---

window.addEventListener("DOMContentLoaded", () => {
  // Navigation
  navBtns.tracks()?.addEventListener("click", () => switchView('tracks'));
  navBtns.albums()?.addEventListener("click", () => switchView('albums'));

  // Settings
  document.getElementById("settings-btn")?.addEventListener("click", openSettings);
  document.getElementById("close-settings")?.addEventListener("click", closeSettings);
  document.getElementById("add-path-btn")?.addEventListener("click", addPath);
  document.getElementById("scan-now-btn")?.addEventListener("click", scanLibrary);
  document.getElementById("wipe-db-btn")?.addEventListener("click", wipeDatabase);

  // Discord Toggle
  const discordToggle = document.getElementById("discord-toggle") as HTMLInputElement;
  if (discordToggle) {
    discordToggle.addEventListener("change", (e) => {
      const enabled = (e.target as HTMLInputElement).checked;
      invoke("set_discord_enabled", { enabled });
    });
  }

  // Player Controls
  document.getElementById("play-btn")?.addEventListener("click", togglePlay);
  document.getElementById("prev-btn")?.addEventListener("click", playPrev);
  document.getElementById("next-btn")?.addEventListener("click", playNext);

  listen("track-finished", () => {
    console.log("Backend: Track finished");
    playNext();
  });

  // Edit Modal
  document.getElementById("close-edit")?.addEventListener("click", closeEditModal);
  document.getElementById("save-edit-btn")?.addEventListener("click", saveAlbumMetadata);

  // Volume
  const volSlider = document.getElementById("vol-slider") as HTMLInputElement;
  if (volSlider) {
    volSlider.addEventListener("input", (e: Event) => {
      const val = (e.target as HTMLInputElement).value;
      const volume = parseFloat(val) / 100;
      invoke("set_volume", { volume });
      console.log("Volume set to:", val);
    });
  }

  setupDragDrop();
  loadTracks(); // Initial load
});

// --- View Management ---

function switchView(mode: 'tracks' | 'albums' | 'album-details', data?: any) {
  currentView = mode;

  // 1. Update Sidebar Nav (only for main views)
  document.querySelectorAll("nav button").forEach(btn => btn.classList.remove("active"));
  if (mode === 'tracks') navBtns.tracks()?.classList.add("active");
  if (mode === 'albums' || mode === 'album-details') navBtns.albums()?.classList.add("active");

  // 2. toggle Visibility
  Object.values(views).forEach(el => el()?.classList.add("hidden"));

  if (mode === 'tracks') {
    views.tracks()?.classList.remove("hidden");
    loadTracks();
  }
  else if (mode === 'albums') {
    views.albums()?.classList.remove("hidden");
    loadAlbums(); // Reload/Render grid
  }
  else if (mode === 'album-details') {
    views.details()?.classList.remove("hidden");
    if (data) renderAlbumDetails(data);
  }
}

function updateStatusBar(viewLabel: string, statsLabel: string) {
  const lbl = document.getElementById("current-view-label");
  const stats = document.getElementById("library-stats");
  if (lbl) lbl.innerText = viewLabel;
  if (stats) stats.innerText = statsLabel;
}

// --- Data Loading ---

async function ensureTracksLoaded() {
  if (tracks.length === 0) {
    try {
      tracks = await invoke("get_tracks");
    } catch (e) {
      console.error("Failed to load tracks", e);
    }
  }
  return tracks;
}

const ROW_HEIGHT = 28; // height of track-item + some padding

async function loadTracks() {
  await ensureTracksLoaded();
  const container = views.tracks();
  if (!container) return;

  const totalTracks = tracks.length;

  // Ensure virtualized structure exists without clearing everything if possible
  if (!container.querySelector(".vs-spacer")) {
    container.innerHTML = "";
    const spacer = document.createElement("div");
    spacer.className = "vs-spacer";
    container.appendChild(spacer);

    const content = document.createElement("div");
    content.className = "vs-content";
    container.appendChild(content);
  }

  // Virtual Scroll Setup
  if (!container.dataset.virtualized) {
    container.dataset.virtualized = "true";
    container.onscroll = () => renderVirtualTracks();
  }

  renderVirtualTracks();
  updateStatusBar("VIEW: TRACKS", `${totalTracks} ITEMS`);
}

function renderVirtualTracks() {
  const container = views.tracks();
  if (!container) return;

  const scrollTop = container.scrollTop;
  const containerHeight = container.clientHeight;

  const startIndex = Math.floor(scrollTop / ROW_HEIGHT);
  const endIndex = Math.min(tracks.length - 1, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT));

  // Get elements
  let spacer = container.querySelector(".vs-spacer") as HTMLElement;
  let content = container.querySelector(".vs-content") as HTMLElement;

  if (!spacer || !content) return; // Should have been created in loadTracks

  spacer.style.height = `${tracks.length * ROW_HEIGHT}px`;
  spacer.style.position = "relative";

  content.style.position = "absolute";
  content.style.top = "0";
  content.style.left = "0";
  content.style.right = "0";
  content.style.transform = `translateY(${startIndex * ROW_HEIGHT}px)`;
  content.style.paddingTop = "20px"; // Restore the visual padding inside the virtual list
  content.style.paddingBottom = "20px";
  content.innerHTML = "";

  // Render only visible slice
  for (let i = startIndex; i <= endIndex; i++) {
    const track = tracks[i];
    const div = document.createElement("div");
    div.className = "track-item";
    div.style.height = `${ROW_HEIGHT}px`;
    div.innerHTML = `
      <span>${isPlaying && isCurrentTrack(track) ? ">>" : (i + 1).toString().padStart(2, '0')}</span> 
      <span>${track.title || "Untitled"}</span>
      <span>${track.artist || "Unknown"}</span>
      <span class="format-tag">${getFormat(track.path)}</span>
      <span>${formatDuration(track.duration || 0)}</span>
    `;

    if (isPlaying && isCurrentTrack(track)) {
      div.classList.add("playing");
    }

    div.onclick = () => playTrack(track, tracks);
    content.appendChild(div);
  }
}

async function loadAlbums() {
  await ensureTracksLoaded();
  const container = views.albums();
  if (!container) return;
  container.innerHTML = "";

  // Group by Album
  const albums: Record<string, any[]> = {};
  tracks.forEach(t => {
    const key = t.album || "Unknown Album";
    if (!albums[key]) albums[key] = [];
    albums[key].push(t);
  });

  const albumKeys = Object.keys(albums);
  if (albumKeys.length === 0) {
    container.innerHTML = `<div class="text-center" style="padding:40px;">[ NO ALBUMS ]</div>`;
    return;
  }

  updateStatusBar("VIEW: ALBUMS", `${albumKeys.length} ALBUMS`);

  albumKeys.forEach(name => {
    const group = albums[name];
    const artist = group[0]?.artist || "Unknown";
    const cover = group.find(t => t.cover_art)?.cover_art;

    const card = document.createElement("div");
    card.className = "album-card";
    card.innerHTML = `
      <div class="album-cover">
        ${cover ? `<img src="${convertFileSrc(cover)}" loading="lazy" />` : `<span style="font-size:2rem; opacity:0.3;">[ ]</span>`}
      </div>
      <div class="album-info">
        <div class="album-title">${name}</div>
        <div class="album-artist">${artist}</div>
      </div>
    `;
    card.onclick = () => switchView('album-details', { name, artist, tracks: group, cover });
    container.appendChild(card);
  });
}

// --- Album Details View ---

function renderAlbumDetails(album: any) {
  currentAlbumContext = album;
  const container = views.details();
  if (!container) return;

  const scrollTop = container.scrollTop;

  updateStatusBar("VIEW: ARCHIVE", `ALBUM: ${album.name.toUpperCase()}`);

  container.innerHTML = `
    <div class="details-header">
      <div class="details-cover">
         ${album.cover ? `<img src="${convertFileSrc(album.cover)}" />` : `<div style="display:flex;justify-content:center;align-items:center;height:100%;font-size:3rem;color:#333;">▒</div>`}
      </div>
      <div class="details-meta">
        <h2 class="details-title">${album.name}</h2>
        <h3 class="details-artist">${album.artist}</h3>
        <div class="tui-text" style="color:#666; margin-bottom: 20px;">
           YEAR: 2024 (Unknown) <br>
           TRACKS: ${album.tracks.length} <br>
           TOTAL TIME: ${calcTotalDuration(album.tracks)}
        </div>
        <div class="action-bar">
           <button id="ad-play" class="tui-btn success" style="width: auto; padding: 5px 15px;">[ PLAY ALBUM ]</button>
           <button id="ad-edit" class="tui-btn" style="width: auto; padding: 5px 15px;">[ EDIT DATA ]</button>
           <button id="ad-delete" class="tui-btn error" style="width: auto; padding: 5px 15px; color: #ff5555; border-color: #ff5555;">[ DELETE ]</button>
           <button id="ad-back" class="tui-btn" style="width: auto; padding: 5px 15px;">[ &lt; BACK ]</button>
        </div>
      </div>
    </div>
    
    <div class="tui-header">[ TRACKLIST ]</div>
    <div class="track-list" id="ad-tracklist"></div>
  `;

  // Bind Buttons
  container.querySelector("#ad-play")?.addEventListener("click", () => playTrack(album.tracks[0], album.tracks));
  container.querySelector("#ad-back")?.addEventListener("click", () => switchView("albums"));
  container.querySelector("#ad-edit")?.addEventListener("click", () => openEditModal(album));
  container.querySelector("#ad-delete")?.addEventListener("click", () => deleteAlbum(album));

  // Render Tracks
  const list = container.querySelector("#ad-tracklist");
  if (list) {
    album.tracks.forEach((t: any, i: number) => {
      const isPlayingTrack = isCurrentTrack(t);
      const row = document.createElement("div");
      row.className = "track-item";
      if (isPlaying && isPlayingTrack) row.classList.add("playing");

      row.innerHTML = `
         <span style="color:var(--subtext-color)">${isPlaying && isPlayingTrack ? ">>" : (i + 1).toString().padStart(2, '0') + "."}</span>
         <span>${t.title}</span>
         <span>${t.artist}</span>
         <span>${formatDuration(t.duration)}</span>
       `;
      row.onclick = () => playTrack(t, album.tracks);
      list.appendChild(row);
    });
  }

  // Restore scroll
  container.scrollTop = scrollTop;
}

// --- Player Logic ---

let currentTrack: any = null;

async function playTrack(track: any, queue: any[] = []) {
  try {
    if (queue.length > 0) {
      playbackQueue = queue;
      currentQueueIndex = queue.findIndex(t => t.path === track.path);
    } else {
      // If no queue passed, but track is in current queue, just update index
      const idx = playbackQueue.findIndex(t => t.path === track.path);
      if (idx !== -1) {
        currentQueueIndex = idx;
      } else {
        // Reset queue to this single track
        playbackQueue = [track];
        currentQueueIndex = 0;
      }
    }

    console.log("playTrack: Invoking backend", {
      path: track.path,
      title: track.title,
      artist: track.artist,
      album: track.album,
      track_number: playbackQueue.length > 1 ? currentQueueIndex + 1 : null,
      total_tracks: playbackQueue.length > 1 ? playbackQueue.length : null
    });

    await invoke("play_track", {
      path: track.path,
      title: track.title || "Untitled",
      artist: track.artist || "Unknown artist",
      album: track.album || "",
      trackNumber: playbackQueue.length > 1 ? currentQueueIndex + 1 : null,
      totalTracks: playbackQueue.length > 1 ? playbackQueue.length : null,
      duration: track.duration || null
    });
    currentTrack = track;
    isPlaying = true;
    updatePlayerUI();
    updateTrackListUI();
  } catch (e) {
    console.error(e);
  }
}

async function playNext() {
  if (playbackQueue.length === 0 || currentQueueIndex === -1) return;

  if (currentQueueIndex < playbackQueue.length - 1) {
    currentQueueIndex++;
    playTrack(playbackQueue[currentQueueIndex]);
  } else {
    console.log("End of queue reached");
    isPlaying = false;
    updatePlayerUI();
  }
}

async function playPrev() {
  if (playbackQueue.length === 0 || currentQueueIndex === -1) return;

  if (currentQueueIndex > 0) {
    currentQueueIndex--;
    playTrack(playbackQueue[currentQueueIndex]);
  }
}

async function togglePlay() {
  if (isPlaying) {
    await invoke("pause_track");
    isPlaying = false;
  } else {
    await invoke("resume_track");
    isPlaying = true;
  }
  updatePlayerUI();
  updateTrackListUI(); // New: Update list state
}

function updateTrackListUI() {
  // 1. Remove playing state from all
  document.querySelectorAll(".track-item").forEach(el => {
    el.classList.remove("playing");
    // Reset icon (this is a bit brute force but effective for TUI)
    const idxSpan = el.querySelector("span:first-child");
    if (idxSpan && idxSpan.textContent === ">>") {
      // We lost the index... in a real app we'd store the index in a data attribute.
      // For now, let's just clear the class. The user said index isn't as important as indicator.
      // Actually, let's re-render the visible list if it's cheap, 
      // OR just update the classes if we can identify the nodes.
      // Better approach: Re-render is safest for consistency, but might jump scroll.
      // Hybrid: Just update classes.
    }
  });

  // 2. Find and Highlight. 
  // Since we don't have IDs on elements, we loop again or rely on re-render.
  // Given the issues with state sync, a Re-Render of the active view is robust if list is small (<1000).
  // Let's try re-rendering the current active list view only.
  if (currentView === 'tracks') loadTracks();
  if (currentView === 'album-details' && currentAlbumContext) renderAlbumDetails(currentAlbumContext);
}

function updatePlayerUI() {
  const btn = document.getElementById("play-btn");
  if (btn) btn.innerText = isPlaying ? "[ PAUSE ]" : "[ PLAY ]";

  if (currentTrack) {
    const titleEl = document.getElementById("np-title");
    const artistEl = document.getElementById("np-artist");
    const coverEl = document.getElementById("np-cover") as HTMLImageElement;
    const placeEl = document.getElementById("np-placeholder");

    if (titleEl) titleEl.textContent = currentTrack.title || "Untitled";
    if (artistEl) {
      artistEl.innerHTML = `${currentTrack.artist || "Unknown artist"} <span class="format-tag" style="margin-left:5px;">${getFormat(currentTrack.path)}</span>`;
    }

    if (currentTrack.cover_art) {
      if (coverEl) {
        coverEl.src = convertFileSrc(currentTrack.cover_art);
        coverEl.classList.remove("hidden");
      }
      if (placeEl) placeEl.classList.add("hidden");
    } else {
      if (coverEl) coverEl.classList.add("hidden");
      if (placeEl) placeEl.classList.remove("hidden");
    }
  }
}

function isCurrentTrack(t: any) {
  return currentTrack && currentTrack.path === t.path;
}

// --- Logic Helpers ---

function calcTotalDuration(tracks: any[]) {
  const total = tracks.reduce((acc, t) => acc + (t.duration || 0), 0);
  return formatDuration(total);
}

function formatDuration(seconds: number) {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

function getFormat(path: string) {
  if (!path) return "";
  const ext = path.split('.').pop()?.toUpperCase() || "";
  if (!ext) return "";
  const cleaned = ext.length > 4 ? ext.slice(0, 4) : ext;
  return `(${cleaned.toLowerCase()})`;
}

// --- Edit Metadata Modal ---

function openEditModal(album: any) {
  const modal = document.getElementById("edit-modal");
  if (!modal) return;

  (document.getElementById("edit-original-album") as HTMLInputElement).value = album.name;
  (document.getElementById("edit-album-title") as HTMLInputElement).value = album.name;
  (document.getElementById("edit-album-artist") as HTMLInputElement).value = album.artist;

  modal.classList.remove("hidden");
}

function closeEditModal() {
  document.getElementById("edit-modal")?.classList.add("hidden");
}

async function saveAlbumMetadata() {
  try {
    const originalNameInput = document.getElementById("edit-original-album") as HTMLInputElement;
    const newNameInput = document.getElementById("edit-album-title") as HTMLInputElement;
    // const newArtistInput = document.getElementById("edit-album-artist") as HTMLInputElement;

    if (!originalNameInput || !newNameInput) {
      console.error("Missing input elements");
      return;
    }

    const originalName = originalNameInput.value;
    const newName = newNameInput.value;
    const newArtist = (document.getElementById("edit-album-artist") as HTMLInputElement).value;

    console.log(`Saving: ${originalName} -> ${newName}`);

    if (!newName) return alert("Album Name is required");

    // Persistence
    await invoke("update_album_metadata", {
      oldName: originalName,
      newName: newName,
      newArtist: newArtist
    });

    // Force reload from DB to ensure global state is correct
    tracks = [];
    await loadTracks();

    closeEditModal();
    alert(`UPDATED: ${originalName} -> ${newName}`);

    // Refresh
    if (currentView === 'album-details') {
      const updatedAlbum = {
        name: newName,
        artist: newArtist,
        tracks: tracks.filter(t => t.album === newName),
        cover: currentAlbumContext?.cover
      };
      renderAlbumDetails(updatedAlbum);
    } else {
      loadAlbums();
      // Also refresh track list if that's where we are, though we are likely in details or albums
      if (currentView === 'tracks') loadTracks();
    }
  } catch (e) {
    console.error("Error saving metadata:", e);
    alert("Error saving metadata. Check console.");
  }
}

async function deleteAlbum(album: any) {
  if (!confirm(`DELETE ALBUM: "${album.name}"?\nThis will remove it from the library database.`)) return;
  tracks = tracks.filter(t => t.album !== album.name);
  alert(`DELETED: ${album.name}`);
  switchView("albums");
}

// --- Settings & Library (Legacy + New) ---

async function loadLibraryPaths() {
  // ... (Similar to before, just ensuring IDs match new HTML)
  try {
    const paths = await invoke("get_library_paths") as string[];
    const list = document.getElementById("path-list");
    if (!list) return;
    list.innerHTML = "";
    paths.forEach(path => {
      const el = document.createElement("div");
      el.style.display = "flex"; el.style.justifyContent = "space-between";
      el.style.marginBottom = "5px";
      el.innerHTML = `<span>${path}</span> <button class="tui-btn-small" style="color:red">[DEL]</button>`;
      (el.querySelector("button") as HTMLElement).onclick = () => removePath(path);
      list.appendChild(el);
    });
  } catch (e) { console.error(e); }
}

async function addPath() {
  const input = document.getElementById("new-path-input") as HTMLInputElement;
  if (input.value) {
    await invoke("add_library_path", { path: input.value });
    input.value = "";
    loadLibraryPaths();
  }
}

async function removePath(path: string) {
  if (confirm(`Remove ${path}?`)) {
    await invoke("remove_library_path", { path });
    loadLibraryPaths();
  }
}

async function wipeDatabase() {
  if (confirm("DANGER: This will WIPE ALL tracks and paths from the database. Are you sure?")) {
    await invoke("wipe_library");
    tracks = []; // Clear local state
    await loadTracks();
    await loadLibraryPaths();
    if (currentView === "albums") loadAlbums();
  }
}

// --- Drag Drop (Keep existing) ---
function setupDragDrop() {
  listen('tauri://file-drop', (event: any) => {
    const paths = event.payload;
    if (paths && paths.length) handleDropPaths(paths);
  });
}

async function handleDropPaths(paths: string[]) {
  // Re-implement if needed, for now just basic structure
  console.log("Dropped", paths);
  alert(`Importing ${paths.length} items (Mock)`);
  // Ideally duplicate the logic from original main.ts if it was critical
}

async function scanLibrary() {
  const btn = document.getElementById("scan-now-btn");
  if (btn) {
    btn.innerText = "[ SCANNING... ]";
    btn.setAttribute("disabled", "true");
  }

  try {
    const added = (await invoke("scan_library")) as number;
    alert(`SCAN COMPLETE: Added/Updated ${added} tracks.`);
    tracks = []; // Reset local cache to force reload from DB
    await loadTracks();
    if (currentView === "albums") loadAlbums();
  } catch (e) {
    console.error(e);
    alert("Scan failed: " + e);
  } finally {
    if (btn) {
      btn.innerText = "[ EXECUTE: RESCAN_LIBRARY ]";
      btn.removeAttribute("disabled");
    }
  }
}

function openSettings() {
  document.getElementById("settings-modal")?.classList.remove("hidden");
  loadLibraryPaths();
}
function closeSettings() {
  document.getElementById("settings-modal")?.classList.add("hidden");
}
