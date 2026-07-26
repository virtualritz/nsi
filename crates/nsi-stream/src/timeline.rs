//! Publication synchronization primitive.
//!
//! R4 requires exactly one timeline object per stream: each publication
//! carries the value the client must wait on before sampling, and the client
//! never waits on anything else. [`CpuTimeline`] is the CPU-transport
//! equivalent of a Vulkan timeline semaphore -- the "generation counter" the
//! contract calls for -- and
//! [`VulkanTimeline`](crate::transport::gpu::VulkanTimeline) exposes the same
//! `signal`/`wait` shape on the GPU transport.
//!
//! Values are monotonic: [`CpuTimeline::signal`] takes the maximum of the
//! current and the requested value, so an out-of-order signal can never move
//! the timeline backwards. Waiting is never a spin; it blocks on a condition
//! variable and returns [`Error::WaitTimeout`] rather than looping
//! (`publication-lifecycle.md`, failure modes).

use crate::{Error, Result};
use std::{
    sync::{
        Condvar, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// A monotonic, waitable counter.
#[derive(Debug, Default)]
pub struct CpuTimeline {
    /// Fast path for [`CpuTimeline::value`], kept in step with `guarded`.
    value: AtomicU64,
    guarded: Mutex<u64>,
    signaled: Condvar,
}

impl CpuTimeline {
    /// A timeline at value 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current value.
    #[inline]
    pub fn value(&self) -> u64 {
        self.value.load(Ordering::Acquire)
    }

    /// Raise the timeline to `value`.
    ///
    /// Lower or equal values are ignored -- the timeline never moves
    /// backwards. Every waiter is woken.
    pub fn signal(&self, value: u64) {
        let mut current = self.guarded.lock().expect("timeline mutex");

        if value > *current {
            *current = value;
            self.value.store(value, Ordering::Release);
            self.signaled.notify_all();
        }
    }

    /// Block until the timeline reaches `value`.
    ///
    /// `timeout` of `None` waits indefinitely.
    ///
    /// # Errors
    ///
    /// [`Error::WaitTimeout`] carrying `value` when the deadline expires
    /// first.
    pub fn wait(&self, value: u64, timeout: Option<Duration>) -> Result<()> {
        let deadline = timeout.map(|timeout| Instant::now() + timeout);
        let mut current = self.guarded.lock().expect("timeline mutex");

        while *current < value {
            match deadline {
                None => {
                    current =
                        self.signaled.wait(current).expect("timeline condvar");
                }
                Some(deadline) => {
                    let remaining = deadline
                        .checked_duration_since(Instant::now())
                        .unwrap_or_default();

                    if remaining.is_zero() {
                        Err(Error::WaitTimeout { serial: value })?;
                    }

                    let (guard, result) = self
                        .signaled
                        .wait_timeout(current, remaining)
                        .expect("timeline condvar");
                    current = guard;

                    if result.timed_out() && *current < value {
                        Err(Error::WaitTimeout { serial: value })?;
                    }
                }
            }
        }

        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread};

    #[test]
    fn signal_is_monotonic() {
        let timeline = CpuTimeline::new();

        timeline.signal(7);
        timeline.signal(3);

        assert_eq!(timeline.value(), 7);
    }

    #[test]
    fn wait_returns_timeout() {
        let timeline = CpuTimeline::new();

        assert_eq!(
            timeline.wait(1, Some(Duration::from_millis(10))),
            Err(Error::WaitTimeout { serial: 1 })
        );
    }

    #[test]
    fn wait_wakes_on_signal() {
        let timeline = Arc::new(CpuTimeline::new());
        let signaler = Arc::clone(&timeline);

        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(5));
            signaler.signal(4);
        });

        timeline
            .wait(4, Some(Duration::from_secs(5)))
            .expect("signaled before the deadline");
        handle.join().expect("signaler thread");
    }
}
