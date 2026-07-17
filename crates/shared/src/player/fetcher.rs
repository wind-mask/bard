use crate::models::song::{SongInfo, SongStatus};
use anyhow::{Context, Result};
use compact_str::{CompactString, ToCompactString, format_compact};
use mpris::{Player, PlayerFinder};
use url::Url;
use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator},
    fdo::NameOwnerChanged,
    message::Type,
};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
const MPRIS_NAMESPACE: &str = "org.mpris.MediaPlayer2";

fn artists_from_metadata(metadata: &mpris::Metadata) -> Vec<String> {
    metadata
        .get("xesam:artist")
        .and_then(|artists| match artists {
            mpris::MetadataValue::String(artist) => Some(vec![artist.to_owned()]),
            mpris::MetadataValue::Array(values) => Some(
                values
                    .iter()
                    .filter_map(|value| match value {
                        mpris::MetadataValue::String(artist) => Some(artist.to_owned()),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .filter(|artists| !artists.is_empty())
        .unwrap_or_else(|| vec!["Unknown Artist".to_string()])
}

fn metadata_string(metadata: &mpris::Metadata, key: &str) -> Option<CompactString> {
    metadata.get(key).and_then(|value| match value {
        mpris::MetadataValue::String(value) => Some(value.to_compact_string()),
        _ => None,
    })
}

fn normalize_url(value: &str) -> Option<CompactString> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    Url::parse(value)
        .ok()
        .map(|url| url.as_str().to_compact_string())
}

fn song_identity(
    track_id: Option<&str>,
    url: Option<&str>,
    artist: &str,
    title: &str,
) -> CompactString {
    if let Some(track_id) = track_id.map(str::trim)
        && !track_id.is_empty()
        && track_id != mpris::TrackID::no_track().as_str()
    {
        return format_compact!("track:{track_id}");
    }

    if let Some(url) = url.and_then(normalize_url) {
        return format_compact!("url:{url}");
    }

    format_compact!("metadata:{artist} - {title}")
}

pub fn song_from_player(player: &Player) -> Result<SongInfo> {
    let status = player
        .get_playback_status()
        .context("Could not get playback status")?;
    let metadata = player
        .get_metadata()
        .context("Could not get metadata for player")?;

    let artist = artists_from_metadata(&metadata)
        .join(", ")
        .to_compact_string();
    let title = metadata_string(&metadata, "xesam:title")
        .map(|t| t.to_compact_string())
        .unwrap_or_else(|| "Unknown Title".to_compact_string());
    let position = player
        .get_position()
        .context("Could not get player position")?;
    let url = metadata_string(&metadata, "xesam:url").and_then(|url| normalize_url(&url));
    let track_id = metadata.track_id();
    let id = song_identity(
        track_id.as_ref().map(mpris::TrackID::as_str),
        url.as_deref(),
        &artist,
        &title,
    );

    Ok(SongInfo {
        id,
        artist,
        title,
        position,
        status: match status {
            mpris::PlaybackStatus::Playing => SongStatus::Playing,
            mpris::PlaybackStatus::Paused => SongStatus::Paused,
            mpris::PlaybackStatus::Stopped => SongStatus::Stopped,
        },
        url,
    })
}

pub fn find_active_player() -> Result<Option<Player>> {
    let player_finder = PlayerFinder::new().context("Could not connect to D-Bus")?;

    match player_finder.find_active() {
        Ok(player) => Ok(Some(player)),
        Err(mpris::FindingError::NoPlayerFound) => Ok(None),
        Err(error) => Err(error).context("Could not find active MPRIS player"),
    }
}

/// 查找一个真正处于播放状态的播放器。暂停播放器只会由已选中的 watcher 继续保持。
pub fn find_playing_player() -> Result<Option<Player>> {
    let player_finder = PlayerFinder::new().context("Could not connect to D-Bus")?;
    let players = player_finder
        .iter_players()
        .context("Could not enumerate MPRIS players")?;

    for candidate in players {
        let Ok(player) = candidate else {
            continue;
        };
        if matches!(
            player.get_playback_status(),
            Ok(mpris::PlaybackStatus::Playing)
        ) {
            return Ok(Some(player));
        }
    }

    Ok(None)
}

pub fn get_current_song() -> Result<Option<SongInfo>> {
    find_active_player()?
        .map(|player| song_from_player(&player))
        .transpose()
}

fn has_mpris_name(connection: &Connection) -> Result<bool> {
    let proxy =
        zbus::blocking::fdo::DBusProxy::new(connection).context("Could not create D-Bus proxy")?;
    let names = proxy.list_names().context("Could not list D-Bus names")?;

    Ok(names
        .iter()
        .any(|name| name.as_str().starts_with(MPRIS_PREFIX)))
}

pub fn wait_for_mpris_player() -> Result<()> {
    let connection = Connection::session().context("Could not connect to session D-Bus")?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .path("/org/freedesktop/DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg0ns(MPRIS_NAMESPACE)?
        .build();
    let mut iterator = MessageIterator::for_match_rule(rule, &connection, Some(16))
        .context("Could not subscribe to NameOwnerChanged")?;

    // 避免在 find_active_player() 与完成订阅之间有播放器启动而导致永远等待。
    if has_mpris_name(&connection)? {
        return Ok(());
    }

    for message in &mut iterator {
        let signal = match NameOwnerChanged::from_message(message?) {
            Some(signal) => signal,
            None => continue,
        };
        let args = signal.args()?;

        if args.name().as_str().starts_with(MPRIS_PREFIX) && args.new_owner().as_ref().is_some() {
            return Ok(());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_url, song_identity};

    #[test]
    fn identity_prefers_track_id() {
        assert_eq!(
            song_identity(
                Some("/org/mpris/MediaPlayer2/track/42"),
                Some("file:///music/song.flac"),
                "Artist",
                "Title",
            ),
            "track:/org/mpris/MediaPlayer2/track/42"
        );
    }

    #[test]
    fn identity_uses_url_when_track_id_is_missing_or_no_track() {
        assert_eq!(
            song_identity(None, Some("file:///music/song.flac"), "Artist", "Title",),
            "url:file:///music/song.flac"
        );
        assert_eq!(
            song_identity(
                Some("/org/mpris/MediaPlayer2/TrackList/NoTrack"),
                Some("file:///music/song.flac"),
                "Artist",
                "Title",
            ),
            "url:file:///music/song.flac"
        );
    }

    #[test]
    fn identity_ignores_empty_values_and_normalizes_urls() {
        assert_eq!(
            song_identity(Some("  "), Some("  "), "Artist", "Title"),
            "metadata:Artist - Title"
        );
        assert_eq!(
            song_identity(None, Some("file:///music/a b.flac"), "Artist", "Title"),
            "url:file:///music/a%20b.flac"
        );
        assert_eq!(
            normalize_url(" file:///music/a%20b.flac ").as_deref(),
            Some("file:///music/a%20b.flac")
        );
    }

    #[test]
    fn identity_falls_back_to_display_metadata() {
        assert_eq!(
            song_identity(None, None, "Artist", "Title"),
            "metadata:Artist - Title"
        );
    }
}
