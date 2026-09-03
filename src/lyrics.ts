import { invoke } from "@tauri-apps/api/core";
import { LyricLine } from "./types";
import { getCurrentTrack } from "./state";

let lyricsVisible = false;
let parsedLyrics: LyricLine[] = [];
let currentLyricIndex = -1;

export function toggleLyricsSidebar() {
  const lyricsView = document.getElementById("lyrics-sidebar-view");
  const coverImg = document.getElementById("np-cover");
  const placeholder = document.getElementById("np-placeholder");
  if (!lyricsView) return;

  lyricsVisible = !lyricsVisible;

  if (lyricsVisible) {
    lyricsView.classList.remove("hidden");
    coverImg?.classList.add("hidden");
    placeholder?.classList.add("hidden");
    loadLyrics();
  } else {
    lyricsView.classList.add("hidden");
    const coverImgEl = coverImg as HTMLImageElement;
    const ct = getCurrentTrack();
    if (ct?.cover_art && coverImgEl) {
      coverImgEl.classList.remove("hidden");
      placeholder?.classList.add("hidden");
    } else {
      coverImgEl?.classList.add("hidden");
      placeholder?.classList.remove("hidden");
    }
  }
}

export function getLyricsVisible() { return lyricsVisible; }

export async function loadLyrics() {
  const lyricsView = document.getElementById("lyrics-sidebar-view");
  if (!lyricsView) return;

  const ct = getCurrentTrack();
  if (!ct) {
    lyricsView.innerHTML = "<div class='lyric-state'>[NO_TRACK]</div>";
    parsedLyrics = [];
    return;
  }

  lyricsView.innerHTML = "<div class='lyric-state'>[LOADING...]</div>";
  try {
    const lyricsText = await invoke("get_lyrics", { path: ct.path }) as string;
    parsedLyrics = parseLRC(lyricsText);
    renderLyrics();
  } catch {
    lyricsView.innerHTML = "<div class='lyric-state'>[NO_LYRICS]</div>";
    parsedLyrics = [];
  }
}

function parseLRC(text: string): LyricLine[] {
  const lines = text.split('\n');
  const parsed: LyricLine[] = [];
  const lrcRegex = /\[(\d{2}):(\d{2})\.(\d{2})\](.*)$/;

  for (const line of lines) {
    const match = line.match(lrcRegex);
    if (match) {
      const time = parseInt(match[1]) * 60 + parseInt(match[2]) + parseInt(match[3]) / 100;
      const text = match[4].trim();
      if (text) parsed.push({ time, text });
    }
  }

  if (parsed.length === 0) {
    lines.forEach(line => {
      const trimmed = line.trim();
      if (trimmed) parsed.push({ time: -1, text: trimmed });
    });
  }

  return parsed;
}

function renderLyrics() {
  const lyricsView = document.getElementById("lyrics-sidebar-view");
  if (!lyricsView || parsedLyrics.length === 0) return;

  lyricsView.innerHTML = parsedLyrics.map((lyric, idx) =>
    `<div class="lyric-line" data-index="${idx}">${lyric.text}</div>`
  ).join('');
}

export function updateLyricHighlight(currentTime: number) {
  if (!lyricsVisible || parsedLyrics.length === 0) return;
  if (parsedLyrics[0].time === -1) return;

  let activeIndex = -1;
  for (let i = parsedLyrics.length - 1; i >= 0; i--) {
    if (currentTime >= parsedLyrics[i].time) {
      activeIndex = i;
      break;
    }
  }

  if (activeIndex !== currentLyricIndex) {
    currentLyricIndex = activeIndex;
    const lines = document.querySelectorAll(".lyric-line");
    lines.forEach((line, idx) => {
      if (idx === activeIndex) {
        line.classList.add("active");
        line.scrollIntoView({ behavior: "smooth", block: "center" });
      } else {
        line.classList.remove("active");
      }
    });
  }
}
