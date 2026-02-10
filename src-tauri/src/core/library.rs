use std::path::PathBuf;
use walkdir::WalkDir;

pub fn scan_directory(path: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let supported_extensions = ["mp3", "flac", "ogg", "wav", "aac", "m4a"];

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(extension) = entry.path().extension() {
                if let Some(ext_str) = extension.to_str() {
                    if supported_extensions.contains(&ext_str.to_lowercase().as_str()) {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    files
}
