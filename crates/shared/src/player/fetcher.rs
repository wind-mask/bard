use crate::models::song::{SongInfo, SongStatus};
use anyhow::{Context, Result};
use mpris::PlayerFinder;
fn get_songinfo() -> Result<SongInfo> {
    let player_finder = PlayerFinder::new().context("Could not connect to D-Bus")?;

    let player = player_finder
        .find_active()
        .context("Could not find any player")?;

    let status = player
        .get_playback_status()
        .context("Could not get playback status")?;
    let metadata = player
        .get_metadata()
        .context("Could not get metadata for player")?;
    let artists = metadata.get("xesam:artist").and_then(|_as| match _as {
        mpris::MetadataValue::String(a) => Some(vec![a.to_owned()]),
        mpris::MetadataValue::Array(values) => {
            let v = values
                .iter()
                .filter_map(|s| match s {
                    mpris::MetadataValue::String(ss) => Some(ss.to_owned()),
                    _ => None,
                })
                .collect();
            Some(v)
        }
        _ => None,
    });
    let artist = artists
        .unwrap_or_else(|| vec!["Unknown Artist".to_string()])
        .join(", ");
    let title = metadata
        .get("xesam:title")
        .and_then(|t| match t {
            mpris::MetadataValue::String(s) => Some(s.to_owned()),
            _ => None,
        })
        .unwrap_or_else(|| "Unknown Title".to_string());
    let position = player
        .get_position()
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    // Use artist and title as a simple unique ID
    let id = format!("{} - {}", artist, title);
    let url = metadata.get("xesam:url").and_then(|u| match u {
        mpris::MetadataValue::String(s) => Some(s.to_owned()),
        _ => None,
    });
    // Construct SongInfo
    let si = SongInfo {
        id: id.clone(),
        artist: artist.clone(),
        title: title.clone(),
        position,
        status: match status {
            mpris::PlaybackStatus::Playing => SongStatus::Playing,
            _ => SongStatus::Paused,
        },
        url,
    };

    Ok(si)
}

pub fn get_current_song() -> Result<Option<SongInfo>> {
    match get_songinfo() {
        Ok(song) => Ok(Some(song)),
        Err(e) => {
            // If no player is found, return Ok(None)
            if e.to_string().contains("Could not find any player") {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}
