use tauri::{State, Manager};
use crate::db::schema;
use crate::DbState;
use crate::AppSettings;
use crate::core::discord::DiscordRPC;
use crate::core::playback::AudioSystem;
use crate::core::library;
use crate::core::metadata;

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn scan_library(state: State<'_, DbState>, app_handle: tauri::AppHandle) -> Result<usize, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let roots = schema::get_roots(&conn).map_err(|e| e.to_string())?;
    println!("Scan requested. Found roots: {:?}", roots);

    // 1. Gather existing tracks for incremental scan comparison
    let existing_tracks = schema::get_all_tracks(&conn).map_err(|e| e.to_string())?;
    let mut track_map = std::collections::HashMap::new();
    for t in &existing_tracks {
        track_map.insert(t.path.clone(), t.last_modified);
    }

    let mut removed_count = 0;
    let mut added_count = 0;

    // 2. Gather all current files from roots
    let mut all_files = Vec::new();
    for root in &roots {
        println!("Scanning root: {}", root);
        let files = library::scan_directory(root);
        all_files.extend(files);
    }

    // 3. Cleanup: Remove tracks from DB that no longer exist on disk
    let current_files_set: std::collections::HashSet<String> = all_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    for track in existing_tracks {
        if !current_files_set.contains(&track.path) {
            println!("Removing missing file: {}", track.path);
            if schema::delete_track(&conn, &track.path).is_ok() {
                removed_count += 1;
            }
        }
    }
    println!("Cleanup finished. Removed {} dead entries.", removed_count);

    // 4. Process files: Incremental update
    println!("Processing {} current files...", all_files.len());
    
    // Ensure covers directory exists
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let covers_dir = app_data_dir.join("covers");
    std::fs::create_dir_all(&covers_dir).map_err(|e| e.to_string())?;

    for file_path in all_files {
        let path_str = file_path.to_string_lossy().to_string();

        let mtime = std::fs::metadata(&file_path)
            .and_then(|m| m.modified())
            .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
            .ok();

        let needs_update = match track_map.get(&path_str) {
            Some(saved_mtime) => mtime.map_or(true, |curr| curr != saved_mtime.unwrap_or(0)),
            None => true,
        };

        if needs_update {
            if let Some(meta) = metadata::read_metadata(&file_path) {
                let mut cover_path = None;
                
                if let (Some(data), Some(mime)) = (meta.cover_data, meta.cover_mime) {
                    // Use a hash of the data for the filename to avoid duplicates
                    use std::collections::hash_map::DefaultHasher;
                    use std::hash::{Hash, Hasher};
                    let mut hasher = DefaultHasher::new();
                    data.hash(&mut hasher);
                    let hash = hasher.finish();
                    
                    let ext = if mime.contains("png") { "png" } else { "jpg" };
                    let filename = format!("{:x}.{}", hash, ext);
                    let full_path = covers_dir.join(&filename);
                    
                    if !full_path.exists() {
                        let _ = std::fs::write(&full_path, data);
                    }
                    cover_path = Some(filename); // Store just the filename relative to covers dir
                }

                let res = schema::add_track(
                    &conn,
                    &path_str,
                    meta.title.as_deref(),
                    meta.artist.as_deref(),
                    meta.album.as_deref(),
                    meta.genre.as_deref(),
                    cover_path.as_deref(),
                    meta.duration,
                    mtime,
                    meta.year,
                );

                if res.is_ok() {
                    added_count += 1;
                }
            }
        }
    }

    println!("Scan finished. Added/Updated {} tracks.", added_count);
    Ok(added_count)
}

