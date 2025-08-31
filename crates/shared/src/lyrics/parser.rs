use crate::models::lyrics::{LyricLine, WordTimestamp};
use regex::Regex;

pub fn parse_lyrics(lyrics_text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let lines_vec: Vec<&str> = lyrics_text.lines().collect();

    // 支持两种格式的正则表达式
    let timestamp_regex = Regex::new(r"\[(\d+):(\d+)\.(\d+)\](.*)").unwrap();
    let word_timestamp_regex = Regex::new(r"<(\d+):(\d+)\.(\d+)>([^<]*)").unwrap();

    let mut i = 0;
    while i < lines_vec.len() {
        let line = lines_vec[i];

        if let Some(caps) = timestamp_regex.captures(line) {
            let minutes: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
            let seconds: f64 = caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
            let centiseconds: f64 = caps.get(3).unwrap().as_str().parse().unwrap_or(0.0);
            let content = caps.get(4).unwrap().as_str();

            let timestamp = minutes * 60.0 + seconds + centiseconds / 100.0;

            // 检查是否包含逐字时间戳
            // let words = if content.contains('<') {
            //     parse_word_timestamps(content, &word_timestamp_regex)
            // } else {
            //     // 如果没有逐字时间戳，创建一个简单的词
            //     vec![WordTimestamp {
            //         start_time: timestamp,
            //         end_time: timestamp + 1.0, // 假设持续1秒
            //         text: content.trim().to_string(),
            //     }]
            // };

            // 获取纯文本（去掉时间戳标记）
            let clean_text = extract_clean_text(content);

            // 检查下一行是否是相同时间戳的翻译
            let mut translation = None;
            if i + 1 < lines_vec.len() {
                let next_line = lines_vec[i + 1];
                if let Some(next_caps) = timestamp_regex.captures(next_line) {
                    let next_minutes: f64 = next_caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_seconds: f64 = next_caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_centiseconds: f64 = next_caps.get(3).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_timestamp = next_minutes * 60.0 + next_seconds + next_centiseconds / 100.0;
                    
                    // 如果时间戳相同（允许很小的误差），认为是翻译
                    
                    if next_timestamp ==timestamp 
                    {
                        // dbg!("Found translation line:", next_line);

                        let next_content = next_caps.get(4).unwrap().as_str();
                        let next_clean_text = extract_clean_text(next_content);
                        if !next_clean_text.is_empty() {
                            translation = Some(next_clean_text);
                            i += 1; // 跳过翻译行
                        }
                    }
                }
            }

            if !clean_text.is_empty() {
                lines.push(LyricLine {
                    timestamp,
                    text: clean_text,
                    translation,
                });
            }
        } else if !line.trim().is_empty() && !line.starts_with('[') {
            // 对于非时间戳行（可能是纯文本歌词或翻译），如果不是翻译就添加
            lines.push(LyricLine {
                timestamp: 0.0,
                text: line.trim().to_string(),
                translation: None,
            });
        }

        i += 1;
    }

    // 按时间戳排序
    lines.sort_by(|a, b| a.timestamp.partial_cmp(&b.timestamp).unwrap());
    lines
}

fn parse_word_timestamps(content: &str, regex: &Regex) -> Vec<WordTimestamp> {
    let mut words = Vec::new();

    for caps in regex.captures_iter(content) {
        let minutes: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
        let seconds: f64 = caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
        let centiseconds: f64 = caps.get(3).unwrap().as_str().parse().unwrap_or(0.0);
        let text = caps.get(4).unwrap().as_str();

        let start_time = minutes * 60.0 + seconds + centiseconds / 100.0;

        if !text.trim().is_empty() {
            words.push(WordTimestamp {
                start_time,
                end_time: start_time + 0.5, // 默认每个词持续0.5秒
                text: text.trim().to_string(),
            });
        }
    }

    // 计算真实的结束时间（下一个词的开始时间）
    for i in 0..words.len().saturating_sub(1) {
        words[i].end_time = words[i + 1].start_time;
    }

    words
}

fn extract_clean_text(content: &str) -> String {
    // 移除所有时间戳标记，保留纯文本
    let word_timestamp_regex = Regex::new(r"<\d+:\d+\.\d+>").unwrap();
    word_timestamp_regex
        .replace_all(content, "")
        .trim()
        .to_string()
}
