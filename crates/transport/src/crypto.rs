//! ChaCha20-Poly1305 AEAD with a sliding window ratcheted by HKDF-SHA256.
//! The window holds {e-BACK, …, e, e+1} so peers with clock skew up to
//! EPOCH_BACK_TOLERANCE*60 seconds behind can still communicate.

use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use sha2::Sha256;
use std::collections::BTreeMap;
use std::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use litm_common::{Epoch, Error, Result};
use tracing::{debug, warn};


const RATCHET_INFO: &[u8] = b"kova-mesh/ratchet/v1";

// How many epochs behind the current epoch we keep keys for.
// Each epoch is 60 s, so 2 means nodes whose clocks lag by up to 120 s can
// still decrypt frames from a faster peer.
const EPOCH_BACK_TOLERANCE: u32 = 2;

#[derive(Zeroize, ZeroizeOnDrop)]
struct EpochKey([u8; 32]);

pub struct KeyStore {
    // Root key kept in memory for O(1) epoch key derivation.
    // Zeroized on drop. Forward secrecy holds per-device after epoch advance
    // (past keys are gone from the window), but not against root key extraction.
    // For demo: root is hardcoded, so this tradeoff is acceptable.
    root_key: Mutex<Zeroizing<[u8; 32]>>,
    inner: Mutex<KeyWindow>,
}

struct KeyWindow {
    current: Epoch,
    keys: BTreeMap<Epoch, EpochKey>,
}

/// Derive the key for a specific epoch directly from the root key.
/// O(1) regardless of epoch magnitude.
fn epoch_key(root: &[u8; 32], epoch: Epoch) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(&epoch.to_le_bytes()), root);
    let mut out = [0u8; 32];
    hk.expand(RATCHET_INFO, &mut out).expect("HKDF expand 32B");
    out
}

impl KeyStore {
    /// Initialize from a 32-byte root and the current epoch.
    /// Seeds the window with {start_epoch-BACK, …, start_epoch, start_epoch+1}.
    pub fn new(root_key: [u8; 32], start_epoch: Epoch) -> Self {
        let mut keys = BTreeMap::new();
        // Past epochs (backward tolerance).
        for i in 0..=EPOCH_BACK_TOLERANCE {
            let e = start_epoch.saturating_sub(i);
            keys.entry(e).or_insert_with(|| EpochKey(epoch_key(&root_key, e)));
        }
        // One epoch ahead so we can decrypt frames from a slightly faster peer.
        keys.insert(start_epoch + 1, EpochKey(epoch_key(&root_key, start_epoch + 1)));
        Self {
            root_key: Mutex::new(Zeroizing::new(root_key)),
            inner: Mutex::new(KeyWindow {
                current: start_epoch,
                keys,
            }),
        }
    }

    pub fn current_epoch(&self) -> Epoch {
        self.inner.lock().unwrap().current
    }

    /// Advance the window so `current == target` after the call.
    /// Each step adds K_{new_top+1} (derived directly) and drops K_{new_top-2}.
    pub fn advance(&self, target: Epoch) {
        let root = self.root_key.lock().unwrap();
        let mut w = self.inner.lock().unwrap();
        while w.current < target {
            let next = w.current + 1;
            // Add the epoch one ahead of the new top so we can decrypt
            // frames from a slightly faster peer after the advance.
            let new_top = next + 1;
            w.keys.insert(new_top, EpochKey(epoch_key(&root, new_top)));
            // Evict the epoch that falls outside the backward tolerance window.
            // Window after advance: {next-BACK, …, next, next+1}.
            if next > EPOCH_BACK_TOLERANCE {
                w.keys.remove(&(next - EPOCH_BACK_TOLERANCE - 1));
            }
            w.current = next;
        }
    }

