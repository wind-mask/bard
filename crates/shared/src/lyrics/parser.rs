use crate::models::lyrics::LyricLine;
use regex::Regex;

pub fn parse_lyrics(lyrics_text: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let lines_vec: Vec<&str> = lyrics_text.lines().collect();

    // 正则表达式匹配时间戳行，例如 [01:23.456]歌词内容
    let timestamp_regex = Regex::new(r"\[(\d+):(\d+)\.(\d+)\](.*)").unwrap();

    let mut i = 0;
    while i < lines_vec.len() {
        let line = lines_vec[i];

        if let Some(caps) = timestamp_regex.captures(line) {
            let minutes: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
            let seconds: f64 = caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
            let centiseconds: f64 = caps.get(3).unwrap().as_str().parse().unwrap_or(0.0);
            let content = caps.get(4).unwrap().as_str();

            let timestamp = minutes * 60.0 + seconds + centiseconds / 1000.0;

            // 获取纯文本（去掉时间戳标记）
            let clean_text = extract_clean_text(content);

            // 检查下一行是否是相同时间戳的翻译
            let mut translation = None;
            if i + 1 < lines_vec.len() {
                let next_line = lines_vec[i + 1];
                if let Some(next_caps) = timestamp_regex.captures(next_line) {
                    let next_minutes: f64 =
                        next_caps.get(1).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_seconds: f64 =
                        next_caps.get(2).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_centiseconds: f64 =
                        next_caps.get(3).unwrap().as_str().parse().unwrap_or(0.0);
                    let next_timestamp =
                        next_minutes * 60.0 + next_seconds + next_centiseconds / 1000.0;

                    // 如果时间戳相同（允许很小的误差），认为是翻译
                    if (next_timestamp - timestamp).abs() < 0.01 {
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
    // 对没有翻译的行查找有无时间戳相同的翻译行
    // 先出现的作为原文，后出现的作为翻译，已匹配的不再重复寻找
    let mut used_as_translation = vec![false; lines.len()];
    let mut updates: Vec<(usize, usize)> = Vec::new(); // (原文索引, 翻译索引)

    for i in 0..lines.len() {
        if lines[i].translation.is_some() || used_as_translation[i] {
            continue;
        }
        for j in (i + 1)..lines.len() {
            if used_as_translation[j] || lines[j].translation.is_some() {
                continue;
            }
            if (lines[j].timestamp - lines[i].timestamp).abs() < 0.01
                && lines[j].text != lines[i].text
            {
                updates.push((i, j));
                used_as_translation[j] = true;
                break;
            }
        }
    }

    // 应用更新：将后出现的行文本设为翻译
    for &(orig, trans) in &updates {
        lines[orig].translation = Some(lines[trans].text.clone());
    }
    // 删除掉作为翻译的行（从后往前删以保持索引正确）
    let mut remove_indices: Vec<usize> = updates.iter().map(|&(_, t)| t).collect();
    remove_indices.sort_unstable_by(|a, b| b.cmp(a));
    for idx in remove_indices {
        lines.remove(idx);
    }
    
    lines
}

fn extract_clean_text(content: &str) -> String {
    // 移除所有时间戳标记，保留纯文本
    let word_timestamp_regex = Regex::new(r"<\d+:\d+\.\d+>").unwrap();
    word_timestamp_regex
        .replace_all(content, "")
        .trim()
        .to_string()
}
