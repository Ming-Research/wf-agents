//! The clock the loop reads, the one thing of the process that reads one (principle `outside-behind-interfaces`).

use std::time::{SystemTime, UNIX_EPOCH};

use work::Time;

/// A clock: the time now, in milliseconds since the start of 1970.
pub trait Clock: Send + Sync {
    /// The time now, or why it cannot be read.
    fn now(&self) -> Result<Time, String>;
}

/// The system's clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Result<Time, String> {
        let since = (SystemTime::now().duration_since(UNIX_EPOCH))
            .map_err(|_| "the system's clock is set before the start of 1970".to_string())?;
        u64::try_from(since.as_millis())
            .map(Time)
            .map_err(|_| "the system's clock is set beyond what a time can hold".to_string())
    }
}

/// A clock that counts from a time it is given by the runtime's own clock: with the runtime's time paused, as a test
/// pauses it, it reads the times the test makes pass, and the times a work keeps are known to the test.
#[derive(Clone, Copy, Debug)]
pub struct TokioClock {
    from: Time,
    start: tokio::time::Instant,
}

impl TokioClock {
    /// A clock that reads `from` now.
    pub fn new(from: Time) -> Self {
        TokioClock {
            from,
            start: tokio::time::Instant::now(),
        }
    }
}

impl Clock for TokioClock {
    fn now(&self) -> Result<Time, String> {
        let passed = u64::try_from(self.start.elapsed().as_millis()).ok();
        (passed.and_then(|passed| self.from.0.checked_add(passed)))
            .map(Time)
            .ok_or_else(|| "more time has passed than a time can hold".to_string())
    }
}
