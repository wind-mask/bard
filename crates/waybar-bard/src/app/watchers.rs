use log::{debug, error};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use compact_str::ToCompactString;
use shared::models::{SongInfo, SongStatus};
use shared::player;
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator},
    message::Type,
};

use super::event::AppEvent;

const MPRIS_NAMESPACE: &str = "org.mpris.MediaPlayer2";
const RETRY_DELAY: Duration = Duration::from_secs(2);
const IDLE_RESCAN_FALLBACK: Duration = Duration::from_secs(30);
const POSITION_CONFIRM_DELAY: Duration = Duration::from_millis(100);
const POSITION_RETRY_LIMIT: usize = 4;

pub fn spawn_signal_watcher(events: Sender<AppEvent>) -> Result<()> {
    let mut signals = Signals::new([SIGUSR1]).context("Failed to register SIGUSR1 handler")?;
    thread::spawn(move || {
        for _ in signals.forever() {
            if events.send(AppEvent::ToggleHidden).is_err() {
                break;
            }
        }
    });
    Ok(())
}

pub fn spawn_player_manager(events: Sender<AppEvent>, rescans: Receiver<()>) {
    thread::spawn(move || {
        loop {
            let player = match player::find_playing_player() {
                Ok(Some(player)) => player,
                Ok(None) => {
                    let _ = events.send(AppEvent::NoActivePlayer {});
                    if !wait_for_rescan(&rescans) {
                        break;
                    }
                    continue;
                }
                Err(error) => {
                    error!("waybar-bard: failed to find a playing MPRIS player: {error}");
                    let _ = events.send(AppEvent::NoActivePlayer {});
                    if !wait_for_rescan(&rescans) {
                        break;
                    }
                    continue;
                }
            };

            if let Err(error) = watch_selected_player(&player, &events) {
                error!("waybar-bard: selected player watcher failed: {error}");
                let _ = events.send(AppEvent::PlayerUnavailable {});
                if !wait_for_rescan(&rescans) {
                    break;
                }
            }

            while rescans.try_recv().is_ok() {}
            if events.send(AppEvent::NoActivePlayer {}).is_err() {
                break;
            }
        }
    });
}

pub fn spawn_seeked_watcher(events: Sender<AppEvent>) {
    thread::spawn(move || {
        loop {
            if let Err(error) = watch_seeked_signals(&events) {
                error!("waybar-bard: zbus Seeked watcher failed: {error}");
                thread::sleep(RETRY_DELAY);
            }
        }
    });
}

pub fn spawn_candidate_watchers(rescans: SyncSender<()>) {
    let names = rescans.clone();
    thread::spawn(move || {
        loop {
            if let Err(error) = watch_name_owner_changes(&names) {
                error!("waybar-bard: MPRIS name watcher failed: {error}");
                thread::sleep(RETRY_DELAY);
            }
        }
    });

    thread::spawn(move || {
        loop {
            if let Err(error) = watch_player_property_changes(&rescans) {
                error!("waybar-bard: MPRIS property watcher failed: {error}");
                thread::sleep(RETRY_DELAY);
            }
        }
    });
}

fn wait_for_rescan(rescans: &Receiver<()>) -> bool {
    match rescans.recv_timeout(IDLE_RESCAN_FALLBACK) {
        Ok(()) | Err(RecvTimeoutError::Timeout) => {
            while rescans.try_recv().is_ok() {}
            true
        }
        Err(RecvTimeoutError::Disconnected) => false,
    }
}

fn notify_rescan(rescans: &SyncSender<()>) -> bool {
    match rescans.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => true,
        Err(TrySendError::Disconnected(())) => false,
    }
}

