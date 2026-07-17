use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp: f64,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct LyricsStatus<'a> {
    pub current_line: Option<&'a LyricLine>,
    pub next_line: Option<&'a LyricLine>,
    pub next_timestamp: Option<f64>,
}
impl Display for LyricLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:.2}] {} {}",
            self.timestamp,
            self.text,
            self.translation.as_ref().map_or("", |v| v)
        )
    }
}
