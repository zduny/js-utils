//! Sleeping.

use futures::{Future, FutureExt};
use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};
use zduny_wasm_timer::Delay;
pub use zduny_wasm_timer::Instant;

/// Waits until `duration` has elapsed.
///
/// An asynchronous analog to [`std::thread::sleep`].
#[must_use]
pub fn sleep(duration: Duration) -> Sleep {
    sleep_until(Instant::now() + duration)
}

/// Waits until `deadline` is reached.
#[must_use]
pub fn sleep_until(deadline: Instant) -> Sleep {
    Sleep {
        deadline,
        delay: Delay::new_at(deadline),
    }
}

/// Future returned by [`sleep`] and [`sleep_until`].
#[derive(Debug)]
pub struct Sleep {
    deadline: Instant,
    delay: Delay,
}

impl Sleep {
    /// Returns the instant at which the future will complete.
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns `true` if `Sleep` has elapsed.
    ///
    /// A `Sleep` instance is elapsed when the requested duration has elapsed.
    pub fn is_elapsed(&self) -> bool {
        Instant::now() > self.deadline
    }

    /// Resets the `Sleep` instance to a new deadline.
    ///
    /// Calling this function allows changing the instant at which the `Sleep`
    /// future completes without having to create new associated state.
    ///
    /// This function can be called both before and after the future has
    /// completed.
    pub fn reset(&mut self, deadline: Instant) {
        self.delay.reset_at(deadline);
    }
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.delay.poll_unpin(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{sleep, sleep::Instant};

    #[wasm_bindgen_test]
    async fn test_sleep() {
        let current = Instant::now();
        sleep(Duration::from_secs(1)).await;
        let difference = Instant::now() - current;
        assert!(difference.as_secs_f64() > 1.0)
    }
}
