use tauri::{State, Manager};
use crate::error::AppResult;
use crate::core::playback::AudioSystem;

#[tauri::command]
pub async fn play_track(
    path: String,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    track_number: Option<u32>,
    total_tracks: Option<u32>,
    duration: Option<f64>,
    app_handle: tauri::AppHandle,
    audio: State<'_, AudioSystem>,
) -> AppResult<()> {
    use std::time::{SystemTime, UNIX_EPOCH};

    audio.play(&path);

    if let (Some(t), Some(a)) = (title, artist) {
        let album_opt = album.clone();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let end_time = duration.map(|d| now + (d as i64));

        tauri::async_runtime::spawn(async move {
            let mut cover_url = None;
            if let Some(alb) = album_opt {
                cover_url = crate::core::discord::fetch_cover_url(&a, &alb).await;
            }

            if let Some(settings) = app_handle.try_state::<crate::AppSettings>() {
                let rpc_guard = settings.rpc.read().await;
                if let Some(ref rpc_val) = *rpc_guard {
                    if rpc_val.is_enabled() {
                        drop(rpc_guard);
                        let mut rpc_write = settings.rpc.write().await;
                        if let Some(ref mut rpc_val) = *rpc_write {
                            rpc_val.ensure_connected();
                            rpc_val.update_presence(&t, &a, cover_url, track_number, total_tracks, Some(now), end_time);
                        }
                    }
                }
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn pause_track(audio: State<'_, AudioSystem>) -> AppResult<()> {
    audio.pause();
    Ok(())
}

#[tauri::command]
pub async fn resume_track(audio: State<'_, AudioSystem>) -> AppResult<()> {
    audio.resume();
    Ok(())
}

#[tauri::command]
pub async fn seek_track(seconds: f64, audio: State<'_, AudioSystem>) -> AppResult<()> {
    audio.seek(seconds);
    Ok(())
}

#[tauri::command]
pub async fn set_volume(volume: f32, audio: State<'_, AudioSystem>) -> AppResult<()> {
    audio.set_volume(volume);
    Ok(())
}
