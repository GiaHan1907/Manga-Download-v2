//! Bandwidth pacing for page downloads.
//!
//! Virtual scheduling rather than a token bucket: every caller reserves the
//! instant it may start, so parallel page fetches queue up behind one another
//! instead of all waking at once and bursting. The state is process-wide on
//! purpose — a per-chapter limiter would multiply the cap by the number of
//! chapters running concurrently.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct PaceState {
    /// End of the window already reserved by earlier callers.
    next_allowed: Instant,
}

impl PaceState {
    fn new(now: Instant) -> Self {
        PaceState { next_allowed: now }
    }
}

/// How long a caller carrying `bytes` must wait before sending, at `kbps`
/// thousand bytes per second (0 = unlimited). Pure, so the arithmetic is
/// testable without sleeping.
pub fn wait_for(state: &mut PaceState, kbps: u64, bytes: u64, now: Instant) -> Duration {
    if kbps == 0 || bytes == 0 {
        return Duration::ZERO;
    }
    let start = state.next_allowed.max(now);
    let window = Duration::from_secs_f64(bytes as f64 / (kbps as f64 * 1000.0));
    state.next_allowed = start + window;
    start.saturating_duration_since(now)
}

static PACE: OnceLock<Mutex<PaceState>> = OnceLock::new();

/// Sleep until this payload is allowed to go out. No-op with no limit set.
pub async fn pace(bytes: u64, kbps: u64) {
    if kbps == 0 || bytes == 0 {
        return;
    }
    let wait = {
        let mut guard = PACE
            .get_or_init(|| Mutex::new(PaceState::new(Instant::now())))
            .lock()
            .unwrap();
        wait_for(&mut guard, kbps, bytes, Instant::now())
    };
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlimited_or_empty_payloads_never_wait() {
        let now = Instant::now();
        let mut state = PaceState::new(now);
        assert_eq!(wait_for(&mut state, 0, 1024 * 1024, now), Duration::ZERO);
        assert_eq!(wait_for(&mut state, 500, 0, now), Duration::ZERO);
    }

    #[test]
    fn the_wait_is_the_payload_share_of_the_rate() {
        let now = Instant::now();
        let mut state = PaceState::new(now);
        // 500 KB at 1000 KB/s = 0.5 s.
        assert_eq!(wait_for(&mut state, 1000, 500_000, now), Duration::ZERO);
        // The next payload waits for the window the first one reserved.
        assert_eq!(
            wait_for(&mut state, 1000, 500_000, now),
            Duration::from_millis(500)
        );
        // A caller arriving after the reserved window waits nothing itself —
        // and still reserves the window its own payload will occupy.
        let later = now + Duration::from_millis(1_200);
        assert_eq!(wait_for(&mut state, 1000, 500_000, later), Duration::ZERO);
        assert_eq!(
            wait_for(&mut state, 1000, 500_000, later),
            Duration::from_millis(500)
        );
    }

    #[test]
    fn pacing_never_speeds_up_to_recover_an_idle_period() {
        let now = Instant::now();
        let mut state = PaceState::new(now);
        wait_for(&mut state, 100, 100_000, now); // reserves 1 s
        let idle = now + Duration::from_secs(10);
        assert_eq!(wait_for(&mut state, 100, 100_000, idle), Duration::ZERO);
        // The next one is scheduled from the idle moment, not from the past.
        assert_eq!(
            wait_for(&mut state, 100, 100_000, idle),
            Duration::from_millis(1000)
        );
    }
}
