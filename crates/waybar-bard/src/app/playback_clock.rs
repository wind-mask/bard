use std::time::{Duration, Instant};

use shared::models::SongStatus;

#[derive(Clone, Debug)]
pub struct PlaybackClock {
    position_at_sync: Duration,
    synced_at: Instant,
    status: SongStatus,
}

impl PlaybackClock {
    pub fn new(position: Duration, status: SongStatus, synced_at: Instant) -> Self {
        Self {
            position_at_sync: position,
            synced_at,
            status,
        }
    }

    pub fn position_at(&self, now: Instant) -> Duration {
        if self.status == SongStatus::Playing {
            self.position_at_sync
                .saturating_add(now.saturating_duration_since(self.synced_at))
        } else {
            self.position_at_sync
        }
    }

    pub fn resync(&mut self, position: Duration, status: SongStatus, now: Instant) {
        self.position_at_sync = position;
        self.synced_at = now;
        self.status = status;
    }

    pub fn seek(&mut self, position: Duration, now: Instant) {
        self.position_at_sync = position;
        self.synced_at = now;
    }

    pub fn status(&self) -> SongStatus {
        self.status
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use shared::models::SongStatus;

    use super::PlaybackClock;

    #[test]
    fn playing_advances_but_paused_freezes() {
        let start = Instant::now();
        let mut clock = PlaybackClock::new(Duration::from_secs(10), SongStatus::Playing, start);
        assert_eq!(
            clock.position_at(start + Duration::from_secs(2)),
            Duration::from_secs(12)
        );

        clock.resync(
            Duration::from_secs(12),
            SongStatus::Paused,
            start + Duration::from_secs(2),
        );
        assert_eq!(
            clock.position_at(start + Duration::from_secs(20)),
            Duration::from_secs(12)
        );
    }

    #[test]
    fn resume_and_seek_reanchor_the_clock() {
        let start = Instant::now();
        let mut clock = PlaybackClock::new(Duration::from_secs(5), SongStatus::Paused, start);
        clock.resync(
            Duration::from_secs(5),
            SongStatus::Playing,
            start + Duration::from_secs(1),
        );
        assert_eq!(
            clock.position_at(start + Duration::from_secs(3)),
            Duration::from_secs(7)
        );

        clock.seek(Duration::from_secs(30), start + Duration::from_secs(3));
        assert_eq!(
            clock.position_at(start + Duration::from_secs(4)),
            Duration::from_secs(31)
        );
    }
}
