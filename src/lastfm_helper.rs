use anyhow::{Context, Result, bail};
use reqwest::Client;
use serde::Deserialize;

const USER_AGENT: &str = "ytmusic-like-to-playlist/0.1 (contact@example.com)"; // replace with your contact
const LASTFM_ENDPOINT: &str = "https://ws.audioscrobbler.com/2.0/";
const TOP_TAG_LIMIT: usize = 20;

#[derive(Debug, Deserialize)]
struct TrackInfoResponse {
    track: Option<Track>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Track {
    #[serde(default)]
    toptags: Option<TopTags>,
}

#[derive(Debug, Deserialize)]
struct ArtistTopTagsResponse {
    toptags: Option<TopTags>,
    error: Option<i32>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TopTags {
    #[serde(default, deserialize_with = "deserialize_tags")]
    tag: Vec<Tag>,
}

#[derive(Debug, Deserialize)]
struct Tag {
    name: String,
    #[serde(default, deserialize_with = "deserialize_count")]
    count: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TagList {
    Many(Vec<Tag>),
    One(Tag),
}

fn deserialize_tags<'de, D>(deserializer: D) -> std::result::Result<Vec<Tag>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let list = Option::<TagList>::deserialize(deserializer)?;
    let tags = match list {
        Some(TagList::Many(tags)) => tags,
        Some(TagList::One(tag)) => vec![tag],
        None => Vec::new(),
    };
    Ok(tags)
}

fn deserialize_count<'de, D>(deserializer: D) -> std::result::Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    let count = match raw {
        Some(serde_json::Value::Number(n)) => n.as_u64().map(|v| v as u32),
        Some(serde_json::Value::String(s)) => s.parse::<u32>().ok(),
        _ => None,
    };
    Ok(count)
}

pub async fn fetch_genres(api_key: &str, title: &str, artist: &str) -> Result<Vec<String>> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("Failed to build HTTP client")?;

    let mut names = fetch_track_tags(&client, &api_key, title, artist).await?;

    if names.is_empty() {
        names = fetch_artist_tags(&client, &api_key, artist).await?;
    }

    names.sort_unstable();
    names.dedup();

    Ok(names)
}

async fn fetch_track_tags(
    client: &Client,
    api_key: &str,
    title: &str,
    artist: &str,
) -> Result<Vec<String>> {
    let response = client
        .get(LASTFM_ENDPOINT)
        .query(&[
            ("method", "track.getInfo"),
            ("api_key", api_key),
            ("artist", artist),
            ("track", title),
            ("format", "json"),
            ("autocorrect", "1"),
        ])
        .send()
        .await
        .context("Last.fm track.getInfo request failed")?;

    let response = response
        .error_for_status()
        .context("Last.fm track.getInfo returned error status")?;

    let payload: TrackInfoResponse = response
        .json()
        .await
        .context("Failed to deserialize Last.fm track.getInfo response")?;

    if let Some(code) = payload.error {
        match code {
            6 => return Ok(Vec::new()), // Track not found
            _ => {
                let message = payload
                    .message
                    .unwrap_or_else(|| "Last.fm track.getInfo failed".to_string());
                bail!("{} (error code {})", message, code);
            }
        }
    }

    if let Some(track) = payload.track {
        if let Some(tags) = track.toptags {
            return Ok(select_top_tags(tags.tag));
        }
    }

    Ok(Vec::new())
}

async fn fetch_artist_tags(client: &Client, api_key: &str, artist: &str) -> Result<Vec<String>> {
    let response = client
        .get(LASTFM_ENDPOINT)
        .query(&[
            ("method", "artist.getTopTags"),
            ("api_key", api_key),
            ("artist", artist),
            ("format", "json"),
            ("autocorrect", "1"),
        ])
        .send()
        .await
        .context("Last.fm artist.getTopTags request failed")?;

    let response = response
        .error_for_status()
        .context("Last.fm artist.getTopTags returned error status")?;

    let payload: ArtistTopTagsResponse = response
        .json()
        .await
        .context("Failed to deserialize Last.fm artist.getTopTags response")?;

    if let Some(code) = payload.error {
        match code {
            6 => return Ok(Vec::new()), // Artist not found
            _ => {
                let message = payload
                    .message
                    .unwrap_or_else(|| "Last.fm artist.getTopTags failed".to_string());
                bail!("{} (error code {})", message, code);
            }
        }
    }

    if let Some(tags) = payload.toptags {
        return Ok(select_top_tags(tags.tag));
    }

    Ok(Vec::new())
}

fn select_top_tags(mut tags: Vec<Tag>) -> Vec<String> {
    if tags.is_empty() {
        return Vec::new();
    }

    let mut counted: Vec<Tag> = tags
        .drain(..)
        .filter(|t| t.count.unwrap_or(0) > 0)
        .collect();

    if !counted.is_empty() {
        counted.sort_by(|a, b| {
            b.count
                .unwrap_or(0)
                .cmp(&a.count.unwrap_or(0))
                .then(a.name.cmp(&b.name))
        });

        return counted
            .into_iter()
            .filter(|t| !t.name.is_empty())
            .take(TOP_TAG_LIMIT)
            .map(|t| t.name)
            .collect();
    }

    tags.into_iter()
        .filter(|t| !t.name.is_empty())
        .take(TOP_TAG_LIMIT)
        .map(|t| t.name)
        .collect()
}
