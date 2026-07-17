use std::time::Duration;

use compact_str::CompactString;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SongStatus {
    Paused,
    Playing,
    Stopped,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SongInfo {
    pub id: CompactString,
    pub artist: CompactString,
    pub title: CompactString,
    pub position: Duration,
    pub status: SongStatus,
    pub url: Option<CompactString>,
}
