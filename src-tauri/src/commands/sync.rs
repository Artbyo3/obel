use tauri::{AppHandle, Emitter, State};
use crate::core::sync;
use crate::db::schema;
use crate::error::{AppError, AppResult};
use crate::DbState;

/// Report the current sync status: connected devices and their properties.
#[tauri::command]
pub async fn get_sync_status() -> AppResult<Vec<sync::DriveInfo>> {
    let devices = sync::detect_devices();
    Ok(devices)
}

/// Sync the entire Obel library to a connected device drive letter (e.g. "E:").
#[tauri::command]
pub async fn sync_to_device(drive_letter: String, state: State<'_, DbState>, app: AppHandle) -> AppResult<SyncSummary> {
    let devices = sync::detect_devices();
    let Some(target) = devices.iter().find(|d| d.letter == drive_letter) else {
        return Err(AppError::Custom(format!(
            "Device {} is not connected or detected. Reconnect the player and try again.",
            drive_letter
        )));
    };

    // Load the full library once.
    let tracks = {
        let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
        schema::get_all_tracks(&conn)?
    };

    let plan = sync::build_sync_plan(tracks);

    // Free space check: only count bytes we might actually need to copy.
    let needed = sync::required_space(&plan);
    if needed > target.free_bytes {
        return Err(AppError::Custom(format!(
            "Not enough free space on device: need ~{:.1} MB but only {:.1} MB free.",
            needed as f64 / 1e6,
            target.free_bytes as f64 / 1e6
        )));
    }

    let total = plan.len();
    let _ = app.emit("sync-started", total);

    // Run copy in a blocking thread pool task so the async runtime stays responsive.
    let drive_clone = target.clone();
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        sync::sync_tracks(plan, &drive_clone, |i, n, title| {
            let _ = app_for_task.emit("sync-progress", (i + 1, n, title.to_string()));
        })
    })
    .await
    .map_err(|e| AppError::Custom(format!("Sync task failed: {}", e)))?;

    let (copied, skipped) = result.map_err(AppError::Io)?;
    let _ = app.emit("sync-finished", (copied, skipped));

    Ok(SyncSummary { copied, skipped, total })
}

#[derive(serde::Serialize)]
pub struct SyncSummary {
    pub copied: usize,
    pub skipped: usize,
    pub total: usize,
}
