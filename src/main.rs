mod lastfm_helper;

use anyhow::{Context, Result};
use ytmusicapi::{BrowserAuth, YTMusicClient};

const CANONICAL_RULES: &[(&str, &str)] = &[("black metal", "metal"),
                                           ("death metal", "metal"),
                                           ("punkrock", "punkrock"),
                                           ("hiphop", "hiphop"),
                                           ("Hip-Hop", "hiphop"),
                                           ("hard rock", "hard rock"),
                                           ("hardcore", "hardcore"),
                                           ("psychedelic rock", "psychedelic rock")];

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    println!("Authenticating with YouTube Music...");
    let auth = BrowserAuth::from_file("headers.json").context("Failed to load headers.json")?;
    let client = YTMusicClient::builder().with_browser_auth(auth).build()?;

    println!("Fetching Liked Songs...");
    // Limit to 50 for testing
    let liked_playlist = client.get_liked_songs(Some(50)).await?;

    println!(
        "Found {} songs. Fetching genres...",
        liked_playlist.tracks.len()
    );

    println!(
        "{:<30} | {:<20} | {:<40} | {}",
        "Artist", "YTM Category", "Last.fm Genres", "Title"
    );
    println!("{:-<30}-+-{:-<20}-+-{:-<40}-+-{:-<30}", "", "", "", "");

    for track in liked_playlist.tracks {
        let video_id = match track.video_id {
            Some(ref id) => id,
            None => continue,
        };

        let title = track.title.clone().unwrap_or_default();
        let artist_name = track
            .artists
            .first()
            .map(|a| a.name.as_str())
            .unwrap_or("Unknown");

        // YTM LOOKUP
        let mut ytm_category = "Unknown".to_string();
        if let Ok(song) = client.get_song(video_id).await {
            ytm_category = song
                .microformat
                .and_then(|m| m.microformat_data_renderer.category)
                .unwrap_or_else(|| "Unknown".to_string());
        }

        let lastfm_genres = match lastfm_helper::fetch_genres(&title, artist_name).await {
            Ok(genres) if !genres.is_empty() => canonicalize_genres(genres).join(", "),
            Ok(_) => String::new(),
            Err(err) => {
                eprintln!(
                    "Last.fm lookup failed for '{} - {}': {}",
                    artist_name, title, err
                );
                String::new()
            }
        };

        println!(
            "{:<30} | {:<20} | {:<40} | {}",
            artist_name, ytm_category, lastfm_genres, title
        );

        // Be kind to external APIs: avoid hammering with rapid requests.
        //sleep(Duration::from_secs(2)).await;
    }

    Ok(())
}

fn canonicalize_genres(tags: Vec<String>) -> Vec<String> {
    if tags.is_empty() {
        return tags;
    }

    let lowered: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();

    for (pattern, canonical) in CANONICAL_RULES {
        let needle = pattern.to_lowercase();
        if lowered.iter().any(|t| t.contains(&needle)) {
            return vec![(*canonical).to_string()];
        }
    }

    tags
}
