use crate::models::{LyricLine, LyricsStatus};

const POSITION_OFFSET_SECONDS: f64 = 0.1;

pub fn get_lyrics_status(lyrics: &[LyricLine], position: f64) -> LyricsStatus {
    // Include offset in position comparison
    let adjusted_position = position - POSITION_OFFSET_SECONDS;

    let current_index = lyrics
        .iter()
        .enumerate()
        .take_while(|(_, line)| line.timestamp <= adjusted_position)
        .map(|(i, _)| i)
        .last();

    match current_index {
        Some(i) => {
            let current_line = &lyrics[i];

            // 检查是否有下一行
            let (next_line, next_timestamp, _) = if i < lyrics.len() - 1 {
                let next = &lyrics[i + 1];

                {
                    (
                        next.text.clone(),
                        Some(next.timestamp),
                        current_line.translation.clone(),
                    )
                }
            } else {
                (String::new(), None, None)
            };

            LyricsStatus {
                current_line: current_line.clone(),
                next_line,
                next_timestamp,
            }
        }
        None => {
            // No current line found, check if there's an upcoming line
            if !lyrics.is_empty() {
                LyricsStatus {
                    current_line: LyricLine {
                        timestamp: 0.0,
                        text: String::new(),
                        translation: None,
                    },
                    next_line: lyrics[0].text.clone(),
                    next_timestamp: Some(lyrics[0].timestamp),
                }
            } else {
                LyricsStatus {
                    current_line: LyricLine {
                        timestamp: 0.0,
                        text: String::new(),
                        translation: None,
                    },
                    next_line: String::new(),
                    next_timestamp: None,
                }
            }
        }
    }
}

pub fn format_lyrics_for_tooltip(lyrics: &[LyricLine]) -> String {
    lyrics
        .iter()
        .map(|line| format!("{:#?}", line))
        .collect::<Vec<String>>()
        .join("\n")
}
