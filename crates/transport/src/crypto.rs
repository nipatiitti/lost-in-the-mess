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
use zeroize::{Zeroize, ZeroizeOnDrop};

use litm_common::{Epoch, Error, Result};

const RATCHET_INFO: &[u8] = b"kova-mesh/ratchet/v1";

#[derive(Zeroize, ZeroizeOnDrop)]
struct EpochKey([u8; 32]);

pub struct KeyStore {
    inner: Mutex<KeyWindow>,
}

struct KeyWindow {
    current: Epoch,
    keys: BTreeMap<Epoch, EpochKey>,
}

impl KeyStore {
    /// Initialize from a 32-byte root and the starting epoch.
    /// Forward-ratchets `start_epoch` times to derive K_{start_epoch}, then
    /// also derives K_{start_epoch+1}. There is no K_{start_epoch-1} because
    /// the ratchet is one-way; the window grows to 3 keys after the first advance.
    pub fn new(mut root_key: [u8; 32], start_epoch: Epoch) -> Self {
        let mut k = root_key;
        for _ in 0..start_epoch {
            k = ratchet(&k);
        }
        let k_next = ratchet(&k);
        let mut keys = BTreeMap::new();
        keys.insert(start_epoch, EpochKey(k));
        keys.insert(start_epoch + 1, EpochKey(k_next));
        root_key.zeroize();
        Self {
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
    /// Each step adds K_{new_top+1} and drops K_{new_top-2}.
    pub fn advance(&self, target: Epoch) {
        let mut w = self.inner.lock().unwrap();
        while w.current < target {
            let next = w.current + 1;
            let new_top = next + 1;
            let Some(k_next) = w.keys.get(&next) else {
                break;
            };
            let kk = ratchet(&k_next.0);
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

    pub fn open(&self, epoch: Epoch, nonce: &[u8; 12], aad: &[u8], ct: &[u8]) -> Result<Vec<u8>> {
        let w = self.inner.lock().unwrap();
        let k = w.keys.get(&epoch).ok_or(Error::AuthFailed)?;
        let cipher = ChaCha20Poly1305::new(Key::from_slice(&k.0));
        cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct, aad })
            .map_err(|_| Error::AuthFailed)
    }
}

fn ratchet(k: &[u8; 32]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, k);
    let mut out = [0u8; 32];
    hk.expand(RATCHET_INFO, &mut out).expect("HKDF expand 32B");
    out
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
}
