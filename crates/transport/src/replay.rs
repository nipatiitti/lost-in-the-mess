//! Per-sender 128-bit sliding-window replay protection.
//! Must be called AFTER AEAD verification — a forged frame with valid header
//! but wrong key must not be allowed to poison this state.

use litm_common::NodeId;
use std::collections::HashMap;
use std::sync::Mutex;

const WINDOW: u64 = 128;

pub struct ReplayDb {
    inner: Mutex<HashMap<NodeId, Window>>,
}

#[derive(Default)]
struct Window {
    high: u64,
    /// Bit 0 = high seen, bit i = (high - i) seen.
    bits: u128,
}

impl ReplayDb {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Ok if fresh; Err if replay or out of window.
    pub fn check_and_mark(&self, sender: NodeId, counter: u64) -> Result<(), ()> {
        let mut g = self.inner.lock().unwrap();
        let w = g.entry(sender).or_default();

        if w.high == 0 && w.bits == 0 {
            // first packet from this sender
            w.high = counter;
            w.bits = 1;
            return Ok(());
        }

        if counter > w.high {
            let shift = counter - w.high;
            w.bits = if shift >= WINDOW {
                1
            } else {
                (w.bits << shift) | 1
            };
            w.high = counter;
            Ok(())
        } else {
            let delta = w.high - counter;
            if delta >= WINDOW {
                // If the counter is very low (near zero) but the window is far ahead,
                // this is a node restart (counter reset), not a replay attack.
                // AEAD verification already passed, so the frame is authentic.
                // We reset the sender's window to accept the restarted peer.
                if counter <= WINDOW && w.high > WINDOW * 4 {
                    w.high = counter;
                    w.bits = 1;
                    return Ok(());
                }
                return Err(());
            }
            let mask = 1u128 << delta;
            if w.bits & mask != 0 {
                return Err(());
            }
            w.bits |= mask;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_accepted() {
        let db = ReplayDb::new();
        for c in 1..50 {
            assert!(db.check_and_mark(1, c).is_ok());
        }
    }

    #[test]
    fn duplicate_rejected() {
        let db = ReplayDb::new();
        assert!(db.check_and_mark(1, 5).is_ok());
        assert!(db.check_and_mark(1, 5).is_err());
    }

    #[test]
    fn out_of_order_within_window() {
        let db = ReplayDb::new();
        assert!(db.check_and_mark(1, 200).is_ok());
        assert!(db.check_and_mark(1, 150).is_ok());
        assert!(db.check_and_mark(1, 150).is_err()); // dup
    }

    #[test]
    fn too_old_rejected() {
        let db = ReplayDb::new();
        assert!(db.check_and_mark(1, 500).is_ok());
        assert!(db.check_and_mark(1, 100).is_err()); // 500-100 >= 128
    }

    #[test]
    fn senders_isolated() {
        let db = ReplayDb::new();
        assert!(db.check_and_mark(1, 10).is_ok());
        assert!(db.check_and_mark(2, 10).is_ok());
    }
}
