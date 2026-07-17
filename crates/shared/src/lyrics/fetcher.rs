use std::path::PathBuf;

use lofty::config::ParseOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use url::Url;

use crate::lyrics::parser::parse_lyrics;
use crate::models::{LyricLine, SongInfo};

/// 从歌曲元数据中获取歌词。
pub fn get_lyrics(song: &SongInfo) -> Option<Vec<LyricLine>> {
    let music_path = song.url.as_deref().and_then(file_url_to_path)?;
    let options = ParseOptions::new()
        .read_properties(false)
        .read_cover_art(false);
    let tagged_file = Probe::open(music_path).ok()?.options(options).read().ok()?;
    let raw_lyrics = tagged_file
        .primary_tag()
        .and_then(|tag| tag.get_string(&ItemKey::Lyrics))?;
    let raw_lyrics = raw_lyrics.trim_start_matches('\u{feff}');
    let lyrics = parse_lyrics(raw_lyrics);
    (!lyrics.is_empty()).then_some(lyrics)
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

    use super::file_url_to_path;

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

    #[cfg(unix)]
    #[test]
    fn rejects_non_local_file_host() {
        assert_eq!(file_url_to_path("file://example.com/tmp/song.flac"), None);
    }
}
