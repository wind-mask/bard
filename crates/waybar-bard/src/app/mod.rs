mod coordinator;
mod event;
mod watchers;

pub use coordinator::Bard;
pub use watchers::{
    spawn_candidate_watchers, spawn_player_manager, spawn_seeked_watcher, spawn_signal_watcher,
};
