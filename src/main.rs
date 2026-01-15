mod spotify_helper;

use anyhow::{Context, Result};
use rspotify::prelude::*;
use ytmusicapi::{BrowserAuth, YTMusicClient};

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env for Spotify credentials
    dotenvy::dotenv().ok();

    println!("Authenticating with YouTube Music...");
    let auth = BrowserAuth::from_file("headers.json").context("Failed to load headers.json")?;
    let client = YTMusicClient::builder().with_browser_auth(auth).build()?;

    println!("Authenticating with Spotify...");
    let spotify = spotify_helper::get_authenticated_client().await?;

    println!("Fetching Liked Songs...");
    // Limit to 50 for testing
    let liked_playlist = client.get_liked_songs(Some(50)).await?;

    println!("Found {} songs. Fetching genres...", liked_playlist.tracks.len());

    println!("{:<30} | {:<20} | {:<40} | {}", "Artist", "YTM Category", "Spotify Genres", "Title");
    println!("{:-<30}-+-{:-<20}-+-{:-<40}-+-{:-<30}", "", "", "", "");

    for track in liked_playlist.tracks {
        let video_id = match track.video_id {
            Some(ref id) => id,
            None => continue,
        };

        let title = track.title.clone().unwrap_or_default();
        let artist_name = track.artists.first().map(|a| a.name.as_str()).unwrap_or("Unknown");

        // YTM LOOKUP
        let mut ytm_category = "Unknown".to_string();
        if let Ok(song) = client.get_song(video_id).await {
             ytm_category = song
                .microformat
                .and_then(|m| m.microformat_data_renderer.category)
                .unwrap_or_else(|| "Unknown".to_string());
             
             if ytm_category == "Music" || ytm_category == "Unknown" {
                 // Try keywords/tags? (omitted for brevity as we focus on Spotify now, or keep if useful)
                 // Keeping simple for now to see Spotify contrast
             }
        }

        // SPOTIFY LOOKUP
        let mut spotify_genres = String::new();
        if let Ok(Some(spotify_track)) = spotify_helper::search_track(&spotify, &title, artist_name).await {
            if let Some(artist_simpl) = spotify_track.artists.first() {
                if let Some(id) = &artist_simpl.id {
                     if let Ok(full_artist) = spotify.artist(id.clone()).await {
                         if !full_artist.genres.is_empty() {
                             spotify_genres = full_artist.genres.join(", ");
                         }
                     }
                }
            }
        }

        println!("{:<30} | {:<20} | {:<40} | {}", artist_name, ytm_category, spotify_genres, title);
    }

    Ok(())
}
