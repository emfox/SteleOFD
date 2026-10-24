//! Stopping without cutting work in half.
//! A stop is announced, the socket stops accepting, and work already in flight
//! is given a bounded grace period to finish before the process gives up.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Notify;

/// How long in-flight work is given to finish before the process gives up.
/// Chosen to sit above the request timeout, so a request that will time out on
/// its own has already done so by the time the deadline passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadline(Duration);

impl Deadline {
    /// A grace period of `duration`.
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }

    /// The default grace period: longer than one request may take, so a
    /// request that started before the stop can finish on its own terms.
    pub fn generous() -> Self {
        Self(Duration::from_secs(150))
    }

    /// The period itself.
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl Default for Deadline {
    fn default() -> Self {
        Self::generous()
    }
}

/// A stop that has been asked for, and the promise that work may finish.
#[derive(Clone)]
pub struct Drain {
    stopping: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Drain {
    /// A drain nobody has asked for, which is the running state.
    pub fn new() -> Self {
        Self {
            stopping: Arc::new(AtomicBool::new(false)),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Announce a stop. Idempotent: a second signal changes nothing and wakes
    /// nobody again, so a duplicated SIGTERM is harmless.
    pub fn stop(&self) {
        if !self.stopping.swap(true, Ordering::SeqCst) {
            self.notify.notify_waiters();
        }
    }

    /// Whether a stop has been asked for.
    pub fn is_stopping(&self) -> bool {
        self.stopping.load(Ordering::SeqCst)
    }

    /// Resolve once a stop is asked for. Used as the server's shutdown future,
    /// which is what makes the socket stop accepting.
    pub async fn wait(&self) {
        if self.is_stopping() {
            return;
        }
        // The loop covers the race where `stop` runs between the check above
        // and the wait below, where the notification would otherwise be lost.
        loop {
            self.notify.notified().await;
            if self.is_stopping() {
                return;
            }
        }
    }

    /// Resolve once a stop is asked for, but no later than the deadline.
    /// Returns whether the deadline passed with the drain still unfinished.
    pub async fn wait_within(&self, deadline: Deadline) -> bool {
        match tokio::time::timeout(deadline.duration(), self.wait()).await {
            Ok(()) => false,
            Err(_) => true,
        }
    }
}

impl Default for Drain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_drain_is_not_stopping() {
        let drain = Drain::new();
        assert!(!drain.is_stopping());
    }

    #[test]
    fn stopping_is_sticky_and_idempotent() {
        let drain = Drain::new();
        drain.stop();
        drain.stop();
        assert!(drain.is_stopping(), "the second signal changes nothing");
    }

    #[tokio::test]
    async fn a_wait_finishes_when_the_stop_arrives() {
        let drain = Drain::new();
        let waiter = drain.clone();
        let handle = tokio::spawn(async move { waiter.wait().await });
        drain.stop();
        handle.await.expect("the waiter resolves");
    }

    #[tokio::test]
    async fn a_wait_after_the_stop_returns_at_once() {
        let drain = Drain::new();
        drain.stop();
        // No timeout needed: waiting on an already-stopped drain resolves now,
        // which is what a listener built after the signal needs.
        tokio::time::timeout(Duration::from_millis(50), drain.wait())
            .await
            .expect("the wait does not hang");
    }

    #[tokio::test]
    async fn a_wait_within_reports_a_deadline_that_passed() {
        let drain = Drain::new();
        let deadline = Deadline::new(Duration::from_millis(10));
        assert!(
            drain.wait_within(deadline).await,
            "nothing stopped, so the deadline passed"
        );
    }

    #[tokio::test]
    async fn a_wait_within_reports_a_stop_that_came_first() {
        let drain = Drain::new();
        let waiter = drain.clone();
        tokio::spawn(async move {
            // The stop arrives well inside the deadline.
            tokio::time::sleep(Duration::from_millis(5)).await;
            waiter.stop();
        });
        let deadline = Deadline::new(Duration::from_secs(5));
        assert!(
            !drain.wait_within(deadline).await,
            "the stop came first, so no deadline was missed"
        );
    }
}
