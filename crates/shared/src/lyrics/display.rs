
use crate::models::{LyricLine, LyricsStatus};

const POSITION_OFFSET_SECONDS: f64 = 1.0;

pub fn get_lyrics_status(lyrics: &[LyricLine], position: f64) -> LyricsStatus {
    // Include offset in position comparison
    let adjusted_position = position + POSITION_OFFSET_SECONDS;

    let current_index = lyrics
        .iter()
        .enumerate()
        .take_while(|(_, line)| line.timestamp <= adjusted_position)
        .map(|(i, _)| i)
        .last();

    match current_index {
        Some(i) => {
            let current_line = &lyrics[i];

            // 找到当前正在唱的词
            // let current_word_index = current_line
            //     .words
            //     .iter()
            //     .enumerate()
            //     .find(|(_, word)| {
            //         adjusted_position >= word.start_time && adjusted_position < word.end_time
            //     })
            //     .map(|(idx, _)| idx);

            // 检查是否有下一行
            let (next_line, next_timestamp, translation) = if i < lyrics.len() - 1 {
               
                let next = &lyrics[i + 1];
                // // 检查下一行是否是翻译（时间戳相近且没有词时间戳）
                // if next.timestamp - current_line.timestamp < 1.0 && next.words.is_empty() {
                //     // 这是翻译行，找下一个真正的歌词行
                //     let actual_next = if i + 2 < lyrics.len() {
                //         Some(&lyrics[i + 2])
                //     } else {
                //         None
                //     };

                //     (
                //         actual_next.map(|l| l.text.clone()).unwrap_or_default(),
                //         actual_next.map(|l| l.timestamp),
                //         Some(next.text.clone()),
                //     )
                // } else 
                {
                
                    (next.text.clone(), Some(next.timestamp), current_line.translation.clone())
                }
            } 
            else {
                (String::new(), None, None)
            };

            LyricsStatus {
                current_line: current_line.clone(),
                next_line,
                next_timestamp,
                // current_word_index,
                translation,
            }
        }
        None => {
            // No current line found, check if there's an upcoming line
            if !lyrics.is_empty() {
                LyricsStatus {
                    current_line: LyricLine { timestamp: 0.0, text: String::new(), translation: None },
                    next_line: lyrics[0].text.clone(),
                    next_timestamp: Some(lyrics[0].timestamp),
                    translation: None,
                }
            } else {
                LyricsStatus {
                    current_line: LyricLine { timestamp: 0.0, text: String::new(), translation: None },
                    next_line: String::new(),
                    next_timestamp: None,
                    translation: None,
                }
            }
        }
    }
}

pub fn format_lyrics_for_tooltip(lyrics: &[LyricLine]) -> String {
    lyrics
        .iter()
        .map(|line| format!("{:#?}" , line))
        
        .collect::<Vec<String>>()
        .join("\n")
}

/// 获取带有逐字高亮的歌词文本
pub fn get_highlighted_lyrics(lyrics_status: &LyricsStatus, current_line: &LyricLine) -> String {
    // if let Some(word_idx) = lyrics_status.current_word_index {
    //     if word_idx < current_line.words.len() {
    //         // 构建高亮显示的歌词
    //         let mut result = String::new();
    //         for (i, word) in current_line.words.iter().enumerate() {
    //             if i == word_idx {
    //                 // 当前正在唱的词用特殊标记包围
    //                 result.push_str(&format!("【{}】", word.text));
    //             } else if i < word_idx {
    //                 // 已经唱过的词用不同标记
    //                 result.push_str(&format!("＊{}＊", word.text));
    //             } else {
    //                 // 还没唱到的词保持原样
    //                 result.push_str(&word.text);
    //             }
    //         }
    //         result
    //     } else {
    //         current_line.text.clone()
    //     }
    // } else
     {
        current_line.text.clone()
    }
}
