use anyhow::{Context, Result, bail};
use compact_str::{CompactString, ToCompactString};
use shared::lyrics::{get_lyrics, get_lyrics_status};
use shared::models::{LyricLine, SongInfo, SongStatus};
use shared::player;
use signal_hook::{consts::SIGUSR1, iterator::Signals};
use std::io::{self, BufWriter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc, RwLock,
    mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
};
use std::thread;
use std::time::{Duration, Instant};
use zbus::{
    MatchRule,
    blocking::{Connection, MessageIterator},
    message::Type,
};

use crate::waybar::{RenderedFrame, render_if_changed};

mod models;
mod waybar;

const POSITION_CONFIRM_DELAY: Duration = Duration::from_millis(100);
const POSITION_RETRY_LIMIT: usize = 4;

struct AppState {
    song: Option<SongInfo>,
    lyrics: Option<Vec<LyricLine>>,
    last_update_time: Instant,
    player_unique_name: Option<CompactString>,
}

fn notify_render(render_tx: &SyncSender<()>) -> bool {
    match render_tx.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) => true,
        Err(TrySendError::Disconnected(())) => false,
    }
}

fn wait_for_render_change(render_rx: &Receiver<()>, timeout: Option<Duration>) -> bool {
    match timeout {
        Some(timeout) => match render_rx.recv_timeout(timeout) {
            Ok(()) => true,
            Err(RecvTimeoutError::Timeout) => {
                // timeout 与通知可能同时发生；容量 1 队列只需顺手消费一次。
                let _ = render_rx.try_recv();
                true
            }
            Err(RecvTimeoutError::Disconnected) => false,
        },
        None => render_rx.recv().is_ok(),
    }
}

fn next_lyric_timeout(next_timestamp: Option<f64>, current_position: f64) -> Option<Duration> {
    let time_until_next = next_timestamp? - current_position;
    Some(if time_until_next.is_finite() && time_until_next > 0.0 {
        Duration::from_secs_f64(time_until_next.max(0.01))
    } else {
        Duration::from_millis(50)
    })
}

fn render_current_state(
    state: &Arc<RwLock<AppState>>,
    hidden: &AtomicBool,
) -> (RenderedFrame, Option<Duration>) {
    if hidden.load(Ordering::Relaxed) {
        return (RenderedFrame::Hidden, None);
    }

    // 只在锁内构造拥有所有权的渲染帧；JSON 和 stdout I/O 在返回后执行。
    let reader = state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(song) = &reader.song else {
        return (RenderedFrame::NoSong, None);
    };

    if song.status != SongStatus::Playing {
        return (RenderedFrame::NoSong, None);
    }

    let current_position = song.position + reader.last_update_time.elapsed().as_secs_f64();
    match &reader.lyrics {
        Some(lyrics) => {
            let status = get_lyrics_status(lyrics, current_position);
            let current = status
                .current_line
                .map(|line| line.text.as_str())
                .unwrap_or("");
            let next = status
                .current_line
                .and_then(|line| line.translation.as_deref())
                .or_else(|| status.next_line.map(|line| line.text.as_str()))
                .unwrap_or("");
            let frame = RenderedFrame::Lyrics {
                current: current.to_compact_string(),
                next: next.to_compact_string(),
            };
            let timeout = next_lyric_timeout(status.next_timestamp, current_position);
            (frame, timeout)
        }
        None => (
            RenderedFrame::SongInfo {
                artist: song.artist.clone(),
                title: song.title.clone(),
            },
            None,
        ),
    }
}

fn should_reload_lyrics(
    song_changed: bool,
    current_source_url: Option<&str>,
    song_url: Option<&str>,
) -> bool {
    song_changed || song_url.is_some_and(|url| current_source_url != Some(url))
}

fn set_song_state(
    state: &Arc<RwLock<AppState>>,
    last_song_id: &mut CompactString,
    last_song_url: &mut Option<CompactString>,
    song: SongInfo,
    player_unique_name: &str,
    render_tx: &SyncSender<()>,
) -> bool {
    let song_changed = song.id != *last_song_id;
    let reload_lyrics =
        should_reload_lyrics(song_changed, last_song_url.as_deref(), song.url.as_deref());
    // 本地歌词读取保持同步且简单，但必须在状态锁外执行。
    let lyrics_update = if reload_lyrics {
        Some(get_lyrics(&song))
    } else {
        None
    };
    let new_song_id = song.id.clone();
    let new_song_url = song.url.clone();

    {
        let mut writer = state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        writer.song = Some(song);
        writer.player_unique_name = Some(player_unique_name.to_compact_string());
        writer.last_update_time = Instant::now();
        if let Some(lyrics) = lyrics_update {
            writer.lyrics = lyrics;
        }
    }

    if song_changed {
        last_song_id.clone_from(&new_song_id);
        last_song_url.clone_from(&new_song_url);
    } else if reload_lyrics && new_song_url.is_some() {
        last_song_url.clone_from(&new_song_url);
    }
    notify_render(render_tx);
    song_changed
}

