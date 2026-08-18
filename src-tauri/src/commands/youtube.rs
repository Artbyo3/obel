use tokio::process::Command;
use crate::error::{AppError, AppResult};

#[tauri::command]
pub async fn download_from_youtube(url: String, destination: String) -> AppResult<String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err(AppError::Custom("YouTube URL is required.".to_string()));
    }

    let output_dir = std::path::Path::new(&destination);
    std::fs::create_dir_all(output_dir)?;

    let bin_candidates: &[(&str, &[&str])] = &[
        ("yt-dlp", &[]),
        ("youtube-dl", &[]),
    ];
    let python_candidates: &[(&str, &[&str])] = &[
        ("python", &["-m", "yt_dlp"]),
        ("python3", &["-m", "yt_dlp"]),
        ("py", &["-3", "-m", "yt_dlp"]),
    ];

    let mut downloader: Option<(&str, Vec<&str>)> = None;

    for (bin, args) in bin_candidates {
        let output = Command::new(bin)
            .args(*args)
            .arg("--version")
            .output()
            .await;
        if output.map(|o| o.status.success()).unwrap_or(false) {
            downloader = Some((bin, args.to_vec()));
            break;
        }
    }

    if downloader.is_none() {
        for (bin, args) in python_candidates {
            let output = Command::new(bin)
                .args(*args)
                .arg("--version")
                .output()
                .await;
            if output.map(|o| o.status.success()).unwrap_or(false) {
                downloader = Some((bin, args.to_vec()));
                break;
            }
        }
    }

    let (binary, base_args) = downloader.ok_or_else(|| {
        AppError::Custom("yt-dlp not found. Install it: pip install yt-dlp, or download from https://github.com/yt-dlp/yt-dlp/releases".to_string())
    })?;

    let file_pattern = output_dir.join("%(title)s.%(ext)s");
    let file_pattern_str = file_pattern
        .to_str()
        .ok_or_else(|| AppError::Custom("Download folder path is not valid UTF-8".to_string()))?
        .to_string();

    let output = Command::new(binary)
        .args(&base_args)
        .args([
            "--extract-audio",
            "--audio-format",
            "mp3",
            "--audio-quality",
            "0",
            "--restrict-filenames",
            "--no-playlist",
            "--embed-thumbnail",
            "--add-metadata",
            "--output",
            &file_pattern_str,
            trimmed_url,
        ])
        .output()
        .await
        .map_err(|e| AppError::Custom(format!("Failed to start yt-dlp: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Custom(format!("Download failed: {}", stderr.trim())));
    }

    Ok(format!("Downloaded to {}", output_dir.display()))
}
