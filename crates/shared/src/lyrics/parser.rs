use std::sync::LazyLock;
use std::time::Duration;

use regex::{Captures, Regex};

use crate::models::LyricLine;

static LINE_TIMESTAMP_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\[(\d+):([0-5]?\d)(?:\.(\d{1,3}))?\]").expect("歌词行时间戳正则应有效")
});
static OFFSET_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^\[offset:([+-]?\d+)\]$").expect("歌词偏移正则应有效"));
static WORD_TIMESTAMP_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<\d+:[0-5]?\d(?:\.\d{1,3})?>").expect("逐词时间戳正则应有效"));

#[derive(Debug)]
struct TimedText {
    timestamp: Duration,
    text: String,
}

pub fn parse_lyrics(lyrics_text: &str) -> Vec<LyricLine> {
    let lyrics_text = lyrics_text.trim_start_matches('\u{feff}');
    let file_offset_ms = lyrics_text
        .lines()
        .filter_map(parse_offset)
        .next_back()
        .unwrap_or(0);

    let mut timed = Vec::new();
    for line in lyrics_text.lines() {
        parse_timed_line(line, &mut timed);
    }
    // 稳定排序会保留同一原始时间戳在文件中的先后顺序。先合并，再应用偏移，
    // 避免多个负偏移时间戳饱和到零后被误判成翻译。
    timed.sort_by_key(|line| line.timestamp);
    let mut lyrics = merge_same_timestamp(timed);
    for line in &mut lyrics {
        line.timestamp = apply_offset(line.timestamp, file_offset_ms);
    }
    lyrics
}

fn parse_offset(line: &str) -> Option<i64> {
    let captures = OFFSET_REGEX.captures(line.trim())?;
    captures.get(1)?.as_str().parse().ok()
}

fn parse_timed_line(line: &str, output: &mut Vec<TimedText>) {
    let mut remaining = line.trim_start();
    let mut timestamps = Vec::new();

    while let Some(captures) = LINE_TIMESTAMP_REGEX.captures(remaining) {
        let Some(timestamp) = parse_timestamp(&captures) else {
            return;
        };
        let end = captures.get(0).expect("完整时间戳捕获应存在").end();
        timestamps.push(timestamp);
        remaining = &remaining[end..];
    }

    if timestamps.is_empty() {
        return;
    }
    let text = extract_clean_text(remaining);
    if text.is_empty() {
        return;
    }

    output.extend(timestamps.into_iter().map(|timestamp| TimedText {
        timestamp,
        text: text.clone(),
    }));
}

fn parse_timestamp(captures: &Captures<'_>) -> Option<Duration> {
    let minutes = captures.get(1)?.as_str().parse::<u64>().ok()?;
    let seconds = captures.get(2)?.as_str().parse::<u64>().ok()?;
    if seconds >= 60 {
        return None;
    }
    let fraction_ms = match captures.get(3).map(|value| value.as_str()) {
        Some(value) => {
            let parsed = value.parse::<u64>().ok()?;
            match value.len() {
                1 => parsed.checked_mul(100)?,
                2 => parsed.checked_mul(10)?,
                3 => parsed,
                _ => return None,
            }
        }
        None => 0,
    };
    let total_ms = minutes
        .checked_mul(60_000)?
        .checked_add(seconds.checked_mul(1_000)?)?
        .checked_add(fraction_ms)?;
    Some(Duration::from_millis(total_ms))
}

fn apply_offset(timestamp: Duration, offset_ms: i64) -> Duration {
    if offset_ms >= 0 {
        timestamp.saturating_add(Duration::from_millis(offset_ms as u64))
    } else {
        timestamp.saturating_sub(Duration::from_millis(offset_ms.unsigned_abs()))
    }
}

fn extract_clean_text(content: &str) -> String {
    WORD_TIMESTAMP_REGEX
        .replace_all(content, "")
        .trim()
        .to_string()
}

fn merge_same_timestamp(lines: Vec<TimedText>) -> Vec<LyricLine> {
    let mut merged = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let timestamp = lines[index].timestamp;
        let mut texts: Vec<&str> = Vec::new();
        while index < lines.len() && lines[index].timestamp == timestamp {
            let text = lines[index].text.as_str();
            if !texts.contains(&text) {
                texts.push(text);
            }
            index += 1;
        }
        let Some((primary, auxiliary)) = texts.split_first() else {
            continue;
        };
        merged.push(LyricLine {
            timestamp,
            text: (*primary).to_string(),
            translation: (!auxiliary.is_empty()).then(|| auxiliary.join(" / ")),
        });
    }
    merged
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::parse_lyrics;

    fn assert_timestamp(input: &str, expected_ms: u64) {
        let lyrics = parse_lyrics(input);
        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].timestamp, Duration::from_millis(expected_ms));
    }

    #[test]
    fn parses_one_two_and_three_digit_fractions() {
        assert_timestamp("[01:02.1]one", 62_100);
        assert_timestamp("[01:02.12]two", 62_120);
        assert_timestamp("[01:02.123]three", 62_123);
    }

    #[test]
    fn expands_multiple_timestamps_on_one_line() {
        let lyrics = parse_lyrics("[00:01.1][00:02.20]repeat");
        assert_eq!(lyrics.len(), 2);
        assert_eq!(lyrics[0].timestamp, Duration::from_millis(1_100));
        assert_eq!(lyrics[1].timestamp, Duration::from_millis(2_200));
        assert!(lyrics.iter().all(|line| line.text == "repeat"));
    }

    #[test]
    fn applies_positive_and_negative_file_offsets() {
        assert_timestamp("[offset:250]\n[00:01.000]line", 1_250);
        assert_timestamp("[offset:-250]\n[00:01.000]line", 750);
        assert_timestamp("[offset:-2000]\n[00:01.000]line", 0);
    }

    #[test]
    fn negative_offset_clamping_does_not_merge_distinct_lines() {
        let lyrics = parse_lyrics("[offset:-2000]\n[00:00.500]first\n[00:01.500]second");
        assert_eq!(lyrics.len(), 2);
        assert_eq!(lyrics[0].timestamp, Duration::ZERO);
        assert_eq!(lyrics[1].timestamp, Duration::ZERO);
        assert!(lyrics.iter().all(|line| line.translation.is_none()));
    }

    #[test]
    fn merges_and_deduplicates_auxiliary_lines() {
        let lyrics = parse_lyrics(
            "[00:01.100]原文\n[00:01.1]translation\n[00:01.100]romaji\n[00:01.100]translation",
        );
        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].text, "原文");
        assert_eq!(
            lyrics[0].translation.as_deref(),
            Some("translation / romaji")
        );
    }

    #[test]
    fn removes_word_timestamps_with_unicode() {
        let lyrics = parse_lyrics("[00:01.230]<00:01.2>你<00:01.23>好 <00:01.230>world");
        assert_eq!(lyrics[0].text, "你好 world");
        assert_eq!(lyrics[0].timestamp, Duration::from_millis(1_230));
    }

    #[test]
    fn ignores_metadata_plain_text_invalid_seconds_and_empty_lines() {
        let lyrics = parse_lyrics(
            "[ar:Artist]\n[ti:Title]\nplain text\n[00:60.0]invalid\n[00:01.0]   \n[00:02.0]valid",
        );
        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].text, "valid");
    }

    #[test]
    fn strips_utf8_bom() {
        assert_timestamp("\u{feff}[00:01.000]line", 1_000);
    }
}
