use tokio::time::Instant;
use tokio::time::Sleep;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
pub struct ChessClock {
    fast: Duration,
    rest_of_time: Duration,
    started: Option<Instant>,
}

impl ChessClock {
    pub fn new(fast: Duration, rest_of_time: Duration) -> ChessClock {
        ChessClock {
            fast,
            rest_of_time,
            started: None,
        }
    }

    pub fn stop(&mut self) {
        if let Some(s) = self.started.take() {
            let e = s.elapsed().saturating_sub(self.fast);
            self.rest_of_time = self.rest_of_time.saturating_sub(e);
        }
    }

    pub fn start(&mut self) -> Sleep {
        self.started = Some(Instant::now());
        sleep(self.rest_of_time + self.fast)
    }

    pub fn timers(&self) -> (Duration, Duration) {
        (self.fast, self.rest_of_time)
    }
}

impl Default for ChessClock {
    fn default() -> Self {
        ChessClock::new(Duration::from_secs(1), Duration::from_secs(2))
    }
}
