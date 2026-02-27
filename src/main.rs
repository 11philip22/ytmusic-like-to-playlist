mod lastfm_helper;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the sync flow
    Run {
        /// Path to auth.json (Last.fm key + YTM headers)
        #[arg(long, default_value = "auth.json")]
        auth: String,
        /// Path to config.json (canonical rules + overrides)
        #[arg(long, default_value = "config.json")]
        config: String,
    },
    /// Display loaded config details
    Display {
        /// Path to auth.json (Last.fm key + YTM headers)
        #[arg(long, default_value = "auth.json")]
        auth: String,
        /// Path to config.json (canonical rules + overrides)
        #[arg(long, default_value = "config.json")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Cli::parse();
    match args.command {
        Command::Run { auth, config } => run(auth, config),
        Command::Display { auth, config } => display(auth, config).await,
    }
}

async fn display(auth_file: String, config_file: String) -> Result<()> {
    let api_key = load_lastfm_api_key(&auth_file)?;
    let config = load_config(&config_file).context("Failed to load genre config")?;

    // login to youtube api
    println!("Authenticating with YouTube Music...");
    let auth = BrowserAuth::from_file(&auth_file).context("Failed to load headers.json")?;
    let client = YTMusicClient::builder().with_browser_auth(auth).build()?;

    println!("Fetching Liked Songs...");
    // Limit to 50 for testing
    let liked_playlist = client.get_liked_songs(Some(50)).await?;

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

        let lastfm_genres = if let Some(override_genre) = config.genre_overrides.get(&title) {
            override_genre.clone()
        } else {
            match fetch_genres(&api_key, &title, artist_name).await {
                Ok(genres) if !genres.is_empty() => {
                    canonicalize_genres(genres, &config.canonical_rules).join(", ")
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

fn run(auth_file: String, config_file: String) -> Result<()> {
    Ok(())
}

fn canonicalize_genres(tags: Vec<String>, rules: &[(String, String)]) -> Vec<String> {
    if tags.is_empty() {
        return tags;
    }

    let lowered: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    for (pattern, canonical) in rules {
        let needle = pattern.to_lowercase();
        if lowered.iter().any(|t| t.contains(&needle)) {
            return vec![canonical.clone()];
        }
    }

    tags
}

fn load_config(path: &str) -> Result<Config> {
    let data = read_to_string(path).with_context(|| format!("Failed to read {path}"))?;
    let config: Config =
        serde_json::from_str(&data).with_context(|| format!("Invalid JSON in {path}"))?;
    Ok(config)
}

fn load_lastfm_api_key(path: &str) -> Result<String> {
    let data = read_to_string(path).with_context(|| format!("Failed to read {path}"))?;
    let json: Value =
        serde_json::from_str(&data).with_context(|| format!("Invalid JSON in {path}"))?;
    let api_key = json["lastfm_api_key"]
        .as_str()
        .context("Missing lastfm_api_key")?;
    Ok(api_key.to_string())
}
