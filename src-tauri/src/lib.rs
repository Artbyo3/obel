mod commands;
mod core;
mod db;

use std::sync::Mutex;

use core::playback::AudioSystem;
use rusqlite::Connection;
use tauri::Emitter;

pub struct DbState {
    pub conn: Mutex<Connection>,
}

pub struct AppSettings {
    pub discord_enabled: Mutex<bool>,
    pub discord_rpc: Mutex<Option<core::discord::DiscordRPC>>,
    pub lyrics_enabled: Mutex<bool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri::Manager;

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
            let db_path = app_data_dir.join("library.db");

            println!("Database path: {:?}", db_path);

            let conn = db::schema::init_db(&db_path).expect("Failed to initialize database");
            app.manage(DbState {
                conn: Mutex::new(conn),
            });

            let audio_system = AudioSystem::new(app.handle().clone());
            app.manage(Mutex::new(audio_system));

            // Discord RPC (User Provided ID)
            println!("DiscordRPC: Initializing with ID 1470417644732809259");
            let discord_rpc = core::discord::DiscordRPC::new("1470417644732809259");
            if discord_rpc.is_none() {
                println!(
                    "DiscordRPC: Could not connect to Discord on startup. It might not be running."
                );
            }
            app.manage(AppSettings {
                discord_enabled: Mutex::new(true),
                discord_rpc: Mutex::new(discord_rpc),
                lyrics_enabled: Mutex::new(true),
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(event) = event {
                println!("Rust WindowEvent::DragDrop: {:?}", event);
                match event {
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        println!("Rust Drop Detected: {:?}", paths);
                        // Manually emit event to ensure frontend gets it
                        let paths_str: Vec<String> = paths
                            .iter()
                            .map(|p| p.to_string_lossy().to_string())
                            .collect();
                        if let Err(e) = window.emit("custom-file-drop", paths_str) {
                            eprintln!("Failed to emit custom-file-drop: {}", e);
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::greet,
            commands::scan_library,
            commands::get_tracks,
            commands::play_track,
            commands::pause_track,
            commands::resume_track,
            commands::seek_track,
            commands::set_volume,
            commands::get_lyrics,
            commands::set_lyrics_enabled,
            commands::set_discord_enabled,
            commands::update_album_metadata,
            commands::add_library_path,
            commands::get_library_paths,
            commands::remove_library_path,
            commands::clear_library_paths,
            commands::wipe_library,
            commands::create_album,
            commands::import_file,
            commands::test_discord_rpc
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
