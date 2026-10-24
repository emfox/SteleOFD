//! How often one caller may ask for work.
//! A flooding caller is slowed rather than cut off: a token bucket per caller,
//! refilled steadily, with a burst so a page of small requests goes at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// The rate one caller is held to, and how much it may burst.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateLimit {
    /// Tokens added per second.
    pub rate: f64,
    /// Most tokens a caller may hold, so an idle caller is not punished for
    /// the requests it did not make.
    pub burst: f64,
}

impl RateLimit {
    /// A limit that adds `rate` tokens a second and lets `burst` accumulate.
    pub fn new(rate: f64, burst: f64) -> Self {
        Self { rate, burst }
    }

    /// A limit generous enough that a person driving the service never meets
    /// it, which is the default so a trial run needs no tuning.
    pub fn generous() -> Self {
        Self::new(20.0, 40.0)
    }

    /// Whether the budget is large enough to be a limit at all. A rate or a
    /// burst of zero would refuse every request, which is a misconfiguration
    /// rather than a policy.
    pub fn is_usable(&self) -> bool {
        self.rate.is_finite() && self.rate > 0.0 && self.burst.is_finite() && self.burst >= 1.0
    }
}

/// Who a bucket belongs to: the API key if one is presented, the peer address
/// otherwise, so callers are separated even when the service is open.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Caller(pub String);

impl Caller {
    /// The caller a request belongs to. The key settles it when there is one,
    /// because that is what names a tenant; otherwise the peer address does,
    /// which is the best a service without keys can do.
    pub fn of<B>(request: &axum::http::Request<B>) -> Self {
        let headers = request.headers();
        for name in [
            crate::tenancy::API_KEY_HEADER,
            crate::tenancy::AUTHORIZATION_HEADER,
        ] {
            if let Some(value) = headers.get(name).and_then(|v| v.to_str().ok()) {
                let value = value.trim();
                let token = value
                    .strip_prefix("Bearer ")
                    .or_else(|| value.strip_prefix("bearer "))
                    .unwrap_or(value)
                    .trim();
                if !token.is_empty() {
                    return Self(format!("key:{token}"));
                }
            }
        }
        // No key: fall back to the peer. A request built in-process has no
        // peer, and every such caller shares one bucket, which is what a
        // single-client deployment wants anyway.
        let peer = request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.to_string())
            .unwrap_or_else(|| "local".to_owned());
        Self(format!("peer:{peer}"))
    }
}

/// One bucket per caller, sharing a clock.
#[derive(Clone)]
pub struct Limiter {
    buckets: Arc<Mutex<HashMap<Caller, Bucket>>>,
    limit: RateLimit,
    /// Counts calls so a trim runs occasionally rather than on every request.
    calls: Arc<std::sync::atomic::AtomicU64>,
}

/// How many tokens a caller holds and when they were last topped up.
#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    checked: Instant,
}

/// The decision the limiter reaches for one request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    /// The request may proceed.
    Allow,
    /// The request is over budget; the caller should wait this long.
    RetryAfter(Duration),
}

impl Limiter {
    /// A limiter holding every caller to `limit`.
    pub fn new(limit: RateLimit) -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
            limit,
            calls: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    /// The limit every caller is held to.
    pub fn limit(&self) -> RateLimit {
        self.limit
    }