fn clear_player_state(
    state: &Arc<RwLock<AppState>>,
    last_song_id: &mut CompactString,
    last_song_url: &mut Option<CompactString>,
    render_tx: &SyncSender<()>,
) {
    let changed = {
        let mut writer = state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed =
            writer.song.is_some() || writer.lyrics.is_some() || writer.player_unique_name.is_some();
        writer.song = None;
        writer.lyrics = None;
        writer.player_unique_name = None;
        writer.last_update_time = Instant::now();
        changed
    };

    last_song_id.clear();
    last_song_url.take();
    if changed {
        notify_render(render_tx);
    }
}

fn update_song_position(
    state: &Arc<RwLock<AppState>>,
    position: f64,
    render_tx: &SyncSender<()>,
) -> bool {
    let updated = {
        let mut writer = state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(song) = writer.song.as_mut() {
            song.position = position;
            writer.last_update_time = Instant::now();
            true
        } else {
            false
        }
    };

    if updated {
        notify_render(render_tx);
    }
    updated
}
fn estimated_position(state: &Arc<RwLock<AppState>>) -> Option<f64> {
    let reader = state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    reader.song.as_ref().map(|song| {
        if song.status == SongStatus::Playing {
            song.position + reader.last_update_time.elapsed().as_secs_f64()
        } else {
            song.position
        }
    })
}

fn confirmed_song_is_ready(
    candidate: &SongInfo,
    observed: &SongInfo,
    old_position: Option<f64>,
    attempt: usize,
) -> bool {
    if observed.id != candidate.id {
        return false;
    }

    let position_still_looks_old =
        old_position.is_some_and(|old| (observed.position - old).abs() < 0.5);
    !position_still_looks_old || attempt + 1 == POSITION_RETRY_LIMIT
}

