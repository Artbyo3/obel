use tauri::{State, Manager};
use crate::db::schema;
use crate::error::{AppError, AppResult};
use crate::DbState;
use crate::core::library;
use crate::core::metadata;

fn save_cover_art(data: &[u8], mime: &str, covers_dir: &std::path::Path) -> Option<String> {
    let hash = blake3::hash(data);
    let ext = if mime.contains("png") { "png" } else { "jpg" };
    let filename = format!("{}.{}", hash.to_hex(), ext);
    let full_path = covers_dir.join(&filename);
    if !full_path.exists() {
        if let Err(e) = std::fs::write(&full_path, data) {
            eprintln!("Failed to write cover art {}: {}", filename, e);
            return None;
        }
    }
    Some(filename)
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn scan_library(state: State<'_, DbState>, app_handle: tauri::AppHandle) -> AppResult<usize> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    let roots = schema::get_roots(&conn)?;

    let existing_tracks = schema::get_all_tracks(&conn)?;
    let mut track_map = std::collections::HashMap::new();
    for t in &existing_tracks {
        track_map.insert(t.path.clone(), t.last_modified);
    }

    let mut removed_count = 0;
    let mut added_count = 0;

    let mut all_files = Vec::new();
    for root in &roots {
        println!("Scanning root: {}", root);
        let files = library::scan_directory(root);
        all_files.extend(files);
    }

    let current_files_set: std::collections::HashSet<String> = all_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    for track in existing_tracks {
        if !current_files_set.contains(&track.path) {
            if schema::delete_track(&conn, &track.path).is_ok() {
                removed_count += 1;
            }
        }
    }

    let app_data_dir = app_handle.path().app_data_dir().map_err(|e: tauri::Error| AppError::Custom(e.to_string()))?;
    let covers_dir = app_data_dir.join("covers");
    std::fs::create_dir_all(&covers_dir)?;

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
                    cover_path = save_cover_art(&data, &mime, &covers_dir);
                }

                if schema::add_track(
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
                ).is_ok() {
                    added_count += 1;
                }
            }
        }
    }

    Ok(added_count)
}

#[tauri::command]
pub async fn get_tracks(state: State<'_, DbState>, app_handle: tauri::AppHandle) -> AppResult<Vec<schema::Track>> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    let mut tracks = schema::get_all_tracks(&conn)?;

    let app_data_dir = app_handle.path().app_data_dir().map_err(|e: tauri::Error| AppError::Custom(e.to_string()))?;
    let covers_dir = app_data_dir.join("covers");

    for track in &mut tracks {
        if let Some(filename) = &track.cover_art {
            if !filename.starts_with("data:") && !filename.contains("://") && !filename.contains(":\\") && !filename.starts_with("/") {
                let full_path = covers_dir.join(filename);
                track.cover_art = Some(full_path.to_string_lossy().to_string());
            }
        }
    }

    Ok(tracks)
}

#[tauri::command]
pub async fn add_library_path(path: String, state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::add_root(&conn, &path)?;
    Ok(())
}

#[tauri::command]
pub async fn get_library_paths(state: State<'_, DbState>) -> AppResult<Vec<String>> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::get_roots(&conn).map_err(AppError::from)
}

#[tauri::command]
pub async fn remove_library_path(path: String, state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::remove_root(&conn, &path)?;
    Ok(())
}

#[tauri::command]
pub async fn clear_library_paths(state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::clear_roots(&conn)?;
    Ok(())
}

#[tauri::command]
pub async fn wipe_library(state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::clear_tracks(&conn)?;
    schema::clear_roots(&conn)?;
    Ok(())
}

#[tauri::command]
pub async fn create_album(name: String, root_path: String) -> AppResult<String> {
    let path = std::path::Path::new(&root_path).join(&name);
    if path.exists() {
        return Err(AppError::Custom("Album already exists".to_string()));
    }
    std::fs::create_dir_all(&path)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_file(source_path: String, target_dir: String) -> AppResult<()> {
    let source = std::path::Path::new(&source_path);
    let file_name = source.file_name().ok_or_else(|| AppError::Custom("Invalid source path".to_string()))?;
    let target = std::path::Path::new(&target_dir).join(file_name);
    std::fs::copy(source, target)?;
    Ok(())
}

#[tauri::command]
pub async fn delete_album(name: String, state: State<'_, DbState>) -> AppResult<()> {
    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    schema::delete_album(&conn, &name)?;
    Ok(())
}

#[tauri::command]
pub async fn import_dropped_paths(paths: Vec<String>, state: State<'_, DbState>, app_handle: tauri::AppHandle) -> AppResult<usize> {
    let mut added = 0;
    for path_str in &paths {
        let path = std::path::Path::new(path_str);
        if path.is_dir() {
            let files = library::scan_directory(path_str);
            for file_path in files {
                if import_single_file(&file_path, &state, &app_handle)? {
                    added += 1;
                }
            }
        } else if path.is_file() {
            if import_single_file(path, &state, &app_handle)? {
                added += 1;
            }
        }
    }
    Ok(added)
}

fn import_single_file(file_path: &std::path::Path, state: &State<'_, DbState>, app_handle: &tauri::AppHandle) -> AppResult<bool> {
    let supported = ["mp3", "flac", "ogg", "wav", "aac", "m4a"];
    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !supported.contains(&ext.as_str()) {
        return Ok(false);
    }

    let conn = state.conn.lock().map_err(|e| AppError::Custom(e.to_string()))?;
    let path_str = file_path.to_string_lossy().to_string();

    let app_data_dir = app_handle.path().app_data_dir().map_err(|e: tauri::Error| AppError::Custom(e.to_string()))?;
    let covers_dir = app_data_dir.join("covers");
    std::fs::create_dir_all(&covers_dir)?;

    let mtime = std::fs::metadata(file_path)
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
        .ok();

    if let Some(meta) = metadata::read_metadata(file_path) {
        let mut cover_path = None;
        if let (Some(data), Some(mime)) = (meta.cover_data, meta.cover_mime) {
            cover_path = save_cover_art(&data, &mime, &covers_dir);
        }

        schema::add_track(
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
        )?;
        return Ok(true);
    }
    Ok(false)
}
