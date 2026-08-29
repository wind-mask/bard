use std::time::Duration;

use compact_str::CompactString;

use crate::models::LyricLine;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SongStatus {
    Paused,
    Playing,
    Stopped,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SongInfo {
    pub id: CompactString,
    pub artist: CompactString,
    pub title: CompactString,
    pub position: Duration,
    pub status: SongStatus,
    pub lyrics: Option<Vec<LyricLine>>,

    pub url: Option<CompactString>,
}
