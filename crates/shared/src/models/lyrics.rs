#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp: f64,
    pub text: String,
    // pub words: Vec<WordTimestamp>,
    pub translation: Option<String>,
}

pub struct WordTimestamp {
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,
}

pub struct LyricsStatus {
    pub current_line: LyricLine,
    pub next_line: String,
    pub next_timestamp: Option<f64>,
    // pub current_word_index: Option<usize>,
    pub translation: Option<String>,
}
