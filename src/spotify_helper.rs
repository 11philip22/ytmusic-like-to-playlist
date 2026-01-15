use anyhow::{Context, Result};
use rspotify::{
    model::{FullTrack, SearchResult, SearchType},
    prelude::*,
    scopes, AuthCodeSpotify, Credentials, OAuth,
};

/// Get an authenticated Spotify client using PKCE flow.
///
/// Requires `RSPOTIFY_CLIENT_ID`, `RSPOTIFY_CLIENT_SECRET`, and `RSPOTIFY_REDIRECT_URI`
/// in environment variables or `.env` file.
pub async fn get_authenticated_client() -> Result<AuthCodeSpotify> {
    let creds = Credentials::from_env().context("Failed to load Spotify credentials from environment")?;
    let oauth = OAuth::from_env(scopes!("user-read-private")) // Changed scope since we just read for now
        .context("Failed to load Spotify OAuth config from environment")?;

    let spotify = AuthCodeSpotify::new(creds, oauth);

    // Opening browser for auth
    let url = spotify.get_authorize_url(false)?;
    spotify.prompt_for_token(&url).await.context("Failed to authenticate with Spotify")?;

    Ok(spotify)
}

/// Search for a track on Spotify.
pub async fn search_track(spotify: &AuthCodeSpotify, title: &str, artist: &str) -> Result<Option<FullTrack>> {
    // Try specific query first: "track:Title artist:Artist"
    let query = format!("track:{} artist:{}", title, artist);
    let result = spotify.search(&query, SearchType::Track, None, None, Some(1), None).await?;

    if let SearchResult::Tracks(page) = result {
        if let Some(track) = page.items.into_iter().next() {
            return Ok(Some(track));
        }
    }

    // Fallback: search just by title if artist match is too strict or fails
    let relaxed_query = format!("{} {}", title, artist);
     let result = spotify.search(&relaxed_query, SearchType::Track, None, None, Some(1), None).await?;

    if let SearchResult::Tracks(page) = result {
        if let Some(track) = page.items.into_iter().next() {
            return Ok(Some(track));
        }
    }

    Ok(None)
}
