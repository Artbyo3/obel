import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { Track } from "./types";
import {
  getCurrentTrack, setCurrentTrack,
  getIsPlaying, setIsPlaying,
  getPlaybackQueue, setPlaybackQueue,
  getCurrentQueueIndex, setCurrentQueueIndex,
} from "./state";
import { escapeHtml } from "./utils";

export function isCurrentTrack(t: Track): boolean {
  const ct = getCurrentTrack();
  return ct !== null && ct.path === t.path;
}

export async function playTrack(track: Track, queue: Track[] = []) {
  try {
    if (queue.length > 0) {
      setPlaybackQueue(queue);
      setCurrentQueueIndex(queue.findIndex(t => t.path === track.path));
    } else {
      const idx = getPlaybackQueue().findIndex(t => t.path === track.path);
      if (idx !== -1) {
        setCurrentQueueIndex(idx);
      } else {
        setPlaybackQueue([track]);
        setCurrentQueueIndex(0);
      }
    }

    const q = getPlaybackQueue();
    const i = getCurrentQueueIndex();

    await invoke("play_track", {
      path: track.path,
      title: track.title || "Untitled",
      artist: track.artist || "Unknown artist",
      album: track.album || "",
      trackNumber: q.length > 1 ? i + 1 : null,
      totalTracks: q.length > 1 ? q.length : null,
      duration: track.duration || null,
    });

    setCurrentTrack(track);
    setIsPlaying(true);
    updatePlayerUI();
    // Trigger view refresh via callback
    onViewUpdate?.();
  } catch (e) {
    console.error(e);
  }
}

export async function playNext() {
  const q = getPlaybackQueue();
  const idx = getCurrentQueueIndex();
  if (q.length === 0 || idx === -1) return;

  if (idx < q.length - 1) {
    setCurrentQueueIndex(idx + 1);
    playTrack(getPlaybackQueue()[getCurrentQueueIndex()]);
  } else {
    setIsPlaying(false);
    updatePlayerUI();
  }
}

export async function playPrev() {
  const q = getPlaybackQueue();
  const idx = getCurrentQueueIndex();
  if (q.length === 0 || idx === -1) return;

  if (idx > 0) {
    setCurrentQueueIndex(idx - 1);
    playTrack(getPlaybackQueue()[getCurrentQueueIndex()]);
  }
}

export async function togglePlay() {
  if (!getCurrentTrack()) return;
  if (getIsPlaying()) {
    await invoke("pause_track");
    setIsPlaying(false);
  } else {
    await invoke("resume_track");
    setIsPlaying(true);
  }
  updatePlayerUI();
  onViewUpdate?.();
}

export function updatePlayerUI() {
  const isPlaying = getIsPlaying();
  const currentTrack = getCurrentTrack();

  const btn = document.getElementById("play-btn");
  if (btn) btn.innerText = isPlaying ? "[ PAUSE ]" : "[ PLAY ]";

  if (currentTrack) {
    const titleEl = document.getElementById("np-title");
    const artistEl = document.getElementById("np-artist");
    const coverEl = document.getElementById("np-cover") as HTMLImageElement;
    const placeEl = document.getElementById("np-placeholder");

    if (titleEl) titleEl.textContent = currentTrack.title || "Untitled";
    if (artistEl) {
      artistEl.innerHTML = `${escapeHtml(currentTrack.artist || "Unknown artist")} <span class="format-tag" style="margin-left:5px;">${getFormat(currentTrack.path)}</span>`;
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

    const bar = document.getElementById("progress-bar") as HTMLInputElement;
    const totalTimeEl = document.getElementById("total-time");
    if (bar && currentTrack.duration) {
      bar.max = currentTrack.duration.toString();
      bar.value = "0";
    }
    if (totalTimeEl) {
      totalTimeEl.textContent = formatDuration(currentTrack.duration || 0);
    }
  }
}

export function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function getFormat(path: string): string {
  if (!path) return "";
  const ext = path.split('.').pop()?.toUpperCase() || "";
  if (!ext) return "";
  return `(${ext.length > 4 ? ext.slice(0, 4) : ext})`;
}

// Called from main to let player trigger view re-renders
let onViewUpdate: (() => void) | null = null;
export function setOnViewUpdate(fn: () => void) { onViewUpdate = fn; }
