mod lastfm_helper;

use anyhow::{Context, Result};
use clap::Parser;
use lastfm_helper::fetch_genres;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs::read_to_string;
use ytmusicapi::{BrowserAuth, YTMusicClient};

#[derive(Debug, Deserialize)]
struct Config {
    canonical_rules: Vec<(String, String)>,
    genre_overrides: HashMap<String, String>,
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
        })
    }

    pub async fn run(&self, limit: Option<u32>) -> Result<()> {
        println!("Fetching Liked Songs...");
        let liked_playlist = self.yt_client.get_liked_songs(limit).await?;

        println!(
            "Found {} songs. Fetching genres...",
            liked_playlist.tracks.len()
        );

        println!(
            "{:<35} | {:<30} | {}",
            "Artist", "Last.fm Genres", "Title"
        );
        println!("{:-<35}-+-{:-<30}-+-{:-<30}", "", "", "");

        for track in liked_playlist.tracks {
            let title = track.title.clone().unwrap_or_default();
            let artist_name = track
                .artists
                .first()
                .map(|a| a.name.as_str())
                .unwrap_or("Unknown");

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
                                "Last.fm lookup failed for '{} - {}': {}",
                                artist_name, title, err
                            );
                            String::new()
                        }
                    }
                };

            println!(
                "{:<35} | {:<30} | {}",
                artist_name, lastfm_genres, title
            );
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
    let syncer = YtMusicGenreSyncer::new(&args.auth, &args.config)?;
    syncer.run(args.limit.map(|l| l as u32)).await?;
    Ok(())
}
