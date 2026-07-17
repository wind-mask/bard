use crate::models::{LyricLine, LyricsStatus};

const POSITION_OFFSET_SECONDS: f64 = 0.1;

pub fn get_lyrics_status(lyrics: &[LyricLine], position: f64) -> LyricsStatus<'_> {
    let adjusted_position = position - POSITION_OFFSET_SECONDS;
    let next_index = lyrics.partition_point(|line| line.timestamp <= adjusted_position);
    let current_line = next_index
        .checked_sub(1)
        .and_then(|index| lyrics.get(index));
    let next_line = lyrics.get(next_index);

    LyricsStatus {
        current_line,
        next_line,
        // 歌词切换使用了显示偏移，调度时间也必须包含同一偏移，避免边界前反复唤醒。
        next_timestamp: next_line.map(|line| line.timestamp + POSITION_OFFSET_SECONDS),
    }
}

#[cfg(test)]
mod tests {
    use super::get_lyrics_status;
    use crate::models::LyricLine;

    fn lyrics() -> Vec<LyricLine> {
        vec![
            LyricLine {
                timestamp: 1.0,
                text: "first".to_string(),
                translation: None,
            },
            LyricLine {
                timestamp: 2.0,
                text: "second".to_string(),
                translation: None,
            },
        ]
    }

    #[test]
    fn returns_next_line_before_lyrics_start() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, 0.5);

        assert!(status.current_line.is_none());
        assert!(std::ptr::eq(status.next_line.unwrap(), &lyrics[0]));
        assert_eq!(status.next_timestamp, Some(1.1));
    }

    #[test]
    fn respects_display_offset_at_line_boundary() {
        let lyrics = lyrics();

        assert!(get_lyrics_status(&lyrics, 1.0).current_line.is_none());
        assert!(std::ptr::eq(
            get_lyrics_status(&lyrics, 1.1).current_line.unwrap(),
            &lyrics[0]
        ));
    }

    #[test]
    fn returns_borrowed_current_and_next_lines() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, 1.5);

        assert!(std::ptr::eq(status.current_line.unwrap(), &lyrics[0]));
        assert!(std::ptr::eq(status.next_line.unwrap(), &lyrics[1]));
        assert_eq!(status.next_timestamp, Some(2.1));
    }

    #[test]
    fn returns_last_line_after_lyrics_end() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, 3.0);

        assert!(std::ptr::eq(status.current_line.unwrap(), &lyrics[1]));
        assert!(status.next_line.is_none());
        assert!(status.next_timestamp.is_none());
    }

    #[test]
    fn handles_empty_lyrics() {
        let status = get_lyrics_status(&[], 1.0);

        assert!(status.current_line.is_none());
        assert!(status.next_line.is_none());
        assert!(status.next_timestamp.is_none());
    }
}
