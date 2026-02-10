use lofty::prelude::*;
use lofty::read_from_path;
use std::path::Path;

#[derive(Debug, Default)]
pub struct TrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<i32>, // in seconds
    pub cover_data: Option<Vec<u8>>,
    pub cover_mime: Option<String>,
}

pub fn read_metadata(path: &Path) -> Option<TrackMetadata> {
    match read_from_path(path) {
        Ok(tagged_file) => {
            let tag = tagged_file.primary_tag();
            let properties = tagged_file.properties();

            let duration = properties.duration().as_secs() as i32;

            let (title, artist, album, genre, cover_data, cover_mime) = if let Some(tag) = tag {
                let pic = tag.pictures().first();
                let cover_data = pic.map(|p| p.data().to_vec());
                let cover_mime = pic.map(|p| {
                    p.mime_type()
                        .unwrap_or(&lofty::picture::MimeType::Jpeg)
                        .to_string()
                });

                (
                    tag.title().map(|s| s.to_string()),
                    tag.artist().map(|s| s.to_string()),
                    tag.album().map(|s| s.to_string()),
                    tag.genre().map(|s| s.to_string()),
                    cover_data,
                    cover_mime,
                )
            } else {
                (None, None, None, None, None, None)
            };

            Some(TrackMetadata {
                title,
                artist,
                album,
                genre,
                duration: Some(duration),
                cover_data,
                cover_mime,
            })
        }
        Err(_) => None,
    }
}
