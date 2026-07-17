use compact_str::CompactString;

#[derive(Debug, PartialEq, Clone)]
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
    pub position: f64,
    pub status: SongStatus,
    pub url: Option<CompactString>,
}
