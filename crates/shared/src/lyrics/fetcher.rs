use std::path::PathBuf;

use anyhow::{Context, Result};
use compact_str::CompactString;
use lofty::config::ParseOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use url::Url;

use crate::lyrics::parser::parse_lyrics;
use crate::models::LyricLine;

/// 从歌曲元数据中获取歌词。没有本地文件或同步歌词时返回 `Ok(None)`。
pub fn get_lyrics(url: &Option<CompactString>) -> Result<Option<Vec<LyricLine>>> {
    let Some(music_path) = url.as_deref().and_then(file_url_to_path) else {
        return Ok(None);
    };
    let options = ParseOptions::new()
        .read_properties(false)
        .read_cover_art(false);
    let tagged_file = Probe::open(&music_path)
        .with_context(|| format!("Could not open audio file {}", music_path.display()))?
        .options(options)
        .read()
        .with_context(|| format!("Could not read tags from {}", music_path.display()))?;
    let candidates = tagged_file
        .primary_tag()
        .into_iter()
        .chain(tagged_file.tags().iter())
        .filter_map(|tag| tag.get_string(&ItemKey::Lyrics));
    Ok(first_synced_lyrics(candidates))
}

fn first_synced_lyrics<'a>(
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<Vec<LyricLine>> {
    candidates.into_iter().find_map(|raw_lyrics| {
        let mut lyrics = parse_lyrics(raw_lyrics.trim_start_matches('\u{feff}'));
        lyrics.sort_by_key(|line| line.timestamp);

        (!lyrics.is_empty()).then_some(lyrics)
    })
}

fn file_url_to_path(value: &str) -> Option<PathBuf> {
    let url = Url::parse(value).ok()?;
    if url.scheme() != "file" {
        return None;
    }

    url.to_file_path().ok()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{file_url_to_path, first_synced_lyrics};

    #[test]
    fn converts_percent_encoded_file_url() {
        assert_eq!(
            file_url_to_path("file:///tmp/a%20b.mp3"),
            Some(PathBuf::from("/tmp/a b.mp3"))
        );
    }

    #[test]
    fn converts_utf8_file_url() {
        assert_eq!(
            file_url_to_path("file:///tmp/%E6%AD%8C%E6%9B%B2.flac"),
            Some(PathBuf::from("/tmp/歌曲.flac"))
        );
    }

    #[test]
    fn rejects_non_file_and_invalid_urls() {
        assert_eq!(file_url_to_path("https://example.com/song.flac"), None);
        assert_eq!(file_url_to_path("not a url"), None);
    }

    #[test]
    fn skips_unsynchronized_candidates_before_valid_lrc() {
        let lyrics = first_synced_lyrics(["plain text", "", "[00:01.000]synced"]);
        let lyrics = lyrics.expect("第三个候选应提供同步歌词");
        assert_eq!(lyrics.len(), 1);
        assert_eq!(lyrics[0].text, "synced");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_local_file_host() {
        assert_eq!(file_url_to_path("file://example.com/tmp/song.flac"), None);
    }
}
