use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: i32,
    pub path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub cover_art: Option<String>,
    pub duration: Option<i32>,
    pub last_modified: Option<i64>, // mtime in seconds
}

use std::path::Path;

pub fn init_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT,
            artist TEXT,
            album TEXT,
            genre TEXT,
            year INTEGER,
            cover_art TEXT,
            duration INTEGER,
            last_modified INTEGER
        )",
        [],
    )?;

    // Indices for performance
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tracks_path ON tracks(path)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album)",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist)",
        [],
    )?;

    // Add column if not exists (migration)
    let _ = conn.execute("ALTER TABLE tracks ADD COLUMN last_modified INTEGER", []);
    // Add year column if missing (best-effort migration)
    let _ = conn.execute("ALTER TABLE tracks ADD COLUMN year INTEGER", []);

    conn.execute(
        "CREATE TABLE IF NOT EXISTS library_roots (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE
        )",
        [],
    )?;

    Ok(conn)
}

pub fn add_track(
    conn: &Connection,
    path: &str,
    title: Option<&str>,
    artist: Option<&str>,
    album: Option<&str>,
    genre: Option<&str>,
    cover_art: Option<&str>,
    duration: Option<i32>,
    last_modified: Option<i64>,
    year: Option<i32>,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO tracks (path, title, artist, album, genre, year, cover_art, duration, last_modified) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![path, title, artist, album, genre, year, cover_art, duration, last_modified],
    )?;
    Ok(())
}

pub fn get_all_tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut stmt = conn
        .prepare("SELECT id, path, title, artist, album, genre, year, duration, cover_art, last_modified FROM tracks")?;
    let track_iter = stmt.query_map([], |row| {
        Ok(Track {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            genre: row.get(5)?,
            year: row.get(6)?,
            duration: row.get(7)?,
            cover_art: row.get(8)?,
            last_modified: row.get(9)?,
        })
    })?;

    let mut tracks = Vec::new();
    for track in track_iter {
        tracks.push(track?);
    }
    Ok(tracks)
}

pub fn add_root(conn: &Connection, path: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO library_roots (path) VALUES (?1)",
        params![path],
    )?;
    Ok(())
}

pub fn remove_root(conn: &Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM library_roots WHERE path = ?1", params![path])?;
    Ok(())
}

pub fn get_roots(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM library_roots")?;
    let root_iter = stmt.query_map([], |row| row.get(0))?;

    let mut roots = Vec::new();
    for root in root_iter {
        roots.push(root?);
    }
    Ok(roots)
}

pub fn delete_track(conn: &Connection, path: &str) -> Result<()> {
    conn.execute("DELETE FROM tracks WHERE path = ?1", params![path])?;
    Ok(())
}

pub fn clear_roots(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM library_roots", [])?;
    Ok(())
}

pub fn clear_tracks(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM tracks", [])?;
    Ok(())
}

pub fn update_album_metadata(
    conn: &Connection,
    old_name: &str,
    new_name: &str,
    new_artist: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE tracks SET album = ?1, artist = ?2 WHERE album = ?3",
        params![new_name, new_artist, old_name],
    )?;
    Ok(())
}