#[tauri::command]
pub async fn add_library_path(path: String, state: State<'_, DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::add_root(&conn, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_paths(state: State<'_, DbState>) -> Result<Vec<String>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::get_roots(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_library_path(path: String, state: State<'_, DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::remove_root(&conn, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_library_paths(state: State<'_, DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::clear_roots(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn wipe_library(state: State<'_, DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::clear_tracks(&conn).map_err(|e| e.to_string())?;
    schema::clear_roots(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_album(name: String, root_path: String) -> Result<String, String> {
    let path = std::path::Path::new(&root_path).join(&name);
    if path.exists() {
        return Err("Album already exists".to_string());
    }
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// Simple copy for now. In real app, might want to move or organize.
#[tauri::command]
pub async fn import_file(source_path: String, target_dir: String) -> Result<(), String> {
    let source = std::path::Path::new(&source_path);
    let file_name = source.file_name().ok_or("Invalid source path")?;
    let target = std::path::Path::new(&target_dir).join(file_name);
    
    std::fs::copy(source, target).map_err(|e| e.to_string())?;
    Ok(())
}



#[tauri::command]
pub async fn get_tracks(state: State<'_, DbState>, app_handle: tauri::AppHandle) -> Result<Vec<schema::Track>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut tracks = schema::get_all_tracks(&conn).map_err(|e| e.to_string())?;
    
    // Convert relative cover filenames to absolute paths for the frontend
    let app_data_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let covers_dir = app_data_dir.join("covers");
    
    for track in &mut tracks {
        if let Some(filename) = &track.cover_art {
            // If it doesn't look like a URL or an absolute path (legacy base64 or relative filename)
            if !filename.starts_with("data:") && !filename.contains("://") && !filename.contains(":\\") && !filename.starts_with("/") {
                let full_path = covers_dir.join(filename);
                track.cover_art = Some(full_path.to_string_lossy().to_string());
            }
        }
    }
    
    Ok(tracks)
}

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
    audio: State<'_, std::sync::Mutex<AudioSystem>>,
) -> Result<(), String> {
    use tauri::Manager;
    use std::time::{SystemTime, UNIX_EPOCH};

    // 1. Audio Playback
    let system = audio.lock().map_err(|e| e.to_string())?;
    system.play(&path);

    // 2. Discord RPC (Modular / Optional)
    if let (Some(t), Some(a)) = (title, artist) {
        println!("play_track: Notifying Discord. Track: {}, Artist: {}, Album: {:?}, Progress: {:?}/{:?}, Duration: {:?}", t, a, album, track_number, total_tracks, duration);
        let album_opt = album.clone();
        
        // Calculate timestamps for progress bar
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        
        let end_time = duration.map(|d| now + (d as i64));

        // Spawn a background task for Discord to avoid blocking playback
        tauri::async_runtime::spawn(async move {
            // 1. Fetch cover first (outside the lock, it's slow/async)
            let mut cover_url = None;
            if let Some(alb) = album_opt {
                cover_url = crate::core::discord::fetch_cover_url(&a, &alb).await;
            }

            // 2. Now lock and update Discord
            if let Some(settings) = app_handle.try_state::<AppSettings>() {
                if let Ok(mut rpc_guard) = settings.discord_rpc.lock() {
                    let rpc_opt: &mut Option<DiscordRPC> = &mut *rpc_guard;
                    if let Some(rpc) = rpc_opt.as_mut() {
                        let ok = rpc.ensure_connected();
                        println!("DiscordRPC: Connection status: {}", ok);
                        rpc.update_presence(
                            &t, 
                            &a, 
                            cover_url, 
                            track_number, 
                            total_tracks,
                            Some(now),
                            end_time
                        );
                    }
                }
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn test_discord_rpc(settings: State<'_, AppSettings>) -> Result<String, String> {
    if let Ok(mut rpc_guard) = settings.discord_rpc.lock() {
        let rpc_opt: &mut Option<DiscordRPC> = &mut *rpc_guard;
        if let Some(rpc) = rpc_opt.as_mut() {
            rpc.ensure_connected();
            rpc.update_presence("TEST TRACK", "OBEL TEST ARTIST", None, Some(1), Some(1), None, None);
            return Ok("Test command sent to Discord. Check your status!".to_string());
        }
    }
    Err("Discord RPC not initialized in settings".to_string())
}

#[tauri::command]
pub fn set_discord_enabled(enabled: bool, settings: State<'_, AppSettings>) -> Result<(), String> {
    if let Ok(mut enabled_guard) = settings.discord_enabled.lock() {
        *enabled_guard = enabled;
    }

    if let Ok(mut rpc_guard) = settings.discord_rpc.lock() {
        let rpc_opt: &mut Option<DiscordRPC> = &mut *rpc_guard;
        if let Some(rpc) = rpc_opt.as_mut() {
            rpc.set_enabled(enabled);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn pause_track(audio: State<'_, std::sync::Mutex<AudioSystem>>) -> Result<(), String> {
    let system = audio.lock().map_err(|e| e.to_string())?;
    system.pause();
    Ok(())
}

#[tauri::command]
pub fn resume_track(audio: State<'_, std::sync::Mutex<AudioSystem>>) -> Result<(), String> {
    let system = audio.lock().map_err(|e| e.to_string())?;
    system.resume();
    Ok(())
}

#[tauri::command]
pub fn seek_track(seconds: f64, audio: State<'_, std::sync::Mutex<AudioSystem>>) -> Result<(), String> {
    let system = audio.lock().map_err(|e| e.to_string())?;
    system.seek(seconds);
    Ok(())
}

#[tauri::command]
pub fn set_volume(volume: f32, audio: State<'_, std::sync::Mutex<AudioSystem>>) -> Result<(), String> {
    let system = audio.lock().map_err(|e| e.to_string())?;
    system.set_volume(volume);
    Ok(())
}

#[tauri::command]
pub fn update_album_metadata(
    old_name: String,
    new_name: String,
    new_artist: String,
    state: State<'_, DbState>,
) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schema::update_album_metadata(&conn, &old_name, &new_name, &new_artist).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_lyrics(path: String) -> Result<String, String> {
    let path = std::path::Path::new(&path);
    let parent = path.parent().ok_or("Invalid path")?;
    let stem = path.file_stem().ok_or("Invalid file name")?;
    
    // Check for .lrc then .txt
    let lrc_path = parent.join(format!("{}.lrc", stem.to_string_lossy()));
    let txt_path = parent.join(format!("{}.txt", stem.to_string_lossy()));
    
    if lrc_path.exists() {
        std::fs::read_to_string(lrc_path).map_err(|e| e.to_string())
    } else if txt_path.exists() {
        std::fs::read_to_string(txt_path).map_err(|e| e.to_string())
    } else {
        Err("Lyrics not found".to_string())
    }
}

#[tauri::command]
pub fn set_lyrics_enabled(enabled: bool, settings: State<'_, AppSettings>) -> Result<(), String> {
    if let Ok(mut enabled_guard) = settings.lyrics_enabled.lock() {
        *enabled_guard = enabled;
    }
    Ok(())
}
