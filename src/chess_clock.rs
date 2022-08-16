use tokio::time::Duration;
use tokio::time::Instant;

pub(crate) struct ChessClock {
    fast: Duration,
    timer: Duration,
    started: Option<Instant>
}

impl ChessClock {
    pub(crate) fn new(fast: Duration, timer: Duration) -> ChessClock {
        ChessClock {
            fast, timer, started: None
        }
    }

    pub(crate) fn start(&mut self) {
        self.started = Some(Instant::now());
    }

    pub(crate) fn stop(&mut self) {
        if let Some(s) = self.started.take() {
            let e = s.elapsed().saturating_sub(self.fast);
            self.timer = self.timer.saturating_sub(e);
        }
    }

    pub(crate) fn start_and_take_waiter(&mut self) -> tokio::time::Sleep {
        self.start();
        tokio::time::sleep(self.timer + self.fast)
    }
}

impl Default for ChessClock {
    fn default() -> Self {
        ChessClock::new(Duration::from_secs(5), Duration::from_secs(60))
    }
}