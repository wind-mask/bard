use std::time::Duration;

use crate::models::{LyricLine, LyricsStatus};

pub fn get_lyrics_status(
    lyrics: &[LyricLine],
    position: Duration,
    display_offset_ms: i64,
) -> LyricsStatus<'_> {
    let next_index = lyrics
        .partition_point(|line| display_timestamp(line.timestamp, display_offset_ms) <= position);
    let current_line = next_index
        .checked_sub(1)
        .and_then(|index| lyrics.get(index));
    let next_line = lyrics.get(next_index);

    LyricsStatus {
        current_line,
        next_line,
        next_timestamp: next_line.map(|line| display_timestamp(line.timestamp, display_offset_ms)),
    }
}

fn display_timestamp(timestamp: Duration, offset_ms: i64) -> Duration {
    if offset_ms >= 0 {
        timestamp.saturating_add(Duration::from_millis(offset_ms as u64))
    } else {
        timestamp.saturating_sub(Duration::from_millis(offset_ms.unsigned_abs()))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::get_lyrics_status;
    use crate::models::LyricLine;

    fn lyrics() -> Vec<LyricLine> {
        vec![
            LyricLine {
                timestamp: Duration::from_secs(1),
                text: "first".to_string(),
                translation: None,
            },
            LyricLine {
                timestamp: Duration::from_secs(2),
                text: "second".to_string(),
                translation: None,
            },
        ]
    }

    #[test]
    fn returns_next_line_before_lyrics_start() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, Duration::from_millis(500), 100);

        assert!(status.current_line.is_none());
        assert!(std::ptr::eq(status.next_line.unwrap(), &lyrics[0]));
        assert_eq!(status.next_timestamp, Some(Duration::from_millis(1_100)));
    }

    #[test]
    fn respects_positive_and_negative_display_offsets() {
        let lyrics = lyrics();

        assert!(
            get_lyrics_status(&lyrics, Duration::from_secs(1), 100)
                .current_line
                .is_none()
        );
        assert!(std::ptr::eq(
            get_lyrics_status(&lyrics, Duration::from_millis(1_100), 100)
                .current_line
                .unwrap(),
            &lyrics[0]
        ));
        assert!(std::ptr::eq(
            get_lyrics_status(&lyrics, Duration::from_millis(900), -100)
                .current_line
                .unwrap(),
            &lyrics[0]
        ));
    }

    #[test]
    fn returns_borrowed_current_and_next_lines() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, Duration::from_millis(1_500), 0);

        assert!(std::ptr::eq(status.current_line.unwrap(), &lyrics[0]));
        assert!(std::ptr::eq(status.next_line.unwrap(), &lyrics[1]));
        assert_eq!(status.next_timestamp, Some(Duration::from_secs(2)));
    }

    #[test]
    fn returns_last_line_after_lyrics_end() {
        let lyrics = lyrics();
        let status = get_lyrics_status(&lyrics, Duration::from_secs(3), 0);

        assert!(std::ptr::eq(status.current_line.unwrap(), &lyrics[1]));
        assert!(status.next_line.is_none());
        assert!(status.next_timestamp.is_none());
    }

    #[test]
    fn handles_empty_lyrics() {
        let status = get_lyrics_status(&[], Duration::from_secs(1), 0);

        assert!(status.current_line.is_none());
        assert!(status.next_line.is_none());
        assert!(status.next_timestamp.is_none());
    }
}
