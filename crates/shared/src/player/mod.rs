pub mod fetcher;

pub use fetcher::{
    find_active_player, find_playing_player, get_current_song, song_from_player,
    wait_for_mpris_player,
};
