//! Hybrid logical clock (sync-design.md §1.2).
//!
//! Every change in the sync log is stamped with an [`Hlc`]. Comparing two
//! stamps decides which edit of a row wins (last-writer-wins per row), so the
//! clock must only ever move forward, even when the Device's wall clock jumps
//! back or another Device's clock is ahead of ours.
//!
//! The string form, stored in `op.hlc` and every `updated_hlc` column, is
//! `"<wall_ms: 16 digits>-<counter: 4 digits>-<device_id>"`, zero padded so
//! that comparing strings gives the same order as comparing stamps (SQL
//! `MAX(hlc)` and `ORDER BY hlc` therefore work).

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

/// The counter is written with 4 digits. When more than this many stamps are
/// made in one millisecond, the clock borrows the next millisecond instead,
/// so the string form never needs a fifth digit.
const MAX_COUNTER: u32 = 9_999;

/// One HLC value: `(wall_ms, counter, device_id)`, ordered by those fields in
/// that order. The device id breaks ties, so two Devices never produce equal
/// stamps.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hlc {
    wall_ms: u64,
    counter: u32,
    device_id: String,
}

impl Hlc {
    pub fn wall_ms(&self) -> u64 {
        self.wall_ms
    }

    pub fn counter(&self) -> u32 {
        self.counter
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }
}

impl Ord for Hlc {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.wall_ms, self.counter, &self.device_id).cmp(&(
            other.wall_ms,
            other.counter,
            &other.device_id,
        ))
    }
}

impl PartialOrd for Hlc {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Hlc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:016}-{:04}-{}",
            self.wall_ms, self.counter, self.device_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("malformed HLC stamp: {0:?}")]
pub struct ParseHlcError(String);

impl FromStr for Hlc {
    type Err = ParseHlcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseHlcError(s.to_owned());
        let mut parts = s.splitn(3, '-');
        let wall = parts.next().ok_or_else(err)?;
        let counter = parts.next().ok_or_else(err)?;
        let device_id = parts.next().ok_or_else(err)?;
        let digits = |p: &str, len: usize| p.len() == len && p.bytes().all(|b| b.is_ascii_digit());
        if !digits(wall, 16) || !digits(counter, 4) || device_id.is_empty() {
            return Err(err());
        }
        Ok(Hlc {
            wall_ms: wall.parse().map_err(|_| err())?,
            counter: counter.parse().map_err(|_| err())?,
            device_id: device_id.to_owned(),
        })
    }
}

/// The clock of one Device. It remembers the latest stamp it produced or saw.
#[derive(Debug, Clone)]
pub struct Clock {
    device_id: String,
    last_wall_ms: u64,
    last_counter: u32,
}

impl Clock {
    pub fn new(device_id: &str) -> Self {
        Clock {
            device_id: device_id.to_owned(),
            last_wall_ms: 0,
            last_counter: 0,
        }
    }

    /// Take note of a stamp from another Device (a received change), or of
    /// the latest stamp on disk at startup, so that the next local stamp is
    /// later than it: `last = max(last, seen)`.
    pub fn observe(&mut self, seen: &Hlc) {
        if (seen.wall_ms, seen.counter) > (self.last_wall_ms, self.last_counter) {
            self.last_wall_ms = seen.wall_ms;
            self.last_counter = seen.counter;
        }
    }

    /// Stamp a local write. `now_ms` is the wall clock in Unix milliseconds.
    /// `wall = max(now, last.wall)`; the counter increments when the wall
    /// didn't move and resets to 0 when it did.
    pub fn tick(&mut self, now_ms: u64) -> Hlc {
        if now_ms > self.last_wall_ms {
            self.last_wall_ms = now_ms;
            self.last_counter = 0;
        } else if self.last_counter < MAX_COUNTER {
            self.last_counter += 1;
        } else {
            self.last_wall_ms += 1;
            self.last_counter = 0;
        }
        Hlc {
            wall_ms: self.last_wall_ms,
            counter: self.last_counter,
            device_id: self.device_id.clone(),
        }
    }
}
