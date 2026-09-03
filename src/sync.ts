import { invoke } from "@tauri-apps/api/core";
import { escapeHtml } from "./utils";

export interface DriveInfo {
  letter: string;
  label: string;
  total_bytes: number;
  free_bytes: number;
  is_echo_mini: boolean;
  echo_volume: string | null;
}

let devices: DriveInfo[] = [];
let syncing = false;
let pollTimer: number | null = null;

function fmtBytes(n: number): string {
  if (n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  const val = n / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function statusEl(): HTMLElement | null {
  return document.getElementById("sync-status");
}

function devicesEl(): HTMLElement | null {
  return document.getElementById("sync-devices");
}

function setStatus(msg: string) {
  const el = statusEl();
  if (el) el.textContent = msg;
}

function render() {
  const host = devicesEl();
  if (!host) return;

  if (devices.length === 0) {
    host.innerHTML = `<div class="sub-text">No removable device detected. Connect your Echo Mini and wait a moment.</div>`;
    return;
  }

  host.innerHTML = devices
    .map(
      (d) => `
      <div class="tui-field" style="display:flex; justify-content:space-between; align-items:center; gap:8px;">
        <div style="flex:1;">
          <span class="tui-text">${escapeHtml(d.letter)}</span>
          <span class="sub-text" style="margin-left:8px;">${escapeHtml(d.label || "(no label)")}</span>
          ${d.is_echo_mini ? `<span class="sub-text" style="color: var(--accent-color); margin-left:6px;">[ECHO MINI]</span>` : ""}
          <div class="sub-text">${fmtBytes(d.free_bytes)} free / ${fmtBytes(d.total_bytes)} total</div>
        </div>
        <button class="tui-btn" data-sync="${escapeHtml(d.letter)}" ${syncing ? "disabled" : ""}>[ SYNC ]</button>
      </div>`
    )
    .join("");
}

async function refreshSilently(): Promise<void> {
  try {
    devices = await invoke<DriveInfo[]>("get_sync_status");
  } catch {
    devices = [];
  }
  render();
}

export function setupSync() {
  void refreshSilently();

  document.addEventListener("click", async (e) => {
    const btn = (e.target as HTMLElement).closest("[data-sync]") as HTMLElement | null;
    if (!btn) return;
    const letter = btn.dataset.sync;
    if (!letter || syncing) return;
    syncing = true;
    render();
    setStatus(`Syncing to ${letter}...`);
    try {
      const res = await invoke<{ copied: number; skipped: number; total: number }>("sync_to_device", {
        driveLetter: letter,
      });
      setStatus(`Done: copied ${res.copied}, skipped ${res.skipped} (${res.total} total).`);
    } catch (err) {
      setStatus(`Sync failed: ${String(err)}`);
    } finally {
      syncing = false;
      render();
    }
  });
}

// Lightweight periodic refresh to pick up hot-plug/unplug events while the app runs.
export function startSyncPolling() {
  if (pollTimer !== null) return;
  pollTimer = window.setInterval(() => { void refreshSilently(); }, 5000);
}

export function stopSyncPolling() {
  if (pollTimer !== null) {
    window.clearInterval(pollTimer);
    pollTimer = null;
  }
}
