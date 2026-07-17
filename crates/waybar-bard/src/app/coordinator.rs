use std::io::{self, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::Result;
use compact_str::{CompactString, ToCompactString};
use shared::lyrics::{get_lyrics, get_lyrics_status};
use shared::models::{LyricLine, SongInfo, SongStatus};

use crate::waybar::{RenderedFrame, render_if_changed};

use super::event::{AppEvent, SeekSource};
use super::playback_clock::PlaybackClock;

const SEEK_DEDUP_WINDOW: Duration = Duration::from_millis(250);
const SEEK_POSITION_TOLERANCE: Duration = Duration::from_millis(5);

#[derive(Debug)]
struct SeekFingerprint {
    player: CompactString,
    position: Duration,
    observed_at: Instant,
    source: SeekSource,
}

#[derive(Debug, Default)]
struct AppState {
    generation: u64,
    player: Option<CompactString>,
    song: Option<SongInfo>,
    lyrics: Option<Vec<LyricLine>>,
    clock: Option<PlaybackClock>,
    hidden: bool,
    last_seek: Option<SeekFingerprint>,
}

pub struct Coordinator {
    state: AppState,
    last_frame: Option<RenderedFrame>,
    display_offset_ms: i64,
}

impl Coordinator {
    pub fn new(display_offset_ms: i64) -> Self {
        Self {
            state: AppState::default(),
            last_frame: None,
            display_offset_ms,
        }
    }

    pub fn run<W: Write>(&mut self, events: Receiver<AppEvent>, writer: &mut W) -> Result<()> {
        loop {
            let now = Instant::now();
            let (frame, timeout) = self.frame_at(now);
            if let Err(error) = render_if_changed(writer, &mut self.last_frame, frame) {
                if is_broken_pipe(&error) {
                    return Ok(());
                }
                return Err(error);
            }

            let event = match timeout {
                Some(timeout) => match events.recv_timeout(timeout) {
                    Ok(event) => event,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                },
                None => match events.recv() {
                    Ok(event) => event,
                    Err(_) => return Ok(()),
                },
            };
            self.apply_event(event);
        }
    }

    fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::ToggleHidden => {
                self.state.hidden = !self.state.hidden;
                eprintln!("waybar-bard: hidden state changed to {}", self.state.hidden);
            }
            AppEvent::PlayerSnapshot {
                generation,
                player,
                song,
                observed_at,
            } => self.apply_snapshot(generation, player, song, observed_at),
            AppEvent::PlayerUnavailable { generation, player } => {
                if generation == self.state.generation
                    && self.state.player.as_ref() == Some(&player)
                {
                    self.clear_player(generation);
                }
            }
            AppEvent::NoActivePlayer { generation } => {
                if generation >= self.state.generation {
                    self.clear_player(generation);
                }
            }
            AppEvent::Seeked {
                generation,
                player,
                position,
                source,
                observed_at,
            } => self.apply_seek(generation, player, position, source, observed_at),
        }
    }

    fn apply_snapshot(
        &mut self,
        generation: u64,
        player: CompactString,
        song: SongInfo,
        observed_at: Instant,
    ) {
        if generation < self.state.generation {
            return;
        }
        if generation == self.state.generation
            && self
                .state
                .player
                .as_ref()
                .is_some_and(|current| current != player)
        {
            return;
        }
        if song.status == SongStatus::Stopped {
            self.clear_player(generation);
            return;
        }

        let same_player =
            generation == self.state.generation && self.state.player.as_ref() == Some(&player);

        let reload_lyrics = self
            .state
            .song
            .as_ref()
            .is_none_or(|current| current.id != song.id || current.url != song.url);
        let lyrics = reload_lyrics.then(|| match get_lyrics(&song) {
            Ok(lyrics) => lyrics,
            Err(error) => {
                eprintln!("waybar-bard: failed to read embedded lyrics: {error}");
                None
            }
        });
        let position = song.position;

        self.state.generation = generation;
        self.state.player = Some(player);
        if same_player {
            if let Some(clock) = self.state.clock.as_mut() {
                clock.resync(position, song.status, observed_at);
            } else {
                self.state.clock = Some(PlaybackClock::new(position, song.status, observed_at));
            }
        } else {
            self.state.clock = Some(PlaybackClock::new(position, song.status, observed_at));
        }
        self.state.song = Some(song);
        if let Some(lyrics) = lyrics {
            self.state.lyrics = lyrics;
        }
        self.state.last_seek = None;
    }

    fn apply_seek(
        &mut self,
        generation: Option<u64>,
        player: CompactString,
        position: Duration,
        source: SeekSource,
        observed_at: Instant,
    ) {
        if self.state.player.as_ref() != Some(&player)
            || generation.is_some_and(|value| value != self.state.generation)
        {
            return;
        }

        if self.state.last_seek.as_ref().is_some_and(|previous| {
            previous.player == player
                && previous.source != source
                && previous.position.abs_diff(position) <= SEEK_POSITION_TOLERANCE
                && observed_at.saturating_duration_since(previous.observed_at) <= SEEK_DEDUP_WINDOW
        }) {
            return;
        }

        if let Some(clock) = self.state.clock.as_mut() {
            clock.seek(position, observed_at);
            self.state.last_seek = Some(SeekFingerprint {
                player,
                position,
                observed_at,
                source,
            });
        }
    }

    fn clear_player(&mut self, generation: u64) {
        self.state.generation = generation;
        self.state.player = None;
        self.state.song = None;
        self.state.lyrics = None;
        self.state.clock = None;
        self.state.last_seek = None;
    }

    fn frame_at(&self, now: Instant) -> (RenderedFrame, Option<Duration>) {
        if self.state.hidden {
            return (RenderedFrame::Hidden, None);
        }
        let (Some(song), Some(clock)) = (&self.state.song, &self.state.clock) else {
            return (RenderedFrame::NoPlayer, None);
        };
        if clock.status() != SongStatus::Playing {
            return (RenderedFrame::Paused, None);
        }

        let current_position = clock.position_at(now);
        match &self.state.lyrics {
            Some(lyrics) => {
                let status = get_lyrics_status(lyrics, current_position, self.display_offset_ms);
                let current = status
                    .current_line
                    .map(|line| line.text.as_str())
                    .unwrap_or("");
                let alt = status
                    .current_line
                    .and_then(|line| line.translation.as_deref())
                    .or_else(|| status.next_line.map(|line| line.text.as_str()))
                    .unwrap_or("");
                (
                    RenderedFrame::Lyrics {
                        current: current.to_compact_string(),
                        alt: alt.to_compact_string(),
                    },
                    next_lyric_timeout(status.next_timestamp, current_position),
                )
            }
            None => (
                RenderedFrame::NoLyrics {
                    artist: song.artist.clone(),
                    title: song.title.clone(),
                },
                None,
            ),
        }
    }
}

