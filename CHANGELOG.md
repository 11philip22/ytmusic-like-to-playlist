# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Fetch playlists by name via `get_playlist_songs_by_name`
- Load all songs from `playlist_rules` into a flat list via `load_playlist_songs`
- Check if a song is in any playlist before fetching Last.fm genres via `is_song_in_any_playlist`
- Add songs to genre playlists via `add_song_to_genre_playlist` when a detected genre has a mapped playlist

### Changed

- Refactor tool into struct-based design with `YtMusicGenreSyncer` encapsulating config, clients, and playlist state
