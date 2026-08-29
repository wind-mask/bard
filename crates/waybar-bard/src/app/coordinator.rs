use std::io::{self, Write};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use anyhow::Result;
use compact_str::{CompactString, ToCompactString};
use log::info;
use shared::lyrics::get_lyrics_status;
use shared::models::{SongInfo, SongStatus};

use crate::waybar::{RenderedFrame, render_if_changed};

use super::event::AppEvent;

pub struct Bard {
    song: Option<SongInfo>,
    player: Option<CompactString>,
    synced_at: Instant,
    hidden: bool,
    last_frame: Option<RenderedFrame>,
    display_offset_ms: i64,
}

impl Bard {
    pub fn new(display_offset_ms: i64) -> Self {
        Self {
            song: None,
            player: None,
            synced_at: Instant::now(),
            hidden: false,
            last_frame: None,
            display_offset_ms,
        }
    }

    fn frame_at(&self, now: Instant) -> (RenderedFrame, Option<Duration>) {
        if self.hidden {
            return (RenderedFrame::Hidden, None);
        }
        let Some(song) = self.song.as_ref() else {
            return (RenderedFrame::NoPlayer, None);
        };
        if song.status == SongStatus::Stopped {
            return (RenderedFrame::NoPlayer, None);
        }
        if song.status == SongStatus::Paused {
            return (RenderedFrame::Paused, None);
        }

        let position = playback_position(song, self.synced_at, now);
        match song.lyrics.as_ref() {
            Some(lyrics) => {
                let status = get_lyrics_status(lyrics, position, self.display_offset_ms);
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
                    next_lyric_timeout(status.next_timestamp, position),
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

    pub fn run<W: Write>(&mut self, events: Receiver<AppEvent>, writer: &mut W) -> Result<()> {
        loop {
            let (frame, timeout) = self.frame_at(Instant::now());
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
                self.hidden = !self.hidden;
                info!("waybar-bard: hidden state changed to {}", self.hidden);
            }
            AppEvent::ChangeSong {
                player,
                song,
                observed_at,
            } => {
                self.player = Some(player);
                self.synced_at = observed_at;
                self.song = Some(song);
            }
            AppEvent::PlayerUnavailable {} | AppEvent::NoActivePlayer {} => {
                self.song = None;
                self.player = None;
            }
            AppEvent::Seeked {
                player,
                position,
                observed_at,
            } => {
                if self.player.as_ref() != Some(&player) {
                    return;
                }
                if let Some(song) = self.song.as_mut() {
                    song.position = position;
                    self.synced_at = observed_at;
                }
            }
        }
    }
}

fn playback_position(song: &SongInfo, synced_at: Instant, now: Instant) -> Duration {
    if song.status == SongStatus::Playing {
        song.position
            .saturating_add(now.saturating_duration_since(synced_at))
    } else {
        song.position
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

    use super::super::event::AppEvent;
    use super::{Bard, playback_position};
    use crate::waybar::RenderedFrame;

    fn song(id: &str, position: Duration, status: SongStatus) -> SongInfo {
        SongInfo {
            id: id.to_compact_string(),
            artist: "artist".to_compact_string(),
            title: "title".to_compact_string(),
            position,
            status,
            lyrics: None,
            url: None,
        }
    }

    fn change_song(song: SongInfo, observed_at: Instant) -> AppEvent {
        AppEvent::ChangeSong {
            player: ":1.42".to_compact_string(),
            song,
            observed_at,
        }
    }

    #[test]
    fn playing_advances_but_paused_freezes() {
        let mut bard = Bard::new(0);
        let start = Instant::now();
        bard.apply_event(change_song(
            song("track", Duration::from_secs(10), SongStatus::Playing),
            start,
        ));
        assert_eq!(
            playback_position(
                bard.song.as_ref().unwrap(),
                bard.synced_at,
                start + Duration::from_secs(2)
            ),
            Duration::from_secs(12)
        );

        bard.apply_event(change_song(
            song("track", Duration::from_secs(12), SongStatus::Paused),
            start + Duration::from_secs(2),
        ));
        assert_eq!(bard.song.as_ref().unwrap().status, SongStatus::Paused);
        assert_eq!(
            bard.frame_at(start + Duration::from_secs(30)).0,
            RenderedFrame::Paused
        );
        assert_eq!(
            bard.song.as_ref().unwrap().position,
            Duration::from_secs(12)
        );
    }

    #[test]
    fn configured_offset_controls_frame_and_deadline() {
        let start = Instant::now();
        let mut bard = Bard::new(100);
        let mut playing = song("track", Duration::ZERO, SongStatus::Playing);
        playing.lyrics = Some(vec![LyricLine {
            timestamp: Duration::from_secs(1),
            text: "first".to_string(),
            translation: None,
        }]);
        bard.apply_event(change_song(playing, start));

        let (before, timeout) = bard.frame_at(start + Duration::from_secs(1));
        assert_eq!(
            before,
            RenderedFrame::Lyrics {
                current: "".to_compact_string(),
                alt: "first".to_compact_string(),
            }
        );
        assert_eq!(timeout, Some(Duration::from_millis(100)));

        let (at_boundary, _) = bard.frame_at(start + Duration::from_millis(1_100));
        assert_eq!(
            at_boundary,
            RenderedFrame::Lyrics {
                current: "first".to_compact_string(),
                alt: "".to_compact_string(),
            }
        );
    }

    #[test]
    fn snapshot_observation_time_compensates_queue_delay() {
        let mut bard = Bard::new(0);
        let now = Instant::now();
        bard.apply_event(change_song(
            song("track", Duration::from_secs(10), SongStatus::Playing),
            now - Duration::from_secs(2),
        ));

        assert_eq!(
            playback_position(bard.song.as_ref().unwrap(), bard.synced_at, now),
            Duration::from_secs(12)
        );
    }

    #[test]
    fn seek_from_other_player_is_ignored() {
        let mut bard = Bard::new(0);
        let now = Instant::now();
        bard.apply_event(change_song(
            song("track", Duration::from_secs(1), SongStatus::Playing),
            now,
        ));
        bard.apply_event(AppEvent::Seeked {
            player: ":1.99".to_compact_string(),
            position: Duration::from_secs(99),
            observed_at: now,
        });
        bard.apply_event(AppEvent::Seeked {
            player: ":1.42".to_compact_string(),
            position: Duration::from_secs(20),
            observed_at: now,
        });

        assert_eq!(
            playback_position(bard.song.as_ref().unwrap(), bard.synced_at, now),
            Duration::from_secs(20)
        );
    }

    #[test]
    fn interrupted_wait_does_not_rewind_position() {
        let mut bard = Bard::new(0);
        let start = Instant::now();
        bard.apply_event(change_song(
            song("track", Duration::from_secs(5), SongStatus::Playing),
            start,
        ));
        bard.apply_event(AppEvent::ToggleHidden);
        bard.apply_event(AppEvent::ToggleHidden);

        assert_eq!(
            playback_position(
                bard.song.as_ref().unwrap(),
                bard.synced_at,
                start + Duration::from_secs(3)
            ),
            Duration::from_secs(8)
        );
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
        let mut bard = Bard::new(100);
        let (_tx, rx) = mpsc::channel();
        assert!(bard.run(rx, &mut BrokenPipeWriter).is_ok());
    }
}