fn next_lyric_timeout(
    next_timestamp: Option<Duration>,
    current_position: Duration,
) -> Option<Duration> {
    let remaining = next_timestamp?.checked_sub(current_position);
    Some(match remaining {
        Some(remaining) if !remaining.is_zero() => remaining.max(Duration::from_millis(10)),
        _ => Duration::from_millis(50),
    })
}

fn is_broken_pipe(error: &anyhow::Error) -> bool {
    let direct_io_error = error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<io::Error>())
        .any(|error| error.kind() == io::ErrorKind::BrokenPipe);
    let serde_io_error = error
        .downcast_ref::<serde_json::Error>()
        .and_then(serde_json::Error::io_error_kind)
        .is_some_and(|kind| kind == io::ErrorKind::BrokenPipe);
    direct_io_error || serde_io_error
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use compact_str::ToCompactString;
    use shared::models::{LyricLine, SongInfo, SongStatus};

    use super::super::event::{AppEvent, SeekSource};
    use super::Coordinator;
    use crate::waybar::RenderedFrame;

    fn song(id: &str, position: f64, status: SongStatus) -> SongInfo {
        SongInfo {
            id: id.to_compact_string(),
            artist: "artist".to_compact_string(),
            title: "title".to_compact_string(),
            position: Duration::from_secs_f64(position),
            status,
            url: None,
        }
    }

    fn snapshot(generation: u64, status: SongStatus, position: f64) -> AppEvent {
        AppEvent::PlayerSnapshot {
            generation,
            player: ":1.42".to_compact_string(),
            song: song("track", position, status),
            observed_at: Instant::now(),
        }
    }

    #[test]
    fn playback_clock_follows_snapshot_status() {
        let mut coordinator = Coordinator::new(100);
        coordinator.apply_event(snapshot(1, SongStatus::Playing, 10.0));
        assert_eq!(
            coordinator.state.clock.as_ref().unwrap().status(),
            SongStatus::Playing
        );

        coordinator.apply_event(snapshot(1, SongStatus::Paused, 12.0));
        let clock = coordinator.state.clock.as_ref().unwrap();
        assert_eq!(clock.status(), SongStatus::Paused);
        let now = Instant::now() + Duration::from_secs(30);
        assert_eq!(clock.position_at(now), Duration::from_secs(12));
        assert_eq!(coordinator.frame_at(now).0, RenderedFrame::Paused);

        coordinator.apply_event(snapshot(1, SongStatus::Playing, 12.0));
        assert_eq!(
            coordinator.state.clock.as_ref().unwrap().status(),
            SongStatus::Playing
        );
    }

    #[test]
    fn configured_offset_controls_frame_and_deadline() {
        let start = Instant::now();
        let mut coordinator = Coordinator::new(100);
        coordinator.apply_event(AppEvent::PlayerSnapshot {
            generation: 1,
            player: ":1.42".to_compact_string(),
            song: song("track", 0.0, SongStatus::Playing),
            observed_at: start,
        });
        coordinator.state.lyrics = Some(vec![LyricLine {
            timestamp: Duration::from_secs(1),
            text: "first".to_string(),
            translation: None,
        }]);

        let (before, timeout) = coordinator.frame_at(start + Duration::from_secs(1));
        assert_eq!(
            before,
            RenderedFrame::Lyrics {
                current: "".to_compact_string(),
                alt: "first".to_compact_string(),
            }
        );
        assert_eq!(timeout, Some(Duration::from_millis(100)));

        let (at_boundary, _) = coordinator.frame_at(start + Duration::from_millis(1_100));
        assert_eq!(
            at_boundary,
            RenderedFrame::Lyrics {
                current: "first".to_compact_string(),
                alt: "".to_compact_string(),
            }
        );
    }

    #[test]
    fn snapshot_observation_time_compensates_queue_and_io_delay() {
        let mut coordinator = Coordinator::new(100);
        let now = Instant::now();
        coordinator.apply_event(AppEvent::PlayerSnapshot {
            generation: 1,
            player: ":1.42".to_compact_string(),
            song: song("track", 10.0, SongStatus::Playing),
            observed_at: now - Duration::from_secs(2),
        });

        assert_eq!(
            coordinator.state.clock.as_ref().unwrap().position_at(now),
            Duration::from_secs(12)
        );
    }

    #[test]
    fn duplicate_seek_and_stale_generation_are_ignored() {
        let mut coordinator = Coordinator::new(100);
        coordinator.apply_event(snapshot(3, SongStatus::Playing, 1.0));
        let now = Instant::now();
        coordinator.apply_event(AppEvent::Seeked {
            generation: Some(3),
            player: ":1.42".to_compact_string(),
            position: Duration::from_secs(20),
            source: SeekSource::Mpris,
            observed_at: now,
        });
        coordinator.apply_event(AppEvent::Seeked {
            generation: None,
            player: ":1.42".to_compact_string(),
            position: Duration::from_millis(20_002),
            source: SeekSource::Zbus,
            observed_at: now + Duration::from_millis(20),
        });
        coordinator.apply_event(AppEvent::Seeked {
            generation: Some(2),
            player: ":1.42".to_compact_string(),
            position: Duration::from_secs(99),
            source: SeekSource::Mpris,
            observed_at: now + Duration::from_millis(30),
        });

        assert_eq!(
            coordinator.state.clock.as_ref().unwrap().position_at(now),
            Duration::from_secs(20)
        );
    }

    #[test]
    fn stopped_unavailable_and_toggle_clear_or_hide_state() {
        let mut coordinator = Coordinator::new(100);
        coordinator.apply_event(snapshot(1, SongStatus::Playing, 1.0));
        coordinator.apply_event(snapshot(1, SongStatus::Stopped, 1.0));
        assert!(matches!(
            coordinator.frame_at(Instant::now()).0,
            RenderedFrame::NoPlayer
        ));

        coordinator.apply_event(snapshot(2, SongStatus::Playing, 1.0));
        coordinator.apply_event(AppEvent::ToggleHidden);
        assert!(matches!(
            coordinator.frame_at(Instant::now()).0,
            RenderedFrame::Hidden
        ));
        coordinator.apply_event(AppEvent::ToggleHidden);
        coordinator.apply_event(AppEvent::PlayerUnavailable {
            generation: 2,
            player: ":1.42".to_compact_string(),
        });
        assert!(matches!(
            coordinator.frame_at(Instant::now()).0,
            RenderedFrame::NoPlayer
        ));
    }

    struct BrokenPipeWriter;

    impl io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn broken_pipe_is_a_normal_exit() {
        let mut coordinator = Coordinator::new(100);
        let (_tx, rx) = mpsc::channel();
        assert!(coordinator.run(rx, &mut BrokenPipeWriter).is_ok());
    }
}
