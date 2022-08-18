use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::time::{Duration, sleep};
use tokio::time::Instant;
use  tokio::time::Sleep;
use futures::FutureExt;
use futures::future::BoxFuture;


pub struct ChessClock {
    fast: Duration,
    timer: Duration,
    started: Option<Instant>
}

impl ChessClock {
    pub fn new(fast: Duration, timer: Duration) -> ChessClock {
        ChessClock {
            fast, timer, started: None
        }
    }

    //pub fn start(&mut self) {

    //}

    pub fn stop(&mut self) {
        if let Some(s) = self.started.take() {
            let e = s.elapsed().saturating_sub(self.fast);
            self.timer = self.timer.saturating_sub(e);
        }
    }

    pub fn start(&mut self) -> Sleep {
        self.started = Some(Instant::now());
        sleep(self.timer + self.fast)
    }
}

impl Default for ChessClock {
    fn default() -> Self {
        ChessClock::new(Duration::from_secs(5), Duration::from_secs(60))
    }
}