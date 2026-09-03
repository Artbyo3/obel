import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { escapeHtml } from "./utils";

export interface DriveInfo {
  letter: string;
  label: string;
  total_bytes: number;
  free_bytes: number;
  is_echo_mini: boolean;
  echo_volume: string | null;
}

interface Preview {
  total: number;
  to_copy: number;
  up_to_date: number;
  bytes_needed: number;
  free_bytes: number;
}

let devices: DriveInfo[] = [];
let selected: string | null = null;
let preview: Preview | null = null;
let previewing = false;
let syncing = false;
let unlisten: UnlistenFn[] = [];

function fmtBytes(n: number): string {
  if (n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  const val = n / Math.pow(1024, i);
  return `${val.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

function volumeLabel(d: DriveInfo): string {
  if (d.echo_volume === "sd") return "SD CARD";
  if (d.echo_volume === "internal") return "INTERNAL";
  if (d.is_echo_mini) return "ECHO MINI";
  return "USB DRIVE";
}

function el(id: string): HTMLElement | null {
  return document.getElementById(id);
}

function devicesHost(): HTMLElement | null {
  return el("sync-devices");
}

function setStatus(message: string, kind: "error" | "ok" | "info" = "info") {
  const host = el("sync-status");
  if (!host) return;
  host.textContent = message;
  host.className = `sync-status ${kind === "info" ? "" : kind}`;
  host.classList.remove("hidden");
}

function clearStatus() {
  const host = el("sync-status");
  if (host) host.classList.add("hidden");
}

function decodeError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object") {
    const anyE = e as any;
    const msg = anyE?.message || anyE?.error || String(e);
    return msg;
  }
  return String(e);
}

function renderDevices() {
  const host = devicesHost();
  if (!host) return;

  const loading = el("sync-preview");
  if (loading) loading.classList.add("hidden");

  if (syncing) {
    // Keep showing the current device card dimmed while syncing.
  }

  if (devices.length === 0) {
    host.innerHTML = `
      <div class="sync-empty">
        No removable device connected.
        <div class="sync-empty-hint">Plug in your Echo Mini (or any USB drive) and press <b>[ REFRESH ]</b>.</div>
      </div>`;
    return;
  }

  host.innerHTML = devices
    .map((d) => {
      const isSel = selected === d.letter;
      const vol = volumeLabel(d);
      const badge = d.is_echo_mini
        ? `<span class="sync-badge ${d.echo_volume === "sd" ? "sd" : ""}">${escapeHtml(vol)}</span>`
        : "";
      return `
        <div class="sync-card ${d.is_echo_mini ? "echo" : ""} ${isSel ? "selected" : ""}" data-letter="${escapeHtml(d.letter)}" role="button">
          <div class="sync-card-info">
            <div class="sync-volume-line">
              <span class="sync-letter">(${escapeHtml(d.letter)})</span>
              <span class="sync-label">${escapeHtml(d.label || "(no label)")}</span>
              ${badge}
            </div>
            <div class="sync-meta">${fmtBytes(d.free_bytes)} free / ${fmtBytes(d.total_bytes)} total</div>
          </div>
        </div>`;
    })
    .join("");
}

function renderPreview() {
  const host = el("sync-preview");
  if (!host) return;

  if (!selected || !preview) {
    host.classList.add("hidden");
    return;
  }

  const enough = preview.bytes_needed <= preview.free_bytes;
  const spaceLine = enough
    ? `Needs ${fmtBytes(preview.bytes_needed)} — fits in ${fmtBytes(preview.free_bytes)} free.`
    : `Needs ${fmtBytes(preview.bytes_needed)} but only ${fmtBytes(preview.free_bytes)} free — NOT ENOUGH SPACE.`;

  host.classList.remove("hidden");
  host.innerHTML = `
    <div class="tui-header">SYNC PLAN -> (${escapeHtml(selected)})</div>
    <div class="sync-preview-grid">
      <div class="sync-preview-row"><span class="label">Will copy</span><span class="value">${preview.to_copy}</span></div>
      <div class="sync-preview-row"><span class="label">Already up to date</span><span class="value">${preview.up_to_date}</span></div>
      <div class="sync-preview-row"><span class="label">Total tracks</span><span class="value">${preview.total}</span></div>
      <div class="sync-preview-row"><span class="label">Estimated size</span><span class="value">${fmtBytes(preview.bytes_needed)}</span></div>
    </div>
    <div class="${enough ? "sync-meta" : "sync-space-warning"} sync-preview-space">${escapeHtml(spaceLine)}</div>
    <div class="sync-preview-actions">
      <button class="tui-btn success action" id="sync-confirm-btn" ${!enough || previewing || syncing ? "disabled" : ""}>[ SYNC NOW ]</button>
      <button class="tui-btn action" id="sync-cancel-btn">[ CANCEL ]</button>
    </div>
  `;

  el("sync-cancel-btn")?.addEventListener("click", () => {
    selected = null;
    preview = null;
    host.classList.add("hidden");
    renderDevices();
  });

  const confirm = el("sync-confirm-btn");
  confirm?.addEventListener("click", () => {
    if (selected) void runSync(selected);
  });
}

async function runSync(letter: string) {
  syncing = true;
  previewing = false;
  selected = letter;
  clearStatus();
  renderDevices();
  renderPreview();

  // Show live area (progress starts at 0/0 until first event).
  const live = el("sync-live");
  if (live) live.classList.remove("hidden");
  setLiveProgress(0, 1, "Preparing...");

  try {
    const res = await invoke<{ copied: number; skipped: number; total: number }>("sync_to_device", {
      driveLetter: letter,
    });
    finishLive(true, `Done — copied ${res.copied}, skipped ${res.skipped}, ${res.total} total.`);
  } catch (e) {
    finishLive(false, `Sync failed: ${decodeError(e)}`);
  } finally {
    syncing = false;
    // Refresh the plan preview so it reflects the new post-sync state, but
    // don't wipe the completion message shown above.
    await refreshPreviewOnly(letter);
    renderDevices();
  }
}

function setLiveProgress(done: number, total: number, title: string) {
  const fill = el("sync-progress-fill") as HTMLElement | null;
  const count = el("sync-live-count");
  const track = el("sync-live-track");
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
  if (fill) fill.style.width = `${pct}%`;
  if (count) count.textContent = `${done}/${total}`;
  if (track) track.textContent = title || "\u00A0";
}

function finishLive(ok: boolean, message: string) {
  const live = el("sync-live");
  if (live) live.classList.add("hidden");
  const fill = el("sync-progress-fill") as HTMLElement | null;
  if (fill) fill.style.width = "0%";
  setStatus(message, ok ? "ok" : "error");
}

async function previewDevice(letter: string) {
  if (previewing || syncing) return;
  previewing = true;
  selected = letter;
  preview = null;
  clearStatus();
  renderDevices();
  renderPreview();
  await fetchPreview(letter);
  previewing = false;
  renderPreview();
}

// Fetch the preview plan and show it, without touching the status message.
// Used after a sync completes so the "Done" confirmation isn't cleared.
async function refreshPreviewOnly(letter: string): Promise<void> {
  await fetchPreview(letter);
  renderPreview();
}

async function fetchPreview(letter: string) {
  try {
    preview = await invoke<Preview>("preview_sync", { driveLetter: letter });
  } catch (e) {
    selected = null;
    setStatus(`Could not read device: ${decodeError(e)}`, "error");
  }
}

export async function refreshSync(): Promise<void> {
  clearStatus();
  const host = devicesHost();
  if (host) host.innerHTML = `<div class="sync-loading">Detecting device...</div>`;
  try {
    devices = await invoke<DriveInfo[]>("get_sync_status");
  } catch (e) {
    devices = [];
    setStatus(`Device scan failed: ${decodeError(e)}`, "error");
  }
  // Drop selection/preview if the device disappeared.
  if (selected && !devices.some((d) => d.letter === selected)) {
    selected = null;
    preview = null;
  }
  renderDevices();
  renderPreview();
}

export async function setupSync() {
  // Global click delegation for device cards + refresh button.
  document.addEventListener("click", (e) => {
    const target = e.target as HTMLElement;
    if (target.closest("#sync-refresh-btn")) {
      void refreshSync();
      return;
    }
    const card = target.closest<HTMLElement>("[data-letter]");
    if (card && !syncing && !previewing) {
      const letter = card.dataset.letter!;
      if (selected === letter) return;
      void previewDevice(letter);
    }
  });

  unlisten.push(
    await listen<number>("sync-started", (event) => {
      const live = el("sync-live");
      if (live) live.classList.remove("hidden");
      const total = Number(event.payload) || 1;
      setLiveProgress(0, total, "Starting...");
    })
  );

  unlisten.push(
    await listen<[number, number, string]>("sync-progress", (event) => {
      const [done, total, title] = event.payload;
      setLiveProgress(done, total, title);
    })
  );

  unlisten.push(
    await listen<[number, number]>("sync-finished", (event) => {
      const [copied, skipped] = event.payload;
      const total = copied + skipped;
      finishLive(true, `Done — copied ${copied}, skipped ${skipped}, ${total} total.`);
    })
  );

  await refreshSync();
}