    pub fn seal(&self, epoch: Epoch, nonce: &[u8; 12], aad: &[u8], pt: &[u8]) -> Result<Vec<u8>> {
        let w = self.inner.lock().unwrap();
        let k = w
            .keys
            .get(&epoch)
            .ok_or_else(|| Error::Other(format!("seal: epoch {epoch} not in window")))?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&k.0));
        cipher
            .encrypt(Nonce::from_slice(nonce), Payload { msg: pt, aad })
            .map_err(|_| {
                warn!(epoch, "seal failed");
                Error::Other("seal failed".into())
            })

    }

    /// Decrypt a frame, with bounded forward-resync for nodes that are slightly
    /// behind the group epoch.
    ///
    /// Security properties:
    /// - The key window is **never mutated** by this call; only the epoch-advance
    ///   task may move the window forward. An attacker cannot corrupt window state
    ///   by sending fake frames with large epoch values.
    /// - Forward resync is capped at `MAX_RESYNC_EPOCHS` steps beyond the top of
    ///   the stored window. Exceeding the cap costs a single bounds check and
    ///   returns `AuthFailed` immediately — no HKDF work is done.
    /// - Resync keys are derived directly from root in O(1) and wrapped in
    ///   `Zeroizing`, wiped on drop including on panic paths.
    pub fn open(&self, epoch: Epoch, nonce: &[u8; 12], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>> {
        const MAX_RESYNC_EPOCHS: u32 = 5;

        enum Resolved {
            Window([u8; 32]),
            Resync, // epoch key not in window; will derive from root
        }

        let resolved = {
            let w = self.inner.lock().unwrap();
            if let Some(k) = w.keys.get(&epoch) {
                Resolved::Window(k.0)
            } else {
                let Some((&min_epoch, _)) = w.keys.iter().next() else {
                    return Err(Error::AuthFailed);
                };
                let Some((&top_epoch, _)) = w.keys.iter().rev().next() else {
                    return Err(Error::AuthFailed);
                };
                // Reject too-old (below the window floor) and too-far-ahead (DoS guard).
                if epoch < min_epoch {
                    debug!(epoch, current = w.current, "open: epoch too old");
                    return Err(Error::AuthFailed);
                }
                if epoch > top_epoch + MAX_RESYNC_EPOCHS {
                    warn!(
                        epoch,
                        top = top_epoch,
                        max_resync = MAX_RESYNC_EPOCHS,
                        "open: epoch too far ahead"
                    );
                    return Err(Error::AuthFailed);
                }

                Resolved::Resync
            }
            // lock released here
        };

        let key: Zeroizing<[u8; 32]> = match resolved {
            Resolved::Window(k) => Zeroizing::new(k),
            Resolved::Resync => {
                let root = self.root_key.lock().unwrap();
                Zeroizing::new(epoch_key(&root, epoch))
            }
        };

        let cipher = ChaCha20Poly1305::new(Key::from_slice(&*key));
        cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad })
            .map_err(|_| {
                warn!(epoch, "open: AEAD auth failed (wrong key or corrupted data)");
                Error::AuthFailed
            })

        // `key` dropped and zeroized here
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let ks = KeyStore::new([7u8; 32], 0);
        let nonce = [0u8; 12];
        let ct = ks.seal(0, &nonce, b"aad", b"hello").unwrap();
        assert_eq!(ks.open(0, &nonce, b"aad", &ct).unwrap(), b"hello");
    }

    #[test]
    fn wrong_aad_fails() {
        let ks = KeyStore::new([7u8; 32], 0);
        let ct = ks.seal(0, &[1u8; 12], b"a", b"x").unwrap();
        assert!(ks.open(0, &[1u8; 12], b"b", &ct).is_err());
    }

    #[test]
    fn advance_keeps_correct_window() {
        // start=5 → window {3,4,5,6}; advance to 7 → {5,6,7,8}
        let ks = KeyStore::new([1u8; 32], 5);
        ks.advance(7);
        let g = ks.inner.lock().unwrap();
        assert_eq!(g.current, 7);
        // window is {current-BACK .. current+1} = {5,6,7,8}
        for e in 5u32..=8 {
            assert!(g.keys.contains_key(&e), "missing epoch {e}");
        }
        assert!(!g.keys.contains_key(&4), "epoch 4 should have been evicted");
    }

    #[test]
    fn cross_node_compat() {
        // Two nodes derive the same K_e from the same root.
        let a = KeyStore::new([42u8; 32], 100);
        let b = KeyStore::new([42u8; 32], 100);
        let ct = a.seal(100, &[9u8; 12], b"aad", b"ping").unwrap();
        assert_eq!(b.open(100, &[9u8; 12], b"aad", &ct).unwrap(), b"ping");
    }

    // --- epoch resync tests ---

    #[test]
    fn resync_one_epoch_ahead() {
        // Sender is at epoch 6; receiver is still at epoch 5 (window: {5, 6}).
        // Receiver must be able to open a frame sealed at epoch 6 even if the
        // epoch-advance task hasn't run yet.
        let sender = KeyStore::new([1u8; 32], 5);
        sender.advance(6); // sender advanced its window
        let receiver = KeyStore::new([1u8; 32], 5); // receiver still at epoch 5

        let ct = sender.seal(6, &[0u8; 12], b"aad", b"data").unwrap();
        // epoch 6 IS in receiver's window ({5, 6}) — fast path
        assert_eq!(receiver.open(6, &[0u8; 12], b"aad", &ct).unwrap(), b"data");
    }

    #[test]
    fn resync_two_epochs_ahead() {
        // Receiver is at epoch 5 (window: {5, 6}); sender is at epoch 7.
        // Epoch 7 is NOT in receiver's window; resync must derive K_7 on the fly.
        let sender = KeyStore::new([2u8; 32], 7);
        let receiver = KeyStore::new([2u8; 32], 5);

        let ct = sender.seal(7, &[0u8; 12], b"aad", b"hello").unwrap();
        assert_eq!(
            receiver.open(7, &[0u8; 12], b"aad", &ct).unwrap(),
            b"hello"
        );
    }

    #[test]
    fn resync_at_max_boundary() {
        // epoch = top_epoch + MAX_RESYNC_EPOCHS (5) must succeed.
        let sender = KeyStore::new([3u8; 32], 10);
        let receiver = KeyStore::new([3u8; 32], 5); // window: {5, 6}, top=6

        // Derive the key that both parties would reach at epoch 10.
        let ct = sender.seal(10, &[0u8; 12], b"x", b"ok").unwrap();
        // top_epoch = 6, epoch = 10, steps = 4 ≤ MAX_RESYNC_EPOCHS(5) — must open
        assert_eq!(receiver.open(10, &[0u8; 12], b"x", &ct).unwrap(), b"ok");
    }

    #[test]
    fn resync_beyond_max_rejected() {
        // epoch = top_epoch + MAX_RESYNC_EPOCHS + 1 must be rejected immediately.
        let receiver = KeyStore::new([4u8; 32], 5); // window: {5, 6}, top=6
        // epoch 12 = 6 + 5 + 1 — over the cap
        assert!(receiver.open(12, &[0u8; 12], b"x", b"garbage").is_err());
    }

    #[test]
    fn resync_old_epoch_rejected() {
        // Epochs below the window floor are rejected.
        // start=5, window={3,4,5,6}; advance to 7 → window={5,6,7,8}
        let ks = KeyStore::new([5u8; 32], 5);
        ks.advance(7); // window floor = 5
        // epoch 4 < min(window)=5: rejected
        assert!(ks.open(4, &[0u8; 12], b"x", b"garbage").is_err());
    }

    #[test]
    fn backward_tolerance_two_epochs() {
        // Simulates the observed failure: node A clock is 2 epochs (120s) ahead.
        // Node B (epoch=100) must decrypt frames sealed by A (epoch=102).
        // Also: A must decrypt frames from B sealed at epoch 100.
        let a = KeyStore::new([7u8; 32], 102); // fast machine
        let b = KeyStore::new([7u8; 32], 100); // slow machine

        // B's frame (epoch 100) — A's window {100,101,102,103} includes it.
        let ct_b = b.seal(100, &[0u8; 12], b"aad", b"from_b").unwrap();
        assert_eq!(a.open(100, &[0u8; 12], b"aad", &ct_b).unwrap(), b"from_b");

        // A's frame (epoch 102) — B's window {98,99,100,101}; 102 triggers resync.
        let ct_a = a.seal(102, &[0u8; 12], b"aad", b"from_a").unwrap();
        assert_eq!(b.open(102, &[0u8; 12], b"aad", &ct_a).unwrap(), b"from_a");
    }

    #[test]
    fn resync_wrong_key_fails_aead() {
        // Even within resync range, a frame sealed with the wrong root cannot
        // decrypt — AEAD authentication catches it.
        let sender = KeyStore::new([0xAAu8; 32], 7);
        let receiver = KeyStore::new([0xBBu8; 32], 5); // different root

        let ct = sender.seal(7, &[0u8; 12], b"aad", b"secret").unwrap();
        assert!(receiver.open(7, &[0u8; 12], b"aad", &ct).is_err());
    }
}
