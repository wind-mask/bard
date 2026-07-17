use std::sync::LazyLock;

use regex::Regex;

use crate::models::lyrics::LyricLine;

const TIMESTAMP_EPSILON: f64 = 1e-9;

static LINE_TIMESTAMP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[(\d+):(\d+)(?:\.(\d+))?\](.*)$").expect("歌词行时间戳正则应有效")
});
static WORD_TIMESTAMP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\d+:\d+(?:\.\d+)?>").expect("逐词时间戳正则应有效"));

struct ParsedLine {
    lyric: LyricLine,
    has_timestamp: bool,
}

pub fn parse_lyrics(lyrics_text: &str) -> Vec<LyricLine> {
    let mut lines = lyrics_text
        .lines()
        .filter_map(parse_line)
        .collect::<Vec<_>>();

    lines.sort_by(|a, b| a.lyric.timestamp.total_cmp(&b.lyric.timestamp));

    let mut merged: Vec<ParsedLine> = Vec::with_capacity(lines.len());
    for line in lines {
        if line.has_timestamp
            && let Some(previous) = merged.last_mut()
            && previous.has_timestamp
            && previous.lyric.translation.is_none()
            && (previous.lyric.timestamp - line.lyric.timestamp).abs() < TIMESTAMP_EPSILON
            && previous.lyric.text != line.lyric.text
        {
            previous.lyric.translation = Some(line.lyric.text);
            continue;
        }

        merged.push(line);
    }

    merged.into_iter().map(|line| line.lyric).collect()
}

fn parse_line(line: &str) -> Option<ParsedLine> {
    if let Some(captures) = LINE_TIMESTAMP_REGEX.captures(line) {
        let timestamp = parse_timestamp(
            captures.get(1)?.as_str(),
            captures.get(2)?.as_str(),
            captures.get(3).map(|fraction| fraction.as_str()),
        )?;
        let text = extract_clean_text(captures.get(4)?.as_str());
        if text.is_empty() {
            return None;
        }

        return Some(ParsedLine {
            lyric: LyricLine {
                timestamp,
                text,
                translation: None,
            },
            has_timestamp: true,
        });
    }

    let text = line.trim();
    if text.is_empty() || text.starts_with('[') {
        return None;
    }

    Some(ParsedLine {
        lyric: LyricLine {
            timestamp: 0.0,
            text: text.to_string(),
            translation: None,
        },
        has_timestamp: false,
    })
}

fn parse_timestamp(minutes: &str, seconds: &str, fraction: Option<&str>) -> Option<f64> {
    let minutes = minutes.parse::<u64>().ok()?;
    let seconds = seconds.parse::<u64>().ok()?;
    let fraction = match fraction {
        Some(fraction) => {
            let value = fraction.parse::<u64>().ok()?;
            let scale = 10_u64.checked_pow(fraction.len().try_into().ok()?)?;
            value as f64 / scale as f64
        }
        None => 0.0,
    };

    Some(minutes as f64 * 60.0 + seconds as f64 + fraction)
}

fn extract_clean_text(content: &str) -> String {
    WORD_TIMESTAMP_REGEX
        .replace_all(content, "")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {

    use super::parse_lyrics;

    fn assert_timestamp(input: &str, expected: f64) {
        let lyrics = parse_lyrics(input);
        assert_eq!(lyrics.len(), 1);
        assert!((lyrics[0].timestamp - expected).abs() < 1e-9);
    }

    #[test]
    fn parses_one_two_and_three_digit_fractions() {
        assert_timestamp("[01:02.1]one", 62.1);
        assert_timestamp("[01:02.12]two", 62.12);
        assert_timestamp("[01:02.123]three", 62.123);
    }

    #[test]
    fn merges_translation_with_equivalent_fraction_precision() {
        let lyrics = parse_lyrics("[00:01.1]原文\n[00:01.100]translation");

        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].text, "原文");
        assert_eq!(lyrics[0].translation.as_deref(), Some("translation"));
    }

    #[test]
    fn does_not_merge_distinct_timestamps() {
        let lyrics = parse_lyrics("[00:01.100]A\n[00:02.100]B");

        assert_eq!(lyrics.len(), 2);
        assert!(lyrics.iter().all(|line| line.translation.is_none()));
    }

    #[test]
    fn preserves_duplicate_text_at_the_same_timestamp() {
        let lyrics = parse_lyrics("[00:01.100]same\n[00:01.100]same");

        assert_eq!(lyrics.len(), 2);
        assert!(lyrics.iter().all(|line| line.translation.is_none()));
    }

    #[test]
    fn removes_word_timestamps() {
        let lyrics = parse_lyrics("[00:01.230]<00:01.230>Hel<00:01.500>lo");

        assert_eq!(lyrics[0].text, "Hello");
        assert!((lyrics[0].timestamp - 1.23).abs() < 1e-9);
    }

    #[test]
    fn removes_word_timestamps_with_mixed_precision_and_unicode() {
        let lyrics = parse_lyrics("[00:01.23]<00:01.2>你<00:01.23>好 <00:01.230>world");

        assert_eq!(lyrics[0].text, "你好 world");
    }

    #[test]
    fn plain_text_lines_are_not_merged_as_translations() {
        let lyrics = parse_lyrics("first\nsecond");

        assert_eq!(lyrics.len(), 2);
        assert!(lyrics.iter().all(|line| line.translation.is_none()));
    }
}
