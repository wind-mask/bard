use std::time::{Duration, Instant};

use compact_str::CompactString;
use shared::models::SongInfo;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeekSource {
    Mpris,
    Zbus,
}

#[derive(Debug)]
pub enum AppEvent {
    ToggleHidden,
    PlayerSnapshot {
        generation: u64,
        player: CompactString,
        song: SongInfo,
        observed_at: Instant,
    },
    PlayerUnavailable {
        generation: u64,
        player: CompactString,
    },
    NoActivePlayer {
        generation: u64,
    },
    Seeked {
        generation: Option<u64>,
        player: CompactString,
        position: Duration,
        source: SeekSource,
        observed_at: Instant,
    },
}
