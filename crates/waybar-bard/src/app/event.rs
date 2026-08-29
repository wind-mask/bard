use std::time::{Duration, Instant};

use compact_str::CompactString;
use shared::models::SongInfo;

#[derive(Debug)]
pub enum AppEvent {
    ToggleHidden,
    PlayerUnavailable {},
    NoActivePlayer {},
    ChangeSong {
        player: CompactString,
        song: SongInfo,
        observed_at: Instant,
    },
    Seeked {
        player: CompactString,
        position: Duration,
        observed_at: Instant,
    },
}
