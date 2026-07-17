use std::fmt::Display;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricLine {
    pub timestamp: Duration,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct LyricsStatus<'a> {
    pub current_line: Option<&'a LyricLine>,
    pub next_line: Option<&'a LyricLine>,
    pub next_timestamp: Option<Duration>,
}

impl Display for LyricLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.2}] {} {}",
            self.timestamp.as_secs_f64(),
            self.text,
            self.translation.as_ref().map_or("", |value| value)
        )
    }
}
