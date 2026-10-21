//! The runtime's clocks on every target: the monotonic [`Instant`] (timers,
//! `Stopwatch`, collector statistics) and the wall clock of `DateTime.now`. The
//! native targets use `std::time`; WebAssembly, whose `std` has no clock, asks
//! the host (`crate::host`, docs/WASM-HOST-ABI-V1.md).

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

/// Microseconds since the Unix epoch (UTC); negative before it.
#[cfg(not(target_arch = "wasm32"))]
pub fn wall_clock_micros() -> i64 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => d.as_micros() as i64,
        Err(e) => -(e.duration().as_micros() as i64),
    }
}

/// Microseconds since the Unix epoch (UTC); negative before it.
#[cfg(target_arch = "wasm32")]
pub fn wall_clock_micros() -> i64 {
    crate::host::realtime_micros().floor() as i64
}

/// An instant of the host's monotonic clock, with the `std::time::Instant`
/// operations the runtime uses.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(std::time::Duration);

#[cfg(target_arch = "wasm32")]
impl Instant {
    pub fn now() -> Instant {
        let micros = crate::host::monotonic_micros().max(0.0);
        Instant(std::time::Duration::from_nanos((micros * 1000.0) as u64))
    }

    pub fn elapsed(&self) -> std::time::Duration {
        Instant::now().saturating_duration_since(*self)
    }

    pub fn duration_since(&self, earlier: Instant) -> std::time::Duration {
        self.saturating_duration_since(earlier)
    }

    pub fn saturating_duration_since(&self, earlier: Instant) -> std::time::Duration {
        self.0.saturating_sub(earlier.0)
    }

    pub fn checked_duration_since(&self, earlier: Instant) -> Option<std::time::Duration> {
        self.0.checked_sub(earlier.0)
    }

    pub fn checked_add(&self, d: std::time::Duration) -> Option<Instant> {
        self.0.checked_add(d).map(Instant)
    }

    pub fn checked_sub(&self, d: std::time::Duration) -> Option<Instant> {
        self.0.checked_sub(d).map(Instant)
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::Add<std::time::Duration> for Instant {
    type Output = Instant;
    fn add(self, d: std::time::Duration) -> Instant {
        self.checked_add(d)
            .expect("overflow adding a duration to an instant")
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::AddAssign<std::time::Duration> for Instant {
    fn add_assign(&mut self, d: std::time::Duration) {
        *self = *self + d;
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::Sub<std::time::Duration> for Instant {
    type Output = Instant;
    fn sub(self, d: std::time::Duration) -> Instant {
        self.checked_sub(d)
            .expect("overflow subtracting a duration from an instant")
    }
}

#[cfg(target_arch = "wasm32")]
impl std::ops::Sub<Instant> for Instant {
    type Output = std::time::Duration;
    fn sub(self, earlier: Instant) -> std::time::Duration {
        self.saturating_duration_since(earlier)
    }
}
