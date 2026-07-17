use std::io::{self, BufWriter};
use std::sync::mpsc;

use anyhow::Result;

use crate::app::{
    Coordinator, spawn_candidate_watchers, spawn_player_manager, spawn_seeked_watcher,
    spawn_signal_watcher,
};

mod app;
mod models;
mod waybar;

fn main() -> Result<()> {
    let (event_tx, event_rx) = mpsc::channel();
    let (rescan_tx, rescan_rx) = mpsc::sync_channel(1);

    spawn_signal_watcher(event_tx.clone())?;
    spawn_seeked_watcher(event_tx.clone());
    spawn_candidate_watchers(rescan_tx);
    spawn_player_manager(event_tx, rescan_rx);

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    Coordinator::new().run(event_rx, &mut output)
}