    /// Whether `caller` may make a request now, spending a token if so. The
    /// clock is a parameter so the rule can be tested without waiting.
    pub fn check(&self, caller: &Caller, now: Instant) -> Verdict {
        // A caller map that only ever grows would leak, so a trim runs every
        // so often. The counter is per-limiter and shared across threads.
        if self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .is_multiple_of(1024)
        {
            self.trim(now);
        }
        let mut buckets = self.lock();
        let bucket = buckets.entry(caller.clone()).or_insert(Bucket {
            tokens: self.limit.burst,
            checked: now,
        });
        let elapsed = now.saturating_duration_since(bucket.checked).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.limit.rate).min(self.limit.burst);
        bucket.checked = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            return Verdict::Allow;
        }
        // How long until the next whole token, so the wait is the real one.
        let missing = 1.0 - bucket.tokens;
        let seconds = missing / self.limit.rate;
        Verdict::RetryAfter(Duration::from_secs_f64(seconds.max(0.0)))
    }

    /// How many callers are being tracked. A caller that stops asking is kept
    /// until the map is trimmed, so a long run does not grow without bound.
    pub fn tracked(&self) -> usize {
        self.lock().len()
    }

    /// Drop buckets that have had long enough to refill completely, so an idle
    /// caller costs nothing to remember. A bucket younger than that keeps its
    /// state, because a flood is exactly the case where the state matters.
    pub fn trim(&self, now: Instant) {
        let refill = Duration::from_secs_f64(self.limit.burst / self.limit.rate);
        let mut buckets = self.lock();
        buckets.retain(|_, bucket| now.saturating_duration_since(bucket.checked) < refill);
    }

    /// The poisoned-lock fallback, as elsewhere: a panic leaves the map usable.
    fn lock(&self) -> MutexGuard<'_, HashMap<Caller, Bucket>> {
        self.buckets.lock().unwrap_or_else(|err| err.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A limiter with a generous burst and a known rate.
    fn limiter() -> Limiter {
        Limiter::new(RateLimit::new(1.0, 2.0))
    }

    #[test]
    fn a_generous_limit_is_usable_and_a_zero_one_is_not() {
        assert!(RateLimit::generous().is_usable());
        assert!(
            !RateLimit::new(0.0, 1.0).is_usable(),
            "no rate is no service"
        );
        assert!(
            !RateLimit::new(1.0, 0.0).is_usable(),
            "no burst refuses all"
        );
    }

    #[test]
    fn the_first_requests_spend_the_burst() {
        let limiter = limiter();
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        assert_eq!(limiter.check(&caller, start), Verdict::Allow);
        assert_eq!(limiter.check(&caller, start), Verdict::Allow);
        assert!(
            matches!(limiter.check(&caller, start), Verdict::RetryAfter(_)),
            "the third request has no token left"
        );
    }

    #[test]
    fn a_token_comes_back_at_the_stated_rate() {
        let limiter = limiter();
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        limiter.check(&caller, start);
        limiter.check(&caller, start);
        let after = start + Duration::from_secs(1);
        assert_eq!(
            limiter.check(&caller, after),
            Verdict::Allow,
            "one second returns one token at a rate of one"
        );
    }

    #[test]
    fn the_burst_is_the_ceiling_however_long_a_caller_waits() {
        let limiter = limiter();
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        limiter.check(&caller, start);
        // A very long idle period does not bank more than the burst.
        let later = start + Duration::from_secs(10_000);
        assert_eq!(limiter.check(&caller, later), Verdict::Allow);
        assert_eq!(limiter.check(&caller, later), Verdict::Allow);
        assert!(
            matches!(limiter.check(&caller, later), Verdict::RetryAfter(_)),
            "still only two tokens in hand"
        );
    }

    #[test]
    fn each_caller_has_its_own_bucket() {
        let limiter = limiter();
        let first = Caller("a".to_owned());
        let second = Caller("b".to_owned());
        let start = Instant::now();
        limiter.check(&first, start);
        limiter.check(&first, start);
        assert!(matches!(
            limiter.check(&first, start),
            Verdict::RetryAfter(_)
        ));
        assert_eq!(
            limiter.check(&second, start),
            Verdict::Allow,
            "one caller's flood does not slow another"
        );
    }

    #[test]
    fn the_wait_reported_is_the_time_to_the_next_token() {
        let limiter = Limiter::new(RateLimit::new(2.0, 1.0));
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        limiter.check(&caller, start);
        match limiter.check(&caller, start) {
            Verdict::RetryAfter(wait) => {
                // At two tokens a second, half a second buys one request.
                assert!(
                    (wait.as_secs_f64() - 0.5).abs() < 1e-6,
                    "expected about half a second, got {wait:?}"
                );
            }
            Verdict::Allow => panic!("the second request should be over budget"),
        }
    }

    #[test]
    fn trimming_forgets_a_caller_that_has_had_time_to_refill() {
        let limiter = limiter();
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        limiter.check(&caller, start);
        assert_eq!(limiter.tracked(), 1);

        // The bucket is full again after burst/rate seconds, so a trim then
        // may drop it: the next request refills it from the burst anyway.
        let full = start + Duration::from_secs(3);
        limiter.trim(full);
        assert_eq!(limiter.tracked(), 0, "a refilled bucket is dropped");
    }

    #[test]
    fn trimming_keeps_a_caller_that_is_still_spending() {
        let limiter = limiter();
        let caller = Caller("a".to_owned());
        let start = Instant::now();
        limiter.check(&caller, start);
        // Well before the bucket can refill, the state is what keeps a flood
        // out, so a trim must not forget it.
        limiter.trim(start + Duration::from_millis(10));
        assert_eq!(limiter.tracked(), 1, "a fresh bucket is kept");
    }
}
