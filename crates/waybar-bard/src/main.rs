use std::io::{self, BufWriter};
use std::sync::mpsc;

use anyhow::Result;
use log::info;

use crate::app::{
    Bard, spawn_candidate_watchers, spawn_player_manager, spawn_seeked_watcher,
    spawn_signal_watcher,
};
use crate::cli::CliAction;

mod app;
mod cli;
mod models;
mod waybar;

fn main() -> Result<()> {
    env_logger::init();
    info!("Starting waybar-bard v{}", env!("CARGO_PKG_VERSION"));
    let offset_ms = match cli::parse()? {
        CliAction::Run(config) => config.offset_ms,
        CliAction::Help => {
            print!("{}", cli::help());
            return Ok(());
        }
        CliAction::Version => {
            println!("waybar-bard {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    };

    let (event_tx, event_rx) = mpsc::channel();
    let (rescan_tx, rescan_rx) = mpsc::sync_channel(1);

    spawn_signal_watcher(event_tx.clone())?;
    spawn_seeked_watcher(event_tx.clone());
    spawn_candidate_watchers(rescan_tx);
    spawn_player_manager(event_tx, rescan_rx);

    let stdout = io::stdout();
    let mut output = BufWriter::new(stdout.lock());
    Bard::new(offset_ms).run(event_rx, &mut output)
}
