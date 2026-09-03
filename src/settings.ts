import { invoke } from "@tauri-apps/api/core";
import { setTracks } from "./state";
import { loadTracks, loadAlbums } from "./views";
import { getCurrentView } from "./state";
import { escapeHtml } from "./utils";

export function setupSettings() {
  document.getElementById("settings-btn")?.addEventListener("click", openSettings);
  document.getElementById("close-settings")?.addEventListener("click", closeSettings);
  document.getElementById("add-path-btn")?.addEventListener("click", addPath);
  document.getElementById("scan-now-btn")?.addEventListener("click", scanLibrary);
  document.getElementById("wipe-db-btn")?.addEventListener("click", wipeDatabase);
  document.getElementById("youtube-download-btn")?.addEventListener("click", downloadYoutubeMusic);
  setupSettingsTabs();
}

export async function loadLibraryPaths() {
  try {
    const paths = await invoke("get_library_paths") as string[];
    const list = document.getElementById("path-list");
    if (!list) return;
    list.innerHTML = "";
    paths.forEach(path => {
      const el = document.createElement("div");
      el.className = "path-item";
      el.innerHTML = `<span>${escapeHtml(path)}</span> <button class="tui-btn-small del-btn">[DEL]</button>`;
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
    setTracks([]);
    await loadTracks();
    await loadLibraryPaths();
    if (getCurrentView() === "albums") loadAlbums();
  }
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
    setTracks([]);
    await loadTracks();
    if (getCurrentView() === "albums") loadAlbums();
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

async function downloadYoutubeMusic() {
  const urlInput = document.getElementById("youtube-url-input") as HTMLInputElement | null;
  const folderInput = document.getElementById("youtube-download-folder") as HTMLInputElement | null;
  const statusEl = document.getElementById("youtube-download-status");

  const url = urlInput?.value.trim() ?? "";
  const destination = folderInput?.value.trim() ?? "";

  if (!url) {
    if (statusEl) statusEl.textContent = "Please paste a YouTube URL first.";
    return;
  }
  if (!destination) {
    if (statusEl) statusEl.textContent = "Please enter a download folder.";
    return;
  }

  try {
    if (statusEl) statusEl.textContent = "Downloading via yt-dlp...";
    const result = await invoke("download_from_youtube", { url, destination });

    try {
      const currentPaths = await invoke("get_library_paths") as string[];
      if (!currentPaths.includes(destination)) {
        await invoke("add_library_path", { path: destination });
      }
      await invoke("scan_library");
      setTracks([]);
      await loadTracks();
      if (getCurrentView() === "albums") loadAlbums();
    } catch (scanErr) {
      console.warn("Download succeeded but scan failed", scanErr);
    }

    if (statusEl) statusEl.textContent = String(result);
  } catch (error) {
    console.error("YouTube download failed", error);
    if (statusEl) statusEl.textContent = "Download failed: " + String(error);
  }
}

function openSettings() {
  document.getElementById("settings-modal")?.classList.remove("hidden");
  loadLibraryPaths();
}

function closeSettings() {
  document.getElementById("settings-modal")?.classList.add("hidden");
}

function setupSettingsTabs() {
  const tabs = Array.from(document.querySelectorAll('.settings-tab')) as HTMLElement[];
  const sections = Array.from(document.querySelectorAll('.settings-section')) as HTMLElement[];

  function showSection(name: string) {
    tabs.forEach(t => t.classList.toggle('active', t.dataset.section === name));
    sections.forEach(s => s.classList.toggle('hidden', s.id !== `settings-${name}`));
  }

  tabs.forEach(tab => {
    tab.addEventListener('click', () => showSection(tab.dataset.section || 'library'));
  });

  showSection('library');
}
