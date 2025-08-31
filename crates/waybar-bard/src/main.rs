use anyhow::Result;
use shared::config::Config;
use shared::lyrics::{format_lyrics_for_tooltip, get_lyrics, get_lyrics_status};
use shared::models::{LyricLine, SongInfo, SongStatus};
use shared::player;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::runtime::Runtime;
use std::sync::atomic::{AtomicBool, Ordering};
use signal_hook::{consts::SIGUSR1, iterator::Signals};

mod models;
mod waybar;

fn main() -> () {
    // Load configuration
    let config = Config::load().expect("Failed to load config");

    // Create a Tokio runtime
    let rt = Runtime::new().expect("Failed to create Tokio runtime");

    // Track current song path
    let current_song_id = Arc::new(Mutex::new(String::new()));
    let lyrics = Arc::new(Mutex::new(Result::<Option<Vec<LyricLine>>>::Ok(None)));
    
    // State to track if output should be hidden
    let hidden = Arc::new(AtomicBool::new(false));

    // Setup signal handler for SIGUSR1
    let hidden_clone = hidden.clone();
    thread::spawn(move || {
        let mut signals = Signals::new(&[SIGUSR1]).expect("Failed to register signal handler");
        for _sig in signals.forever() {
            // Toggle hidden state
            let current = hidden_clone.load(Ordering::Relaxed);
            hidden_clone.store(!current, Ordering::Relaxed);
            if current == false {
                // If now hidden, clear the output
                waybar::render_empty();
            }else {
                waybar::render_just();
            }
            eprintln!("waybar-bard: Toggled hidden state to {}", !current);
        }
    });

    // Main loop
    loop {
        // Check if output should be hidden
        if hidden.load(Ordering::Relaxed) {
            waybar::render_empty();
            thread::sleep(Duration::from_secs(1));
            continue;
        }

        // Get current song info from player
        let song_info = player::get_current_song(&config);

        match song_info {
            Ok(Some(song)) => {
                if song.id != current_song_id.lock().unwrap().as_str() {
                    // Song changed
                    current_song_id.lock().unwrap().clear();
                    current_song_id.lock().unwrap().push_str(&song.id);
                    // Update lyrics
                    *lyrics.lock().unwrap() = rt.block_on(get_lyrics(&song));
                }

                if let Err(e) = update_lyrics(&lyrics.lock().unwrap(), &song) {
                    eprintln!("Error updating lyrics: {}", e);
                }
            }
            Ok(None) => {
                // No song playing
                waybar::render_no_song();
                thread::sleep(Duration::from_secs(1));
                continue;
            }
            Err(e) => {
                eprintln!("Error getting current song info: {}", e);
                waybar::render_no_song();
                thread::sleep(Duration::from_secs(2));
            }
        }
    }
}

// Update lyrics on screen
fn update_lyrics(lyrics_result: &Result<Option<Vec<LyricLine>>>, song: &SongInfo) -> Result<()> {
    match lyrics_result {
        Ok(Some(lyrics_data)) => {
            if song.status == SongStatus::Paused {
                waybar::render_song_info(song);
                thread::sleep(Duration::from_secs(1));
                return Ok(());
            }
            // Find current lyric line based on position
            let current_lyric = get_lyrics_status(lyrics_data, song.position);

            // 构建增强的tooltip，包含翻译
            let mut tooltip = format_lyrics_for_tooltip(lyrics_data);
            if let Some(translation) = &current_lyric.translation {
                tooltip = format!("{}\n\n当前翻译: {}", tooltip, translation);
            }

            // 渲染歌词，如果有翻译就显示翻译
            let display_current = if let Some(trans) = &current_lyric.translation {
                format!("{}\n{}", current_lyric.current_line, trans)
            } else {
                current_lyric.current_line.clone()
            };

            waybar::render_lyrics(display_current, current_lyric.next_line, tooltip);

            // Calculate sleep duration based on next lyric timestamp or word timing
            let sleep_duration = if let Some(next_timestamp) = current_lyric.next_timestamp {
                let time_until_next = next_timestamp - song.position;
                if time_until_next > 0.0 {
                    // 对于逐字歌词，使用更短的更新间隔
                    time_until_next.max(0.01).min(2.0)
                } else {
                    0.1
                }
            } else {
                // 对于逐字歌词，使用更频繁的更新
                if !lyrics_data.is_empty() && !lyrics_data[0].words.is_empty() {
                    0.2 // 200ms更新一次以支持逐字高亮
                } else {
                    2.0
                }
            };

            thread::sleep(Duration::from_secs_f64(sleep_duration));
        }
        Ok(None) => {
            // No lyrics found
            waybar::render_song_info(song);
            thread::sleep(Duration::from_secs(2));
        }
        Err(e) => {
            eprintln!("Error getting lyrics: {}", e);
            // Error getting lyrics
            waybar::render_song_info(song);
            thread::sleep(Duration::from_secs(2));
        }
    }

    Ok(())
}
