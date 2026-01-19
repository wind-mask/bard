use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;

use crate::lyrics::parser::parse_lyrics;
use crate::models::LyricLine;
use crate::models::SongInfo;

/// 从歌曲元数据中获取歌词
pub fn get_lyrics(song: &SongInfo) -> Option<Vec<LyricLine>> {
    if let Some(url) = &song.url {
        // url like "file:///home/user/Music/Artist - Title.mp3"
        let music_path = url.trim_start_matches("file://");
        use lofty::read_from_path;
        let lyrics = read_from_path(music_path)
            .ok()
            .as_ref()
            .and_then(|tagged_file| tagged_file.primary_tag())
            .and_then(|tag| tag.get_string(&ItemKey::Lyrics))
            .and_then(|s| parse_lyrics(s).into());
        if lyrics.is_some() {
            return lyrics;
        }
    }
    // No lyrics found
    None
}
