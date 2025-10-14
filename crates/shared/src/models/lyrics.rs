use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp: f64,
    pub text: String,
    pub translation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LyricsStatus {
    pub current_line: LyricLine,
    pub next_line: String,
    pub next_timestamp: Option<f64>,
}
impl Display for LyricLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:.2}] {}", self.timestamp, self.text)
    }
}