fn confirm_song_after_track_change(
    player: &mpris::Player,
    mut candidate: SongInfo,
    old_position: Option<f64>,
    shutdown: &AtomicBool,
) -> Result<SongInfo> {
    let mut delay = POSITION_CONFIRM_DELAY;
    let mut last_error = None;

    for attempt in 0..POSITION_RETRY_LIMIT {
        if shutdown.load(Ordering::Acquire) {
            bail!("Shutting down while confirming track position");
        }
        thread::sleep(delay);
        if shutdown.load(Ordering::Acquire) {
            bail!("Shutting down while confirming track position");
        }

        match player::song_from_player(player) {
            Ok(observed) => {
                let ready = confirmed_song_is_ready(&candidate, &observed, old_position, attempt);
                candidate = observed;
                if ready {
                    return Ok(candidate);
                }
                last_error = None;
            }
            Err(error) => last_error = Some(error),
        }

        delay = delay.saturating_mul(2);
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Track identity did not stabilize")))
}

fn refresh_song_from_player(
    state: &Arc<RwLock<AppState>>,
    active_player: &mpris::Player,
    last_song_id: &mut CompactString,
    last_song_url: &mut Option<CompactString>,
    render_tx: &SyncSender<()>,
    shutdown: &AtomicBool,
) -> Result<()> {
    if shutdown.load(Ordering::Acquire) {
        bail!("Shutting down before refreshing player state");
    }

    let old_position = estimated_position(state);
    let mut song = player::song_from_player(active_player)?;
    if !last_song_id.is_empty() && song.id != *last_song_id {
        song = confirm_song_after_track_change(active_player, song, old_position, shutdown)?;
    }
    if shutdown.load(Ordering::Acquire) {
        bail!("Shutting down before committing player state");
    }

    set_song_state(
        state,
        last_song_id,
        last_song_url,
        song,
        active_player.unique_name(),
        render_tx,
    );
    Ok(())
}

fn watch_seeked_signals(
    state: &Arc<RwLock<AppState>>,
    render_tx: &SyncSender<()>,
    shutdown: &AtomicBool,
) -> Result<()> {
    let connection = Connection::session().context("Could not connect to session D-Bus")?;
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .path("/org/mpris/MediaPlayer2")?
        .interface("org.mpris.MediaPlayer2.Player")?
        .member("Seeked")?
        .build();
    let mut iterator = MessageIterator::for_match_rule(rule, &connection, Some(16))
        .context("Could not subscribe to MPRIS Seeked")?;

    for message in &mut iterator {
        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let message = message?;
        let Some(sender) = message.header().sender().map(|sender| sender.to_string()) else {
            continue;
        };
        let position_in_us: i64 = message
            .body()
            .deserialize()
            .context("Could not read MPRIS Seeked position")?;
        if position_in_us < 0 {
            continue;
        }

        if shutdown.load(Ordering::Acquire) {
            break;
        }
        let position = Duration::from_micros(position_in_us as u64).as_secs_f64();
        let updated = {
            let mut writer = state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if writer.player_unique_name.as_deref() == Some(sender.as_str())
                && let Some(song) = writer.song.as_mut()
            {
                song.position = position;
                writer.last_update_time = Instant::now();
                true
            } else {
                false
            }
        };

        if updated {
            notify_render(render_tx);
        }
    }

    Ok(())
}

fn watch_player(
    state: &Arc<RwLock<AppState>>,
    active_player: &mpris::Player,
    last_song_id: &mut CompactString,
    last_song_url: &mut Option<CompactString>,
    render_tx: &SyncSender<()>,
    shutdown: &AtomicBool,
) -> Result<()> {
    refresh_song_from_player(
        state,
        active_player,
        last_song_id,
        last_song_url,
        render_tx,
        shutdown,
    )?;

    let events = active_player
        .events()
        .context("Could not start MPRIS event stream")?;

    for event in events {
        if shutdown.load(Ordering::Acquire) {
            return Ok(());
        }
        match event? {
            mpris::Event::PlayerShutDown => break,
            mpris::Event::Playing
            | mpris::Event::Paused
            | mpris::Event::Stopped
            | mpris::Event::TrackChanged(_)
            | mpris::Event::TrackMetadataChanged { .. } => {
                refresh_song_from_player(
                    state,
                    active_player,
                    last_song_id,
                    last_song_url,
                    render_tx,
                    shutdown,
                )?;
            }
            // mpris 的 Seeked 路径与独立 zbus 监听均按兼容性要求保留。
            mpris::Event::Seeked { position_in_us } => {
                let position = Duration::from_micros(position_in_us).as_secs_f64();
                if !update_song_position(state, position, render_tx) {
                    refresh_song_from_player(
                        state,
                        active_player,
                        last_song_id,
                        last_song_url,
                        render_tx,
                        shutdown,
                    )?;
                }
            }
            mpris::Event::LoopingChanged(_)
            | mpris::Event::ShuffleToggled(_)
            | mpris::Event::VolumeChanged(_)
            | mpris::Event::PlaybackRateChanged(_)
            | mpris::Event::TrackAdded(_)
            | mpris::Event::TrackRemoved(_)
            | mpris::Event::TrackListReplaced => {}
        }
    }

    clear_player_state(state, last_song_id, last_song_url, render_tx);
    Ok(())
}

fn main() -> Result<()> {
    let state = Arc::new(RwLock::new(AppState {
        song: None,
        lyrics: None,
        last_update_time: Instant::now(),
        player_unique_name: None,
    }));
    let hidden = Arc::new(AtomicBool::new(false));
    let shutdown = Arc::new(AtomicBool::new(false));
    let (render_tx, render_rx) = mpsc::sync_channel(1);

    let mut signals = Signals::new([SIGUSR1]).context("Failed to register signal handler")?;
    let signal_handle = signals.handle();
    let hidden_clone = hidden.clone();
    let render_tx_signal = render_tx.clone();
    let shutdown_signal = shutdown.clone();
    let signal_thread = thread::spawn(move || {
        for _signal in signals.forever() {
            if shutdown_signal.load(Ordering::Acquire) {
                break;
            }
            let current = hidden_clone.load(Ordering::Relaxed);
            hidden_clone.store(!current, Ordering::Relaxed);
            eprintln!("waybar-bard: Toggled hidden state to {}", !current);
            if !notify_render(&render_tx_signal) {
                break;
            }
        }
    });

    let state_seek = state.clone();
    let render_tx_seek = render_tx.clone();
    let shutdown_seek = shutdown.clone();
    thread::spawn(move || {
        while !shutdown_seek.load(Ordering::Acquire) {
            if let Err(error) = watch_seeked_signals(&state_seek, &render_tx_seek, &shutdown_seek) {
                if shutdown_seek.load(Ordering::Acquire) {
                    break;
                }
                eprintln!("Error watching MPRIS Seeked: {error}");
                thread::sleep(Duration::from_secs(2));
            }
        }
    });

    let state_updater = state.clone();
    let render_tx_updater = render_tx.clone();
    let shutdown_updater = shutdown.clone();
    thread::spawn(move || {
        let mut last_song_id = CompactString::new("");
        let mut last_song_url = None;

        while !shutdown_updater.load(Ordering::Acquire) {
            match player::find_active_player() {
                Ok(Some(active_player)) => {
                    if let Err(error) = watch_player(
                        &state_updater,
                        &active_player,
                        &mut last_song_id,
                        &mut last_song_url,
                        &render_tx_updater,
                        &shutdown_updater,
                    ) {
                        if shutdown_updater.load(Ordering::Acquire) {
                            break;
                        }
                        // 保留最近一次有效状态；Position/元数据读取失败不写入伪造值。
                        eprintln!("Error watching MPRIS player: {error}");
                        thread::sleep(Duration::from_millis(500));
                    }
                }
                Ok(None) => {
                    clear_player_state(
                        &state_updater,
                        &mut last_song_id,
                        &mut last_song_url,
                        &render_tx_updater,
                    );
                    if let Err(error) = player::wait_for_mpris_player() {
                        if shutdown_updater.load(Ordering::Acquire) {
                            break;
                        }
                        eprintln!("Error waiting for MPRIS player: {error}");
                        thread::sleep(Duration::from_secs(2));
                    }
                }
                Err(error) => {
                    if shutdown_updater.load(Ordering::Acquire) {
                        break;
                    }
                    eprintln!("Error finding active MPRIS player: {error}");
                    thread::sleep(Duration::from_secs(2));
                }
            }
        }
    });

    drop(render_tx);

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    let mut last_rendered_frame = None;
    let run_result = loop {
        let (frame, timeout) = render_current_state(&state, &hidden);
        if let Err(error) = render_if_changed(&mut output, &mut last_rendered_frame, frame) {
            break Err(error);
        }
        if !wait_for_render_change(&render_rx, timeout) {
            break Ok(());
        }
    };

    shutdown.store(true, Ordering::Release);
    signal_handle.close();
    let _ = signal_thread.join();
    run_result
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc, RwLock,
        atomic::AtomicBool,
        mpsc::{TryRecvError, sync_channel},
    };
    use std::time::Instant;

    use compact_str::ToCompactString;
    use shared::models::{SongInfo, SongStatus};

    use crate::waybar::RenderedFrame;

    use super::{
        AppState, POSITION_RETRY_LIMIT, confirmed_song_is_ready, estimated_position, notify_render,
        render_current_state, should_reload_lyrics,
    };

    #[test]
    fn render_notifications_are_coalesced_to_one_slot() {
        let (tx, rx) = sync_channel(1);

        for _ in 0..1000 {
            assert!(notify_render(&tx));
        }
        assert_eq!(rx.recv().unwrap(), ());
        assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
        assert!(notify_render(&tx));
        assert_eq!(rx.recv().unwrap(), ());
    }

    #[test]
    fn disconnected_render_notification_does_not_panic() {
        let (tx, rx) = sync_channel(1);
        drop(rx);

        assert!(!notify_render(&tx));
    }

    #[test]
    fn lyrics_reload_when_url_becomes_available_or_changes() {
        assert!(!should_reload_lyrics(false, None, None));
        assert!(should_reload_lyrics(false, None, Some("file:///a.flac")));
        assert!(!should_reload_lyrics(
            false,
            Some("file:///a.flac"),
            Some("file:///a.flac")
        ));
        assert!(should_reload_lyrics(
            false,
            Some("file:///a.flac"),
            Some("file:///b.flac")
        ));
        assert!(should_reload_lyrics(true, None, None));
    }

    fn song(id: &str, position: f64, status: SongStatus) -> SongInfo {
        SongInfo {
            id: id.to_compact_string(),
            artist: "artist".to_compact_string(),
            title: "title".to_compact_string(),
            position,
            status,
            url: None,
        }
    }

    #[test]
    fn track_confirmation_requires_stable_identity_and_fresh_position() {
        let candidate = song("B", 87.0, SongStatus::Playing);
        let different_track = song("C", 0.2, SongStatus::Playing);
        assert!(!confirmed_song_is_ready(
            &candidate,
            &different_track,
            Some(87.0),
            0
        ));

        let fresh_position = song("B", 0.2, SongStatus::Playing);
        assert!(confirmed_song_is_ready(
            &candidate,
            &fresh_position,
            Some(87.0),
            0
        ));

        let position_not_yet_updated = song("B", 87.1, SongStatus::Playing);
        assert!(!confirmed_song_is_ready(
            &candidate,
            &position_not_yet_updated,
            Some(87.0),
            0
        ));
        assert!(confirmed_song_is_ready(
            &candidate,
            &position_not_yet_updated,
            Some(87.0),
            POSITION_RETRY_LIMIT - 1
        ));
    }

    #[test]
    fn stopped_song_does_not_advance_or_render() {
        let state = Arc::new(RwLock::new(AppState {
            song: Some(song("A", 10.0, SongStatus::Stopped)),
            lyrics: None,
            last_update_time: Instant::now(),
            player_unique_name: None,
        }));

        assert_eq!(estimated_position(&state), Some(10.0));
        let (frame, timeout) = render_current_state(&state, &AtomicBool::new(false));
        assert_eq!(frame, RenderedFrame::NoSong);
        assert!(timeout.is_none());
    }
}
