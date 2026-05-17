# LITM — Lost in the Mes(h)s

A battle-hardened tactical mesh communication stack for autonomous drone swarms, built from first principles over raw IEEE 802.11 frames — no IP stack, no association, no existing protocol overhead.

### What We Built

LITM is a full-stack mesh communication system written entirely in Rust (edition 2024), structured as a Cargo workspace of five focused crates. It injects and captures raw 802.11 frames using monitor-mode WiFi adapters, giving the protocol complete control over every byte on the air.

---

### Transmission Layer

Every packet is a raw IEEE 802.11 unicast frame with a custom 22-byte header. Encryption uses **ChaCha20-Poly1305 AEAD** with an **HKDF epoch ratchet** for forward secrecy — past keys are zeroized immediately on each 60-second advance. A **per-sender 128-bit sliding window** provides replay protection. The `Kind` byte and all payload structure live inside the ciphertext, making FEC and Control frames length-indistinguishable (LPI/LPD).

Per-packet overhead: **38 bytes** (22-byte header + 16-byte AEAD tag).

---

### Delivery Layer — RaptorQ Fountain FEC

Objects are encoded with **RaptorQ fountain codes** and broadcast to the entire mesh simultaneously — no per-receiver retransmits. The sender adapts the symbol rate live from measured PRR: `target = ⌈(K + margin) / PRR⌉`. Transmission stops automatically once peer beacon bitmaps confirm that `desired_coverage` nodes have decoded. Every FEC symbol carries the full OTI, so late-joining receivers can reconstruct without any re-send. Relays forward encrypted symbol packets without ever decrypting them.

---

### Mesh Layer

- **Beacon-based membership** — 100 ms jittered beacons carry neighbor lists, PRR measurements, and a 256-bit decoded-object bitmap. Peers are evicted after 2 000 ms of silence.
- **PRR estimation** — a conservative blend of RSSI and beacon delivery ratio: `PRR = 0.3×old + 0.7×min(rssi_prr, delivery_prr)`.
- **Counter-suppressed flooding** — each relay schedules a random 0–50 ms rebroadcast delay and cancels if it overhears ≥2 copies. Dense regions self-quiet; 1× overhead at any density.
- **Link-state Dijkstra routing** — every node builds the full topology graph from received beacons; no convergence protocol needed.
- **Coordinated channel hopping** — a `ChannelSwitch` control message floods the mesh; all nodes switch atomically at the next epoch boundary (≤60 s). Zero disruption, no extra sync messages.

---

### Application Layer

**SDK** — a one-line `NodeBuilder` API exposes reliable `Text / Image / File / Custom` delivery over FEC, a fire-and-forget JPEG video lane with three quality presets, live neighbor PRR/RSSI, full topology graph, and operator-triggered anti-jam channel hops.

**REST + WebSocket API** (Axum) — bridges the Rust stack to any frontend: full node state, send text/image, push camera frames, MJPEG multipart streams per node, live RaptorQ telemetry over WebSocket, channel hop trigger.

**Svelte Dashboard** — live interactive topology map with PRR-weighted edges, real-time FEC matrix fill animation, live MJPEG video from any mesh node, and a channel control panel.

---

### Security Properties

| Threat | Mitigation |
|---|---|
| Passive eavesdrop | ChaCha20-Poly1305; Kind + payload inside ciphertext |
| Active jammer | Coordinated atomic channel hop across entire mesh |
| Replay attack | Per-sender 128-bit sliding window; checked after AEAD |
| Spoofed frames | AEAD tag rejection; wrong key → AuthFailed immediately |
| Node capture | HKDF ratchet; past keys zeroized; forward secrecy holds |
| Network partition | Counter-suppressed flooding recovers automatically on heal |

### Bandwidth Efficiency

High-MCS unicast (5–30 Mbps vs. 1–6 Mbps broadcast fallback), adaptive FEC rate, coverage-driven termination, bitmap ACKs piggybacked on beacons for zero extra packets, and a separate fire-and-forget video lane with no FEC overhead for latency-sensitive streams.

---

**Stack:** Rust · Tokio · Axum · kova-wfb-rs · ChaCha20-Poly1305 · HKDF-SHA256 · BLAKE3 · RaptorQ · Postcard · Svelte · TypeScript
