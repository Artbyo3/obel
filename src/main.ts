import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { playNext, playPrev, togglePlay, setOnViewUpdate, formatDuration } from "./player";
import { switchView, loadTracks, renderAlbumDetails, closeEditModal, saveAlbumMetadata } from "./views";
import { setupSearch } from "./search";
import { setupSettings } from "./settings";
import { setupDragDrop } from "./dragdrop";
import { toggleLyricsSidebar, updateLyricHighlight } from "./lyrics";
import { getCurrentTrack, getCurrentView, getCurrentAlbumContext, ensureTracksLoaded, getTracks } from "./state";

window.addEventListener("DOMContentLoaded", () => {
  document.getElementById("tracks-btn")?.addEventListener("click", () => switchView('tracks'));
  document.getElementById("albums-btn")?.addEventListener("click", () => switchView('albums'));
  document.getElementById("play-btn")?.addEventListener("click", togglePlay);
  document.getElementById("prev-btn")?.addEventListener("click", playPrev);
  document.getElementById("next-btn")?.addEventListener("click", playNext);
  document.getElementById("toggle-lyrics-btn")?.addEventListener("click", toggleLyricsSidebar);
  document.getElementById("close-edit")?.addEventListener("click", closeEditModal);
  document.getElementById("save-edit-btn")?.addEventListener("click", saveAlbumMetadata);

  const discordToggle = document.getElementById("discord-toggle") as HTMLInputElement;
  if (discordToggle) {
    discordToggle.addEventListener("change", (e) => {
      invoke("set_discord_enabled", { enabled: (e.target as HTMLInputElement).checked });
    });
  }

  const volSlider = document.getElementById("vol-slider") as HTMLInputElement;
  if (volSlider) {
    volSlider.addEventListener("input", (e: Event) => {
      invoke("set_volume", { volume: parseFloat((e.target as HTMLInputElement).value) / 100 });
    });
  }

  const progressBar = document.getElementById("progress-bar") as HTMLInputElement;
  if (progressBar) {
    progressBar.addEventListener("change", (e: Event) => {
      invoke("seek_track", { seconds: parseFloat((e.target as HTMLInputElement).value) });
    });
    progressBar.addEventListener("input", (e: Event) => {
      const currTimeEl = document.getElementById("curr-time");
      if (currTimeEl) currTimeEl.textContent = formatDuration(parseFloat((e.target as HTMLInputElement).value));
    });
  }

  listen("track-finished", () => playNext());
  listen("playback-progress", (event: any) => {
    const elapsed = event.payload as number;
    const bar = document.getElementById("progress-bar") as HTMLInputElement;
    const currTimeEl = document.getElementById("curr-time");
    if (bar) bar.value = elapsed.toString();
    if (currTimeEl) currTimeEl.textContent = formatDuration(elapsed);
    updateLyricHighlight(elapsed);
  });

  // Cover art click -> navigate to album
  const coverArt = document.getElementById("np-cover");
  if (coverArt) {
    coverArt.style.cursor = "pointer";
    coverArt.addEventListener("click", async () => {
      const ct = getCurrentTrack();
      if (!ct?.album) return;
      await ensureTracksLoaded();

      const albumsMap: Record<string, any> = {};
      getTracks().forEach(t => {
        const key = `${(t.album || '').toString()}||${(t.artist || '').toString()}`;
        if (!albumsMap[key]) albumsMap[key] = { name: t.album || 'Unknown Album', artist: t.artist || 'Unknown', tracks: [], cover: t.cover_art };
        albumsMap[key].tracks.push(t);
        if (!albumsMap[key].cover && t.cover_art) albumsMap[key].cover = t.cover_art;
      });

      const lookupKey = `${ct.album}||${ct.artist}`;
      let album = albumsMap[lookupKey];
      if (!album) {
        const lowerName = (ct.album || '').toString().toLowerCase();
        for (const k in albumsMap) {
          if (albumsMap[k].name?.toString().toLowerCase() === lowerName) {
            album = albumsMap[k];
            break;
          }
        }
      }
      if (album) switchView('album-details', album);
    });
  }

  // Let player trigger view re-renders
  setOnViewUpdate(() => {
    const view = getCurrentView();
    if (view === 'tracks') loadTracks();
    if (view === 'album-details' && getCurrentAlbumContext()) renderAlbumDetails(getCurrentAlbumContext()!);
  });

  setupSearch();
  setupSettings();
  setupDragDrop();
  loadTracks();
});
