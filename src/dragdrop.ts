import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

export function setupDragDrop() {
  listen('tauri://file-drop', (event: any) => {
    const paths = event.payload;
    if (paths && paths.length) handleDropPaths(paths);
  });
}

async function handleDropPaths(paths: string[]) {
  try {
    const result = await invoke("import_dropped_paths", { paths }) as string;
    alert(result);
  } catch (e) {
    console.error("Import failed:", e);
    alert("Import failed: " + e);
  }
}
