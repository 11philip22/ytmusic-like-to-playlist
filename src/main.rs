mod lastfm_helper;

use anyhow::{Context, Result};
use clap::Parser;
use lastfm_helper::fetch_genres;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::read_to_string;
use ytmusicapi::{BrowserAuth, Playlist, PlaylistTrack, YTMusicClient};

#[derive(Debug, Deserialize)]
struct Config {
    canonical_rules: Vec<(String, String)>,
    genre_overrides: HashMap<String, String>,
    #[serde(default)]
    playlist_rules: HashMap<String, String>,
}

#[derive(Debug, Parser)]
#[command(about = "Sync liked songs with genres")]
struct Cli {
    /// Path to auth.json (Last.fm key + YTM headers)
    #[arg(long, default_value = "auth.json")]
    auth: String,
    /// Path to config.json (canonical rules + overrides)
    #[arg(long, default_value = "config.json")]
    config: String,
    /// Max number of liked songs to process (omit for no limit)
    #[arg(long)]
    limit: Option<usize>,
}

pub struct YtMusicGenreSyncer {
    config: Config,
    lastfm_api_key: String,
    yt_client: YTMusicClient,
    /// All tracks from playlists in playlist_rules (flat list).
    pub playlist_songs: Vec<PlaylistTrack>,
}

impl YtMusicGenreSyncer {
    pub fn new(auth_path: &str, config_path: &str) -> Result<Self> {
        let data = read_to_string(auth_path)?;
        let json: Value = serde_json::from_str(&data)?;
        let lastfm_api_key = json["lastfm_api_key"]
            .as_str()
            .context("Missing lastfm_api_key in auth")?
            .to_string();

        let config =
            load_config(config_path).context("Failed to load genre config")?;

        println!("Authenticating with YouTube Music...");
        let auth =
            BrowserAuth::from_file(auth_path).context("Failed to load headers.json")?;
        let yt_client = YTMusicClient::builder()
            .with_browser_auth(auth)
            .build()?;

        Ok(Self {
            config,
            lastfm_api_key,
            yt_client,
            playlist_songs: Vec::new(),
        })
    }

    pub async fn run(&self, limit: Option<u32>) -> Result<()> {
        println!("Fetching Liked Songs...");
        let liked_playlist = self.yt_client.get_liked_songs(limit).await?;

        println!(
            "Found {} songs. Fetching genres...",
            liked_playlist.tracks.len()
        );

        for track in liked_playlist.tracks {
            let title = track.title.clone().unwrap_or_default();
            let artist_name = track
                .artists
                .first()
                .map(|a| a.name.as_str())
                .unwrap_or("Unknown");

            if self.is_song_in_any_playlist(&title) {
                //println!("{} - {}: (already in playlist)", artist_name, title);
                continue;
            }

            let lastfm_genres =
                if let Some(override_genre) = self.config.genre_overrides.get(&title) {
                    override_genre.clone()
                } else {
                    match fetch_genres(
                        &self.lastfm_api_key,
                        &title,
                        artist_name,
                    )
                    .await
                    {
                        Ok(genres) if !genres.is_empty() => {
                            self.canonicalize_genres(genres).join(", ")
                        }
                        Ok(_) => String::new(),
                        Err(err) => {
                            eprintln!(
                                "Last.fm lookup failed for {} - {}: {}",
                                artist_name, title, err
                            );
                            String::new()
                        }
                    }
                };

            println!("{} - {}: {}", artist_name, title, lastfm_genres);
        }

        Ok(())
    }

    fn canonicalize_genres(&self, tags: Vec<String>) -> Vec<String> {
        if tags.is_empty() {
            return tags;
        }

        let lowered: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
        let rules = &self.config.canonical_rules;

        for (pattern, canonical) in rules {
            let needle = pattern.to_lowercase();
            if lowered.iter().any(|t| t.contains(&needle)) {
                return vec![canonical.clone()];
            }
        }

        tags
    }

    /// Get all songs from a playlist by name. Searches the user's library playlists
    /// for an exact (case-insensitive) match and returns the full playlist with tracks.
    pub async fn get_playlist_songs_by_name(&self, name: &str) -> Result<Playlist> {
        let playlists = self
            .yt_client
            .get_library_playlists(None)
            .await
            .context("Failed to fetch library playlists")?;

        let name_lower = name.to_lowercase();
        let summary = playlists
            .iter()
            .find(|pl| pl.title.to_lowercase() == name_lower)
            .with_context(|| format!("Playlist '{}' not found in library", name))?;

        let playlist = self
            .yt_client
            .get_playlist(&summary.playlist_id, None)
            .await
            .with_context(|| format!("Failed to fetch playlist '{}'", name))?;

        Ok(playlist)
    }

    /// Returns true if the song (by title) is in any of the playlists from
    /// `playlist_rules`. Requires `load_playlist_songs` to have been called first.
    pub fn is_song_in_any_playlist(&self, song_title: &str) -> bool {
        let title_lower = song_title.to_lowercase();
        self.playlist_songs.iter().any(|track| {
            track
                .title
                .as_ref()
                .map(|t| t.to_lowercase() == title_lower)
                .unwrap_or(false)
        })
    }

    /// Fetch all songs from each playlist in `playlist_rules` and store them in
    /// `playlist_songs` as a single flat list.
    pub async fn load_playlist_songs(&mut self) -> Result<()> {
        for (_genre, playlist_name) in &self.config.playlist_rules {
            println!("Fetching playlist '{}'...", playlist_name);
            let playlist = self.get_playlist_songs_by_name(playlist_name).await?;
            let count = playlist.tracks.len();
            self.playlist_songs.extend(playlist.tracks);
            println!("Fetched {} songs for playlist '{}'", count, playlist_name);
        }
        Ok(())
    }
}

fn load_config(path: &str) -> Result<Config> {
    let data = read_to_string(path).with_context(|| format!("Failed to read {path}"))?;
    let config: Config =
        serde_json::from_str(&data).with_context(|| format!("Invalid JSON in {path}"))?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    let mut syncer = YtMusicGenreSyncer::new(&args.auth, &args.config)?;
    syncer.load_playlist_songs().await?;
    syncer.run(args.limit.map(|l| l as u32)).await?;
    Ok(())
}
