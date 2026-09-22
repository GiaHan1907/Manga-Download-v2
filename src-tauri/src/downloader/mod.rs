//! Phase 06 download engine.
//!
//! Rust owns the entire lifecycle: page manifest resolution (through the
//! registered adapter), bounded streaming transfers, per-page checkpoints,
//! verified export and recovery. The renderer only submits chapter ids and
//! lifecycle actions through the narrow IPC surface.

pub mod checkpoint;
pub mod exporter;
pub mod image_validate;
pub mod progress;
pub mod recovery;
pub mod scheduler;
pub mod storage;
pub mod transfer;
pub mod worker;

#[cfg(test)]
mod engine_tests;

use std::time::Duration;

/// Automatic retry budget per task before it is surfaced as `failed`.
pub const MAX_ATTEMPTS: i64 = 3;
/// Hard cap of one backoff sleep (tests shrink via explicit next_retry_at).
pub const BACKOFF_CAP: Duration = Duration::from_secs(15 * 60);
/// Total downloaded bytes allowed per chapter (sum of staged pages).
pub const CHAPTER_MAX_BYTES: u64 = 400 * 1024 * 1024;
/// Upper bound on simultaneously tracked worker handles (queue metadata cap).
pub const MAX_TRACKED_TASKS: usize = 512;

/// Exponential backoff with deterministic jitter (no rand dependency):
/// attempt 1 ≈ 5s, 2 ≈ 20s, 3 ≈ 80s, capped. The task id contributes a
/// small stable offset so a batch does not retry in lockstep.
pub fn backoff_duration(attempt: i64, task_id: &str) -> Duration {
    let attempt = attempt.clamp(1, 6) as u32;
    let base = 5u64.saturating_mul(1u64 << (2 * (attempt - 1).min(5)));
    let jitter = {
        let mut hash: u64 = 0;
        for byte in task_id.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
        hash % 3
    };
    std::cmp::min(Duration::from_secs(base + jitter), BACKOFF_CAP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_and_stays_bounded() {
        let a1 = backoff_duration(1, "t1");
        let a2 = backoff_duration(2, "t1");
        assert!(a2 > a1);
        assert!(backoff_duration(10, "t1") <= BACKOFF_CAP);
        // Deterministic: same inputs, same delay (no Math.random rule).
        assert_eq!(backoff_duration(2, "abc"), backoff_duration(2, "abc"));
    }
}
