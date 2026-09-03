//! Device detection and music library sync for portable MP3 players (e.g. SnowSky Echo Mini).
//!
//! The Echo Mini presents as a USB mass-storage device with up to two removable volumes:
//! internal storage (volume label "ECHO MINI", FAT32) and an optional microSD card
//! (label "Echo SD", exFAT). Detection is done by enumerating removable drives via the
//! Win32 API and matching on volume label or a distinctive firmware file (HIFIEC*.img).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Formats/extensions this sync supports copying to the device.
const SUPPORTED_EXTENSIONS: [&str; 7] = ["mp3", "flac", "ogg", "wav", "aac", "m4a", "ape"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveInfo {
    pub letter: String,
    pub label: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub is_echo_mini: bool,
    pub echo_volume: Option<String>, // "internal" | "sd"
}

impl DriveInfo {
    fn from_letter(letter: char, device_info: Option<&DeviceInfo>) -> DriveInfo {
        let (label, total, free) = volume_info(letter);
        let is_echo_mini = label == "ECHO MINI"
            || label == "Echo SD"
            || device_info.map_or(false, |d| d.seen_letters.contains(&letter));
        let echo_volume = if label.to_uppercase().contains("SD") {
            Some("sd".to_string())
        } else if is_echo_mini {
            Some("internal".to_string())
        } else {
            None
        };
        DriveInfo {
            letter: format!("{}:", letter),
            label,
            total_bytes: total,
            free_bytes: free,
            is_echo_mini,
            echo_volume,
        }
    }
}

/// Cached device info collected from the last scan.
#[derive(Debug, Default)]
struct DeviceInfo {
    seen_letters: Vec<char>,
}

/// Enumerate all removable drive letters currently mounted.
#[cfg(windows)]
fn removable_drive_letters() -> Vec<char> {
    use windows::Win32::Storage::FileSystem::GetLogicalDrives;
    use windows::Win32::Storage::FileSystem::GetDriveTypeW;
    const DRIVE_REMOVABLE: u32 = 2;
    let mut letters = Vec::new();
    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return letters;
    }
    for i in 0..26 {
        if mask & (1 << i) != 0 {
            let letter = (b'A' + i as u8) as char;
            let root = format!("{}:\\", letter);
            let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
            let drive_type = unsafe { GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr())) };
            if drive_type == DRIVE_REMOVABLE {
                letters.push(letter);
            }
        }
    }
    letters
}

#[cfg(not(windows))]
fn removable_drive_letters() -> Vec<char> {
    Vec::new()
}

/// Read volume label, total bytes and free bytes for a drive letter.
#[cfg(windows)]
fn volume_info(letter: char) -> (String, u64, u64) {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{GetDiskFreeSpaceExW, GetVolumeInformationW};
    let root = format!("{}:\\", letter);
    let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();

    let mut label_buf = [0u16; 261];
    let mut total: u64 = 0;
    let mut free: u64 = 0;

    let label = unsafe {
        let ok = GetVolumeInformationW(
            PCWSTR(wide.as_ptr()),
            Some(&mut label_buf),
            None,
            None,
            None,
            None,
        );
        if ok.is_ok() {
            let len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            String::from_utf16_lossy(&label_buf[..len]).trim().to_string()
        } else {
            String::new()
        }
    };
    let _ = unsafe {
        GetDiskFreeSpaceExW(PCWSTR(wide.as_ptr()), Some(&mut free), Some(&mut total), None)
    };

    (label, total, free)
}

#[cfg(not(windows))]
fn volume_info(_letter: char) -> (String, u64, u64) {
    (String::new(), 0, 0)
}

