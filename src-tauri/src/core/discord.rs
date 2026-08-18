use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

pub struct DiscordRPC {
    client: DiscordIpcClient,
    connected: bool,
    enabled: bool,
}

impl DiscordRPC {
    pub fn new(app_id: &str) -> Option<Self> {
        let mut client = DiscordIpcClient::new(app_id);
        let connected = client.connect().is_ok();
        if connected {
            println!("DiscordRPC: Connected with ID {}", app_id);
        } else {
            println!("DiscordRPC: Failed to connect on startup (will retry)");
        }
        Some(Self {
            client,
            connected,
            enabled: true,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn ensure_connected(&mut self) -> bool {
        if !self.enabled {
            return false;
        }
        if self.connected {
            return true;
        }
        if self.client.connect().is_ok() {
            self.connected = true;
            return true;
        }
        false
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled && self.connected {
            let _ = self.client.clear_activity();
        }
    }

    pub fn update_presence(
        &mut self,
        title: &str,
        artist: &str,
        cover_url: Option<String>,
        track_number: Option<u32>,
        total_tracks: Option<u32>,
        start_time: Option<i64>,
        end_time: Option<i64>,
    ) {
        if !self.enabled {
            return;
        }

        let details = format!("\u{266A} {}", title);
        let state = artist.to_string();
        let image = cover_url.unwrap_or_else(|| "app_icon".to_string());

        let mut payload = activity::Activity::new()
            .details(&details)
            .state(&state)
            .assets(activity::Assets::new().large_image(&image));

        if let (Some(start), Some(end)) = (start_time, end_time) {
            payload = payload.timestamps(activity::Timestamps::new().start(start).end(end));
        }

        if let (Some(n), Some(total)) = (track_number, total_tracks) {
            payload = payload.party(
                activity::Party::new()
                    .id("obel_playback_session")
                    .size([n as i32, total as i32]),
            );
        }

        if let Err(e) = self.client.set_activity(payload) {
            println!("DiscordRPC: Failed to set activity: {:?}", e);
            self.connected = false;
        }
    }
}

pub async fn fetch_cover_url(artist: &str, album: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("ObelMusicManager/0.1.0")
        .build()
        .ok()?;

    let query = format!("artist:\"{}\" AND release:\"{}\"", artist, album);
    let search_url = format!(
        "https://musicbrainz.org/ws/2/release?query={}&fmt=json",
        urlencoding::encode(&query)
    );

    let response = client
        .get(search_url)
        .send()
        .await
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;

    let mbid = response["releases"][0]["id"].as_str()?;
    Some(format!(
        "https://coverartarchive.org/release/{}/front",
        mbid
    ))
}
