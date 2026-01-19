use anyhow::Result;
use shared::lyrics::{get_lyrics, get_lyrics_status};
use shared::models::{LyricLine, SongInfo, SongStatus};
use shared::player;
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

mod models;
mod waybar;

struct AppState {
    song: Option<SongInfo>,
    lyrics: Option<Vec<LyricLine>>,
    last_update_time: Instant,
}

fn main() -> Result<()> {
    // Shared state between fetcher and renderer
    let state = Arc::new(RwLock::new(AppState {
        song: None,
        lyrics: None,
        last_update_time: Instant::now(),
    }));

    // State to track if output should be hidden
    let hidden = Arc::new(AtomicBool::new(false));

    // --- Signal Handler Thread ---
    let hidden_clone = hidden.clone();
    thread::spawn(move || {
        let mut signals = Signals::new([SIGUSR1]).expect("Failed to register signal handler");
        for _sig in signals.forever() {
            let current = hidden_clone.load(Ordering::Relaxed);
            hidden_clone.store(!current, Ordering::Relaxed);
            eprintln!("waybar-bard: Toggled hidden state to {}", !current);
            // We don't force render here; the high-frequency render loop will pick it up instantly
        }
    });

    // --- Data Fetcher Thread (Background) ---
    // Handles slow I/O: DBus and File Reading
    let state_updater = state.clone();
    thread::spawn(move || {
        let mut last_song_id = String::new();
        let mut poll_interval;

        loop {
            let loop_start = Instant::now();

            match player::get_current_song() {
                Ok(Some(song)) => {
                    let mut new_lyrics = None;
                    poll_interval = Duration::from_secs(1); // Active mode

                    // Only fetch lyrics if song changed
                    if song.id != last_song_id {
                        match get_lyrics(&song) {
                            Some(lyrics) => {
                                new_lyrics = Some(lyrics);
                            }
                            None => {
                                new_lyrics = None;
                            }
                        }
                        last_song_id = song.id.clone();
                    }

                    // Update shared state
                    if let Ok(mut writer) = state_updater.write() {
                        writer.song = Some(song);
                        writer.last_update_time = Instant::now();
                        if new_lyrics.is_some() {
                            writer.lyrics = new_lyrics;
                        }
                    }
                }
                Ok(None) => {
                    if let Ok(mut writer) = state_updater.write() {
                        writer.song = None;
                        writer.lyrics = None;
                    }
                    last_song_id.clear();
                    poll_interval = Duration::from_secs(5); // Idle mode: no player found
                }
                Err(e) => {
                    eprintln!("Error getting song info: {}", e);
                    poll_interval = Duration::from_secs(2); // Error recovery mode
                }
            }

            // Adaptive sleep
            let elapsed = loop_start.elapsed();
            if elapsed < poll_interval {
                thread::sleep(poll_interval - elapsed);
            }
        }
    });

    // --- Main Render Loop (Foreground) ---
    // Handles UI output. Non-blocking.
    loop {
        // 1. Check hidden state
        if hidden.load(Ordering::Relaxed) {
            waybar::render_empty();
            thread::sleep(Duration::from_millis(500));
            continue;
        }

        let mut sleep_duration = Duration::from_millis(200);

        // 2. Read state and render
        // Use a read lock, which is fast and allows multiple readers if needed
        if let Ok(reader) = state.read() {
            match &reader.song {
                Some(song) => {
                    if song.status == SongStatus::Paused {
                        waybar::render_song_info(song);
                        sleep_duration = Duration::from_millis(500);
                    } else {
                        // Interpolate position: DBus Position + Time since DBus update
                        let elapsed_since_update = reader.last_update_time.elapsed().as_secs_f64();
                        let current_position = song.position + elapsed_since_update;

                        match &reader.lyrics {
                            Some(lyrics_data) => {
                                let current_lyric =
                                    get_lyrics_status(lyrics_data, current_position);

                                let display_text = &current_lyric.current_line.text;
                                let next_text = current_lyric
                                    .current_line
                                    .translation
                                    .clone()
                                    .unwrap_or_else(|| current_lyric.next_line.clone());

                                waybar::render_lyrics(display_text, next_text, String::new());

                                // Calculate dynamic sleep to sync with next line
                                if let Some(next_ts) = current_lyric.next_timestamp {
                                    let time_until_next = next_ts - current_position;
                                    if time_until_next > 0.0 {
                                        // Sleep until next line, but cap at 0.5s for responsiveness
                                        // And floor at 0.05s to avoid busy looping
                                        sleep_duration = Duration::from_secs_f64(
                                            time_until_next.clamp(0.05, 0.5),
                                        );
                                    } else {
                                        sleep_duration = Duration::from_millis(50);
                                    }
                                }
                            }
                            None => {
                                waybar::render_song_info(song);
                                sleep_duration = Duration::from_secs(1);
                            }
                        }
                    }
                }
                None => {
                    waybar::render_no_song();
                    sleep_duration = Duration::from_secs(1);
                }
            }
        }

        thread::sleep(sleep_duration);
    }
}