/// Check if a drive root contains the Echo Mini firmware file marker (HIFIEC*.img).
#[cfg(windows)]
fn has_firmware_marker(root: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.starts_with("HIFIEC") && name.ends_with(".img") {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(not(windows))]
fn has_firmware_marker(_root: &Path) -> bool {
    false
}

/// Scan for connected Echo Mini volumes.
pub fn detect_devices() -> Vec<DriveInfo> {
    let letters = removable_drive_letters();
    let mut device = DeviceInfo::default();
    for &ltr in &letters {
        let root = PathBuf::from(format!("{}:\\", ltr));
        if has_firmware_marker(&root) {
            device.seen_letters.push(ltr);
        }
    }
    letters
        .into_iter()
        .map(|ltr| DriveInfo::from_letter(ltr, Some(&device)))
        .collect()
}

/// Compute the destination path on the device for a track.
fn device_path(drive: &DriveInfo, artist: &str, album: &str, track_num: Option<i32>, title: &str, ext: &str) -> PathBuf {
    let base = PathBuf::from(format!("{}:\\", drive.letter.trim_end_matches(':')));
    let mut p = base.join("Music");
    let artist_dir = sanitize(if artist.is_empty() { "Unknown Artist" } else { artist });
    let album_dir = sanitize(if album.is_empty() { "Unknown Album" } else { album });
    p = p.join(artist_dir).join(album_dir);
    let num = track_num.map_or(String::new(), |n| format!("{:02} - ", n));
    let file = sanitize(title);
    p.join(format!("{}{}.{}", num, file, ext))
}

/// Remove characters illegal on FAT32/exFAT and trim/resolve.
fn sanitize(name: &str) -> String {
    let mut cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() {
        cleaned = "Untitled".to_string();
    }
    cleaned[..cleaned.len().min(150)].to_string()
}

fn is_supported_ext(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

/// A normalized track ready for comparison/sync.
pub struct SyncTrack {
    pub src_path: std::path::PathBuf,
    pub ext: String,
    pub artist: String,
    pub album: String,
    pub track_num: Option<i32>,
    pub title: String,
    pub size: u64,
}

/// Build the list of tracks (from Obel's library) to sync to the device.
///
/// Tracks are numbered sequentially within each (artist, album) group based on
/// their library id, producing stable, meaningful filenames (`01 - Title.ext`).
pub fn build_sync_plan(tracks: Vec<crate::db::schema::Track>) -> Vec<SyncTrack> {
    // Group by (artist, album) preserving library order (by id).
    let mut groups: Vec<(String, String, Vec<crate::db::schema::Track>)> = Vec::new();
    let mut group_index: HashMap<(String, String), usize> = HashMap::new();
    for t in tracks {
        let artist = t.artist.as_deref().unwrap_or("Unknown Artist").to_string();
        let album = t.album.as_deref().unwrap_or("Unknown Album").to_string();
        let key = (artist.clone(), album.clone());
        let idx = if let Some(&i) = group_index.get(&key) {
            i
        } else {
            let i = groups.len();
            groups.push((artist.clone(), album.clone(), Vec::new()));
            group_index.insert(key, i);
            i
        };
        groups[idx].2.push(t);
    }

    let mut plan: Vec<SyncTrack> = Vec::new();
    for (artist, album, group_tracks) in groups {
        for (n, t) in group_tracks.iter().enumerate() {
            let Some(ext) = t.path.rsplit('.').next().map(|e| e.to_lowercase()) else {
                continue;
            };
            if !is_supported_ext(&ext) {
                continue;
            }
            let src = std::path::PathBuf::from(&t.path);
            let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
            plan.push(SyncTrack {
                src_path: src,
                ext,
                artist: artist.clone(),
                album: album.clone(),
                track_num: Some(n as i32 + 1),
                title: t.title.as_deref().unwrap_or("Untitled").to_string(),
                size,
            });
        }
    }
    plan
}

/// Estimate free space required to sync remaining tracks.
pub fn required_space(plan: &[SyncTrack]) -> u64 {
    plan.iter().map(|t| t.size).sum()
}

/// Execute the sync: copy only tracks that are new or changed on the device.
///
/// Returns (copied_count, skipped_count). Emits progress via the callback.
pub fn sync_tracks<F>(plan: Vec<SyncTrack>, drive: &DriveInfo, mut on_progress: F) -> Result<(usize, usize), std::io::Error>
where
    F: FnMut(usize, usize, &str),
{
    let total = plan.len();
    let mut copied = 0;
    let mut skipped = 0;

    for (i, track) in plan.iter().enumerate() {
        let dest = device_path(drive, &track.artist, &track.album, track.track_num, &track.title, &track.ext);

        // Determine if the file already exists and is the same size (assume unchanged).
        let needs_copy = match std::fs::metadata(&dest) {
            Ok(dm) => {
                // Compare size; mtime comparisons are unreliable across FAT32, so size is the
                // primary signal for the MVP. (A future enhancement could hash the source.)
                dm.len() != track.size
            }
            Err(_) => true,
        };

        if needs_copy {
            on_progress(i, total, &track.title);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&track.src_path, &dest)?;
            copied += 1;
        } else {
            skipped += 1;
        }
    }

    Ok((copied, skipped))
}
