//! Hybrid Logical Clock packed into a `u64`: 48 bits of wall-clock milliseconds and a
//! 16-bit logical counter. Total order across devices is `(Hlc, device_id)` (ADR-0005).

use std::fmt;

const COUNTER_BITS: u32 = 16;
const COUNTER_MASK: u64 = (1 << COUNTER_BITS) - 1;
/// Remote clocks further ahead than this are rejected (24 h).
pub const MAX_DRIFT_MS: u64 = 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct Hlc(u64);

impl Hlc {
    pub fn new(millis: u64, counter: u16) -> Self {
        Self((millis << COUNTER_BITS) | u64::from(counter))
    }
    pub const fn from_u64(raw: u64) -> Self {
        Self(raw)
    }
    pub const fn as_u64(self) -> u64 {
        self.0
    }
    pub const fn millis(self) -> u64 {
        self.0 >> COUNTER_BITS
    }
    pub const fn counter(self) -> u16 {
        (self.0 & COUNTER_MASK) as u16
    }
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.millis(), self.counter())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HlcError {
    CounterOverflow,
    RemoteTooFarAhead { remote_ms: u64, local_ms: u64 },
}

impl fmt::Display for HlcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CounterOverflow => write!(f, "HLC counter overflow"),
            Self::RemoteTooFarAhead {
                remote_ms,
                local_ms,
            } => write!(f, "remote clock {remote_ms} too far ahead of {local_ms}"),
        }
    }
}

impl std::error::Error for HlcError {}

/// Stateful clock. Wall time is passed in so the logic is deterministic and testable.
#[derive(Debug, Default)]
pub struct HlcClock {
    last: Hlc,
}

impl HlcClock {
    pub fn new(last: Hlc) -> Self {
        Self { last }
    }

    pub fn last(&self) -> Hlc {
        self.last
    }

    /// Timestamp for a local event (or a send).
    pub fn tick(&mut self, now_ms: u64) -> Result<Hlc, HlcError> {
        let next = if now_ms > self.last.millis() {
            Hlc::new(now_ms, 0)
        } else {
            let c = self
                .last
                .counter()
                .checked_add(1)
                .ok_or(HlcError::CounterOverflow)?;
            Hlc::new(self.last.millis(), c)
        };
        self.last = next;
        Ok(next)
    }

    /// Merges a remote timestamp on receive; the result is greater than both inputs.
    pub fn observe(&mut self, remote: Hlc, now_ms: u64) -> Result<Hlc, HlcError> {
        if remote.millis() > now_ms.saturating_add(MAX_DRIFT_MS) {
            return Err(HlcError::RemoteTooFarAhead {
                remote_ms: remote.millis(),
                local_ms: now_ms,
            });
        }
        let (l, r) = (self.last, remote);
        let max_ms = now_ms.max(l.millis()).max(r.millis());
        let counter = if max_ms == l.millis() && max_ms == r.millis() {
            l.counter().max(r.counter()).checked_add(1)
        } else if max_ms == l.millis() {
            l.counter().checked_add(1)
        } else if max_ms == r.millis() {
            r.counter().checked_add(1)
        } else {
            Some(0)
        }
        .ok_or(HlcError::CounterOverflow)?;
        self.last = Hlc::new(max_ms, counter);
        Ok(self.last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_and_unpacks() {
        let h = Hlc::new(1_732_000_000_000, 7);
        assert_eq!(h.millis(), 1_732_000_000_000);
        assert_eq!(h.counter(), 7);
        assert_eq!(Hlc::from_u64(h.as_u64()), h);
    }

    #[test]
    fn tick_is_monotonic_even_if_wall_clock_goes_back() {
        let mut c = HlcClock::default();
        let a = c.tick(1_000).unwrap();
        let b = c.tick(1_000).unwrap();
        let d = c.tick(900).unwrap(); // clock moved backwards
        let e = c.tick(2_000).unwrap();
        assert!(a < b && b < d && d < e);
        assert_eq!(d, Hlc::new(1_000, 2));
        assert_eq!(e, Hlc::new(2_000, 0));
    }

    #[test]
    fn observe_moves_past_remote() {
        let mut c = HlcClock::new(Hlc::new(1_000, 3));
        let remote = Hlc::new(5_000, 9);
        let merged = c.observe(remote, 2_000).unwrap();
        assert!(merged > remote);
        assert_eq!(merged, Hlc::new(5_000, 10));
        // Local wall clock ahead of everyone resets the counter.
        let merged = c.observe(Hlc::new(10, 0), 9_000).unwrap();
        assert_eq!(merged, Hlc::new(9_000, 0));
    }

    #[test]
    fn rejects_far_future_remote() {
        let mut c = HlcClock::default();
        let remote = Hlc::new(1_000 + MAX_DRIFT_MS + 1, 0);
        assert!(matches!(
            c.observe(remote, 1_000),
            Err(HlcError::RemoteTooFarAhead { .. })
        ));
    }

    #[test]
    fn counter_overflow_is_reported() {
        let mut c = HlcClock::new(Hlc::new(1_000, u16::MAX));
        assert_eq!(c.tick(1_000), Err(HlcError::CounterOverflow));
    }
}