fn watch_selected_player(selected: &mpris::Player, events: &Sender<AppEvent>) -> Result<()> {
    let mut song = player::song_from_player(selected)?;
    if song.status == SongStatus::Stopped {
        return Ok(());
    }
    let mut last_sync = Instant::now();
    if !send_song(events, selected, song.clone(), last_sync) {
        return Ok(());
    }
    let player_events = selected
        .events()
        .context("Could not start MPRIS event stream")?;
    for event in player_events {
        debug!("waybar-bard: MPRIS event: {event:?}");
        match event? {
            mpris::Event::PlayerShutDown | mpris::Event::Stopped => return Ok(()),
            mpris::Event::Playing => {
                let next = player::song_from_player(selected)?;
                if next.status == SongStatus::Stopped {
                    return Ok(());
                }
                song = next;
                last_sync = Instant::now();
                if !send_song(events, selected, song.clone(), last_sync) {
                    return Ok(());
                }
            }
            mpris::Event::Paused => {
                song.position = estimated_position(&song, last_sync);
                song.status = SongStatus::Paused;
                last_sync = Instant::now();
                if !send_song(events, selected, song.clone(), last_sync) {
                    return Ok(());
                }
            }
            mpris::Event::TrackChanged(_) => {
                let old_position = estimated_position(&song, last_sync);
                let candidate = player::song_from_player(selected)?;
                let next = confirm_track_position(selected, candidate, old_position)?;
                if next.status == SongStatus::Stopped {
                    return Ok(());
                }
                song = next;
                last_sync = Instant::now();
                if !send_song(events, selected, song.clone(), last_sync) {
                    return Ok(());
                }
            }
            mpris::Event::TrackMetadataChanged { .. } => {
                let old_position = estimated_position(&song, last_sync);
                let mut next = player::song_from_player(selected)?;
                if next.id != song.id {
                    next = confirm_track_position(selected, next, old_position)?;
                }
                if next.status == SongStatus::Stopped {
                    return Ok(());
                }
                song = next;
                last_sync = Instant::now();
                if !send_song(events, selected, song.clone(), last_sync) {
                    return Ok(());
                }
            }
            mpris::Event::Seeked { position_in_us } => {
                let position = Duration::from_micros(position_in_us);
                song.position = position;
                last_sync = Instant::now();
                if events
                    .send(AppEvent::Seeked {
                        player: selected.unique_name().to_compact_string(),
                        position,
                        observed_at: last_sync,
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
            mpris::Event::LoopingChanged(_)
            | mpris::Event::ShuffleToggled(_)
            | mpris::Event::VolumeChanged(_)
            | mpris::Event::PlaybackRateChanged(_)
            | mpris::Event::TrackAdded(_)
            | mpris::Event::TrackRemoved(_)
            | mpris::Event::TrackListReplaced => {}
        }
    }
    Ok(())
}

fn send_song(
    events: &Sender<AppEvent>,
    selected: &mpris::Player,
    song: SongInfo,
    observed_at: Instant,
) -> bool {
    events
        .send(AppEvent::ChangeSong {
            player: selected.unique_name().to_compact_string(),
            song,
            observed_at,
        })
        .is_ok()
}

fn estimated_position(song: &SongInfo, synced_at: Instant) -> Duration {
    if song.status == SongStatus::Playing {
        song.position.saturating_add(synced_at.elapsed())
    } else {
        song.position
    }
}

fn track_snapshot_ready(
    candidate: &SongInfo,
    observed: &SongInfo,
    old_position: Duration,
    attempt: usize,
) -> bool {
    let identity_stable = observed.id == candidate.id;
    let position_is_fresh = observed.position.abs_diff(old_position) >= Duration::from_millis(500);
    identity_stable && (position_is_fresh || attempt + 1 == POSITION_RETRY_LIMIT)
}

fn confirm_track_position(
    selected: &mpris::Player,
    mut candidate: SongInfo,
    old_position: Duration,
) -> Result<SongInfo> {
    let mut delay = POSITION_CONFIRM_DELAY;
    let mut last_error = None;

    for attempt in 0..POSITION_RETRY_LIMIT {
        thread::sleep(delay);
        match player::song_from_player(selected) {
            Ok(observed) => {
                let ready = track_snapshot_ready(&candidate, &observed, old_position, attempt);
                candidate = observed;
                if ready {
                    return Ok(candidate);
                }
                last_error = None;
            }
            Err(error) => last_error = Some(error),
        }
        delay = delay.saturating_mul(2);
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Track identity did not stabilize")))
}

fn watch_seeked_signals(events: &Sender<AppEvent>) -> Result<()> {
    let connection = Connection::session().context("Could not connect to session D-Bus")?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path("/org/mpris/MediaPlayer2")?
        .interface("org.mpris.MediaPlayer2.Player")?
        .member("Seeked")?
        .build();
    let mut messages = MessageIterator::for_match_rule(rule, &connection, Some(16))
        .context("Could not subscribe to MPRIS Seeked")?;

    for message in &mut messages {
        let message = message?;
        let Some(sender) = message.header().sender().map(|sender| sender.to_string()) else {
            continue;
        };
        let position_in_us: i64 = message
            .body()
            .deserialize()
            .context("Could not read MPRIS Seeked position")?;
        if position_in_us < 0 {
            continue;
        }
        if events
            .send(AppEvent::Seeked {
                player: sender.to_compact_string(),
                position: Duration::from_micros(position_in_us as u64),
                observed_at: Instant::now(),
            })
            .is_err()
        {
            return Ok(());
        }
    }
    Ok(())
}

fn watch_name_owner_changes(rescans: &SyncSender<()>) -> Result<()> {
    let connection = Connection::session().context("Could not connect to session D-Bus")?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.DBus")?
        .path("/org/freedesktop/DBus")?
        .interface("org.freedesktop.DBus")?
        .member("NameOwnerChanged")?
        .arg0ns(MPRIS_NAMESPACE)?
        .build();
    let mut messages = MessageIterator::for_match_rule(rule, &connection, Some(16))
        .context("Could not subscribe to MPRIS NameOwnerChanged")?;
    for message in &mut messages {
        message?;
        if !notify_rescan(rescans) {
            return Ok(());
        }
    }
    Ok(())
}

fn watch_player_property_changes(rescans: &SyncSender<()>) -> Result<()> {
    let connection = Connection::session().context("Could not connect to session D-Bus")?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path("/org/mpris/MediaPlayer2")?
        .interface("org.freedesktop.DBus.Properties")?
        .member("PropertiesChanged")?
        .build();
    let mut messages = MessageIterator::for_match_rule(rule, &connection, Some(16))
        .context("Could not subscribe to MPRIS PropertiesChanged")?;
    for message in &mut messages {
        message?;
        if !notify_rescan(rescans) {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use compact_str::ToCompactString;
    use shared::models::{SongInfo, SongStatus};

    use super::{POSITION_RETRY_LIMIT, estimated_position, track_snapshot_ready};

    fn song(id: &str, position: f64, status: SongStatus) -> SongInfo {
        SongInfo {
            id: id.to_compact_string(),
            artist: "artist".to_compact_string(),
            title: "title".to_compact_string(),
            position: Duration::from_secs_f64(position),
            status,
            lyrics: None,
            url: None,
        }
    }

    #[test]
    fn paused_position_does_not_advance() {
        let song = song("track", 12.0, SongStatus::Paused);
        assert_eq!(
            estimated_position(&song, Instant::now() - Duration::from_secs(30)),
            Duration::from_secs(12)
        );
    }

    #[test]
    fn track_confirmation_requires_stable_identity_and_fresh_position() {
        let candidate = song("new", 87.0, SongStatus::Playing);
        let different = song("other", 0.2, SongStatus::Playing);
        assert!(!track_snapshot_ready(
            &candidate,
            &different,
            Duration::from_secs(87),
            0
        ));

        let fresh = song("new", 0.2, SongStatus::Playing);
        assert!(track_snapshot_ready(
            &candidate,
            &fresh,
            Duration::from_secs(87),
            0
        ));

        let stale = song("new", 87.1, SongStatus::Playing);
        assert!(!track_snapshot_ready(
            &candidate,
            &stale,
            Duration::from_secs(87),
            0
        ));
        assert!(track_snapshot_ready(
            &candidate,
            &stale,
            Duration::from_secs(87),
            POSITION_RETRY_LIMIT - 1
        ));
    }
}
