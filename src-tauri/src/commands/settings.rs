use tauri::State;
use crate::error::{AppError, AppResult};
use crate::DbState;
use crate::AppSettings;
use crate::db::schema;

#[tauri::command]
pub async fn test_discord_rpc(settings: State<'_, AppSettings>) -> AppResult<String> {
    let mut rpc = settings.rpc.write().await;
    if let Some(ref mut rpc) = *rpc {
        rpc.ensure_connected();
        rpc.update_presence("TEST TRACK", "OBEL TEST ARTIST", None, Some(1), Some(1), None, None);
        return Ok("Test command sent to Discord. Check your status!".to_string());
    }
    Err(AppError::Custom("Discord RPC not initialized".to_string()))
}

#[tauri::command]
pub async fn set_discord_enabled(enabled: bool, settings: State<'_, AppSettings>) -> AppResult<()> {
    {
        let mut val = settings.discord_enabled.write().await;
        *val = enabled;
    }
    let mut rpc = settings.rpc.write().await;
    if let Some(ref mut rpc) = *rpc {
        rpc.set_enabled(enabled);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_lyrics_enabled(enabled: bool, settings: State<'_, AppSettings>) -> AppResult<()> {
    let mut val = settings.lyrics_enabled.write().await;
    *val = enabled;
    Ok(())
}

#[tauri::command]
pub async fn update_album_metadata(
    old_name: String,
    new_name: String,
    new_artist: String,
    state: State<'_, DbState>,
) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::update_album_metadata(&conn, &old_name, &new_name, &new_artist)?;
    Ok(())
}

#[tauri::command]
pub async fn get_lyrics(path: String) -> AppResult<String> {
    let path = std::path::Path::new(&path);
    let parent = path.parent().ok_or_else(|| AppError::Custom("Invalid path".to_string()))?;
    let stem = path.file_stem().ok_or_else(|| AppError::Custom("Invalid file name".to_string()))?;

    let lrc_path = parent.join(format!("{}.lrc", stem.to_string_lossy()));
    let txt_path = parent.join(format!("{}.txt", stem.to_string_lossy()));

    if lrc_path.exists() {
        Ok(std::fs::read_to_string(lrc_path)?)
    } else if txt_path.exists() {
        Ok(std::fs::read_to_string(txt_path)?)
    } else {
        Err(AppError::Custom("Lyrics not found".to_string()))
    }
}
