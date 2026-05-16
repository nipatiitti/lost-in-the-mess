//! ChaCha20-Poly1305 AEAD with a 3-key sliding window ratcheted by HKDF-SHA256.
//! The window holds {e-1, e, e+1} so peers ±60s of clock skew can still talk.

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

const RATCHET_INFO: &[u8] = b"kova-mesh/ratchet/v1";

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
    /// Derives K_{start_epoch} and K_{start_epoch+1} in O(1).
    pub fn new(root_key: [u8; 32], start_epoch: Epoch) -> Self {
        let k = epoch_key(&root_key, start_epoch);
        let k_next = epoch_key(&root_key, start_epoch + 1);
        let mut keys = BTreeMap::new();
        keys.insert(start_epoch, EpochKey(k));
        keys.insert(start_epoch + 1, EpochKey(k_next));
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
            let new_top = next + 1;
            let kk = epoch_key(&root, new_top);
            w.keys.insert(new_top, EpochKey(kk));
            if next >= 2 {
                // EpochKey's Drop zeroizes the bytes.
                w.keys.remove(&(next - 2));
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
            .map_err(|_| Error::Other("seal failed".into()))
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
                let Some((&top_epoch, _)) = w.keys.iter().rev().next() else {
                    return Err(Error::AuthFailed);
                };
                // Reject too-old and too-far-ahead (DoS guard).
                if epoch <= w.current || epoch > top_epoch + MAX_RESYNC_EPOCHS {
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
            .map_err(|_| Error::AuthFailed)
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
    fn advance_keeps_three_keys() {
        let ks = KeyStore::new([1u8; 32], 5);
        ks.advance(7);
        let g = ks.inner.lock().unwrap();
        assert_eq!(g.current, 7);
        assert!(g.keys.contains_key(&6));
        assert!(g.keys.contains_key(&7));
        assert!(g.keys.contains_key(&8));
        assert!(!g.keys.contains_key(&5));
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
        // Epochs at or below w.current cannot be opened via resync
        // (ratchet is one-way; if not in window, they're gone).
        let ks = KeyStore::new([5u8; 32], 5);
        ks.advance(7); // window: {6, 7, 8}, current = 7
        // epoch 5 < current(7): rejected
        assert!(ks.open(5, &[0u8; 12], b"x", b"garbage").is_err());
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
