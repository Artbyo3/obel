use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};

pub struct DiscordRPC {
    client: DiscordIpcClient,
    enabled: bool,
}

impl DiscordRPC {
    pub fn new(app_id: &str) -> Option<Self> {
        let mut client = DiscordIpcClient::new(app_id);
        match client.connect() {
            Ok(_) => {
                println!("DiscordRPC: Connected successfully with ID {}", app_id);
                Some(Self {
                    client,
                    enabled: true,
                })
            }
            Err(e) => {
                println!("DiscordRPC: Failed to connect on startup: {:?}", e);
                // We'll try to reconnect later in ensure_connected
                Some(Self {
                    client,
                    enabled: true,
                })
            }
        }
    }

    pub fn ensure_connected(&mut self) -> bool {
        if !self.enabled { return false; }
        
        // Check if we can just set an empty activity to test connection
        // (This is a bit hacky but discord-rich-presence doesn't have an is_connected() check)
        // Alternatively, we just try to reconnect if it fails.
        if self.client.connect().is_ok() {
            return true;
        }
        false
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
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
            println!("DiscordRPC: Presence update skipped (disabled)");
            return;
        }

        println!("DiscordRPC: Updating presence for {} - {}", title, artist);

        let details = format!("♪ {}", title);
        let state = artist.to_string();

        // Use cover_url if provided, otherwise fallback to app_icon
        let image = cover_url.unwrap_or_else(|| "app_icon".to_string());

        let mut payload = activity::Activity::new()
            .details(&details)
            .state(&state)
            .assets(activity::Assets::new().large_image(&image));

        // 2. Add Timestamps (Progress Bar)
        if let (Some(start), Some(end)) = (start_time, end_time) {
            payload = payload.timestamps(activity::Timestamps::new()
                .start(start)
                .end(end));
        }

        // 3. Add Native Party (for the "1 of 12" bubble)
        if let (Some(n), Some(total)) = (track_number, total_tracks) {
            println!("DiscordRPC: Setting party size to {} of {}", n, total);
            payload = payload.party(activity::Party::new()
                .id("obel_playback_session")
                .size([n as i32, total as i32]));
        }

        if let Err(e) = self.client.set_activity(payload) {
            println!("DiscordRPC: Failed to set activity: {:?}", e);
        } else {
            println!("DiscordRPC: Activity set successfully");
        }
    }
}

    pub async fn fetch_cover_url(artist: &str, album: &str) -> Option<String> {
    println!("DiscordRPC: Fetching cover for {} - {}", artist, album);
    let client = reqwest::Client::builder()
// ...
        .user_agent("ObelMusicManager/0.1.0 ( mailto:contact@obel.app )")
        .build()
        .ok()?;

    // 1. Search for the release
    let query = format!("artist:\"{}\" AND release:\"{}\"", artist, album);
    let search_url = format!("https://musicbrainz.org/ws/2/release?query={}&fmt=json", urlencoding::encode(&query));
    
    let response = client.get(search_url).send().await.ok()?.json::<serde_json::Value>().await.ok()?;
    
    // 2. Extract MBID of the first result
    let mbid = response["releases"][0]["id"].as_str()?;
    
    // 3. Return Cover Art Archive URL
    // We don't even need to query CAA, we can just guess the front cover URL
    // But let's use the standard API URL which redirects
    Some(format!("https://coverartarchive.org/release/{}/front", mbid))
}
