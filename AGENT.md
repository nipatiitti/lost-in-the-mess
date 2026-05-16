# LITM — Lightweight Tactical Mesh

Hackathon project for the Kova Labs challenge: build a tactical mesh
communication system over raw IEEE 802.11 frames for autonomous drone
coordination in contested radio environments.

This document is the canonical reference for anyone (human or agent) working
on the project. Read it before touching code. If a decision below feels
wrong, raise it explicitly — don't quietly reverse it in a PR.

## Goals

We are competing on three judging axes:

- **Resilience under jamming and spoofing** — 34%
- **Efficient use of limited radio bandwidth** — 33%
- **Innovative applications of tactical mesh networking** — 33%

Our concrete targets:

- A working 5–6 node mesh demonstrable on Sunday afternoon.
- Solid transmission and mesh layers first; applications built on top once
  the protocol stack works end-to-end.
- A demo that visibly shows multi-hop forwarding, partition recovery, and
  bandwidth efficiency under packet loss.
- Maximum reuse of existing Rust crates — we do not reimplement
  cryptography, FEC, or radio drivers from scratch.

## Hardware

- 3× USB WiFi adapters per team: Realtek RTL8812AU (AC, packet-injection
  capable).
- Channel switch latency on this chipset: ~5–15 ms per hop. This caps any
  channel-hopping scheme to ~10–50 Hz — not true kHz-rate FHSS.
- Realistic injection throughput: ~5–30 Mbps depending on MCS and
  conditions.
- Development is on Linux only; no cross-platform support.

## Project layout

Cargo workspace, edition 2024, GPL-3.0:

```
litm/
├── Cargo.toml             # workspace root
├── AGENT.md               # this file
└── crates/
    ├── common/            # shared types & traits — no logic here
    ├── transport/         # Person A: radio I/O, framing, AEAD, replay
    ├── delivery/          # Person B: RaptorQ FEC, flow control
    ├── mesh/              # Person C: beacons, neighbors, flooding
    └── app/               # demo apps + CLI harness
```

The only external dependency that crosses crate boundaries is
`wfb_rs` (from `kova-labs/kova-wfb-rs`), used only inside `transport`.

## Architecture

Six logical layers, mapped to the challenge's three judged tiers:

```
┌──────────────────────────────────────┐ ┐
│ Application                          │ │ Application
│   Swarm coordination, ISR, C2        │ │
├──────────────────────────────────────┤ ┘
│ Mesh routing & membership            │ ┐
│   Flood, link-state, beacon FD       │ │
├──────────────────────────────────────┤ │ Mesh
│ Forward error correction             │ │
│   RaptorQ erasure coding             │ │
├──────────────────────────────────────┤ ┘
│ Cryptography                         │ ┐
│   ChaCha20-Poly1305 + FS ratchet     │ │
├──────────────────────────────────────┤ │
│ Framing                              │ │ Transmission
│   Header, sender ID, sequence        │ │
├──────────────────────────────────────┤ │
│ Radio link                           │ │
│   kova-wfb-rs, channel hopping       │ │
└──────────────────────────────────────┘ ┘
```

Crate-to-layer mapping:

- `transport` implements **Radio link + Framing + Cryptography**.
- `delivery` implements **Forward error correction**.
- `mesh` implements **Mesh routing & membership**.
- `app` implements **Application**.

## Core design decisions

Each decision below is intentional. Don't reverse without team discussion.

### 1. Broadcast is the design center

Radio is one-to-all by physics. Every primitive in our stack must exploit
this; if a design needs N transmissions to reach N receivers, redesign.

Concretely:

- **No 802.11 broadcast frames.** They fall back to ~1–6 Mbps and have no
  protection. We inject high-MCS unicast frames addressed to a sentinel
  MAC; all nodes in monitor mode demux by our own header.
- **Always-listen, never-probe.** All neighbor discovery, link quality,
  time sync, and ACK signals come from passive observation of beacons and
  forwarded traffic. No dedicated probe/ping/ACK packets in our protocol.
- **Counter-suppressed forwarding.** When a node hears a new message ID,
  it schedules a rebroadcast after a small jittered delay, counts
  overheard rebroadcasts during that delay, and cancels if `count ≥ K`
  (start with K=2). Dense regions self-quiet, sparse regions self-forward.
- **Beacon-of-everything.** One periodic encrypted beacon per node carries
  epoch, neighbor list with link qualities, and a bitmap of recently-
  decoded object IDs. Since one TX reaches all neighbors for free, we
  stuff beacons hard.
- **Implicit ACK via overhearing.** Hearing any neighbor rebroadcast a
  message is sufficient proof of delivery.
- **Bitmap-beacon coverage tracking.** Senders watch beacons to count how
  many peers have decoded each object, stopping transmission once
  `desired_coverage` is reached. This replaces all unicast ACKs.
- **Single fountain stream, many decoders.** One sender broadcasts
  RaptorQ symbols; all listeners decode independently. No per-receiver
  retransmits.

### 2. No Plumtree, no Foca

Both protocols were designed for IP overlays where unicast is cheap and
broadcast is expensive — exactly inverted from our medium. We replace
them with simpler primitives:

- **Membership:** beacon-and-timeout failure detection. A missing beacon
  for `N * beacon_interval` marks the peer down.
- **Reliable broadcast:** counter-suppressed flooding (above).
- **Unicast routing:** for 5–6 nodes, full topology is ~30 edges.
  Beacons advertise each node's neighbor list with link quality; every
  node runs Dijkstra locally on the resulting graph.

### 3. Group key with forward-secret ratchet

Not pairwise Noise, not MLS:

- **Pairwise Noise channels** would require N encryptions per broadcast,
  destroying the broadcast advantage.
- **MLS** is the correct long-term answer but too heavy for one weekend.
- **A shared group key** with periodic forward-only HKDF ratcheting is
  what tactical link encryptors do in practice and is what we ship.
- **Noise IK** (via `snow`) is used **only** for the pairwise handshake
  when a new node joins the group, or for re-keying after eviction.
  Never on the data path.

### 4. FEC above crypto, not below

Each RaptorQ symbol is wrapped in its own AEAD envelope. Reasons:

- Encrypt-then-FEC would break authentication tags on packet loss.
- FEC-then-encrypt means any K-of-N delivered envelopes recover the
  payload.
- Relays can forward encrypted symbol packets without ever decrypting
  them.
- Different relays carrying different symbols compose naturally —
  multi-path mesh delivery falls out for free.

### 5. Synchronous trait API, asynchronous internals

The traits in `common` use only synchronous methods. Each implementation
spawns its own tokio tasks behind the API. Reasons:

- No `async_trait` macro needed.
- Traits stay `dyn`-compatible — we use `Arc<dyn Transport>` everywhere.
- Backpressure is expressed through mpsc channel capacities, not Future
  composition.

### 6. Channels, not callbacks

`subscribe()` returns a `tokio::sync::mpsc::Receiver`. Each module spawns
a task that reads from its mailbox. No callback registries, no shared
mutex on handler lists.

### 7. `Kind` byte lives inside the AEAD ciphertext

The packet kind (Beacon / Fec / Control) is the first byte of the
_plaintext_ fed to ChaCha20-Poly1305. A passive eavesdropper cannot
distinguish beacons from data frames — this matters for low-probability-
of-detection (LPI/LPD). The cost is one byte per packet.

## The shared contract

`crates/common/src/lib.rs` is the single source of truth. The contract is
deliberately small:

- **IDs:** `NodeId = u32`, `ObjectId = u32`, `Epoch = u32`.
- **Constants:** `PROTOCOL_VERSION`, `MAX_PLAINTEXT = 1400`,
  `BITMAP_WORDS = 4`.
- **Error/Result:** one shared `Error` enum, one `Result<T>`. No anyhow,
  no per-crate error types.
- **`Kind`:** `Beacon | Fec | Control`.
- **`PacketMeta`:** sender_id, rssi_dbm, recv_time.
- **`ObjectBitmap`:** 256-bit ring of recently-decoded object IDs.
- **`SendPolicy`:** desired_coverage, ttl, priority.
- **`DeliveredObject`:** id, source, payload.
- **`NeighborInfo`:** id, prr, last_seen, bitmap.
- **Traits:** `Transport`, `Delivery`, `Mesh`.

Trait dependencies:

- `delivery` constructor takes `Arc<dyn Transport>`.
- `mesh` constructor takes `Arc<dyn Transport>` and `Arc<dyn Delivery>`.
- `app` constructor takes `Arc<dyn Delivery>` and `Arc<dyn Mesh>`.

Each crate is independently testable by mocking the layer below.

## Wire formats

### Transport envelope (owned by `transport`)

```
 byte:  0   1   2          6          10                 18
        +---+---+----------+----------+------------------+--------------------------+
        |ver|flg|  epoch   |   sid    |     counter      |  ciphertext + tag (16B)  |
        | 1 | 1 |    4     |    4     |        8         |     variable length      |
        +---+---+----------+----------+------------------+--------------------------+
        \________________________ AAD (18 bytes) ______/\______ AEAD output ________/
```

- AEAD: ChaCha20-Poly1305 (RustCrypto).
- Nonce (12B) = `sid (4B) || counter (8B)` — globally unique without
  coordination.
- AAD = the first 18 bytes of the wire frame.
- Plaintext = `Kind (1B) || inner payload (≤ MAX_PLAINTEXT-1)`.
- Per-packet overhead: **34 bytes** (18 header + 16 tag).

### Key schedule

- Root key `K_root` provisioned out-of-band (hardcoded for demo; QR / NFC
  for production).
- Epoch advance every 60 s:
  `K_{e+1} = HKDF(K_e, "kova-mesh/ratchet/v1")`.
- Old `K_e` is **zeroized immediately** — forward secrecy holds even if
  the device is captured later.
- Each node keeps a sliding window of 3 epoch keys (`e-1, e, e+1`) to
  tolerate clock skew and packet reorder.
- Per-sender replay: 128-bit sliding bitmap below `highest_counter_seen`.
- Post-compromise security: triggered via `Kind::Control` rekey message,
  with the new root distributed pairwise over Noise IK to surviving peers.

### FEC frame (owned by `delivery`, plaintext when `Kind::Fec`)

```
+----------+--------+--------+--------+-----------------------+
| object_id| oti    | esi    | sym_sz | symbol_bytes (≈1400B) |
|   u32    |  12B   |  u32   |  u16   |                       |
+----------+--------+--------+--------+-----------------------+
```

- `oti` is `raptorq::ObjectTransmissionInformation` serialized —
  included in every symbol so receivers can join a stream mid-flight.
- Symbol size T tuned so the total post-envelope frame is ≤ 1500B.
- K (source symbols per block) chosen per object based on payload size.
- Adaptive rate:
  `target_symbols = ceil((K + 4) * 1.2 / observed_prr)`.

### Beacon payload (owned by `mesh`, plaintext when `Kind::Beacon`)

Serialized with `postcard`:

```rust
struct BeaconPayload {
    epoch: u32,
    neighbors_heard: Vec<(NodeId, u8 /* PRR * 255 */)>,
    decoded: ObjectBitmap,
}
```

- Beacon interval: 500 ms (jittered ±10%).
- Liveness timeout: 3 missed beacons → peer marked down.

## Ownership

| Person | Crate(s)           | Owns                                                             |
| ------ | ------------------ | ---------------------------------------------------------------- |
| A      | `transport`        | kova-wfb-rs integration, wire format, AEAD, ratchet, replay      |
| B      | `delivery`         | RaptorQ encode/decode, FEC frame, adaptive rate, bitmap export   |
| C      | `mesh` + `app/ops` | Beacons, neighbor table, flooding, channel hopping, demo harness |

Application layer is chosen Sunday morning and built collaboratively by
whoever finishes their core work first.

## Milestones

### Friday evening (all hands, ~2 hours)

1. Pick up radios, install drivers, prove packet injection on at least
   one machine using a kova-wfb-rs example.
2. Scaffold the cargo workspace.
3. Lock the interfaces in `common`.
4. Each person writes `todo!()` stubs that compile.
5. Agree on repo layout, branching, log format.

### Saturday

- **12:00** — Transport ferries a real packet between two machines.
  Delivery works against a mock transport. Beacons parse.
- **17:00** — End-to-end: one object sent through real radio, real
  crypto, real FEC, decoded at one peer.
- **22:00** — Three-node test: object delivered via one-hop forwarding.
  Beacon bitmaps stopping the sender. Counter-suppressed flooding
  working.

### Sunday

- **11:00** — Five-node test with simulated partition (MAC-based
  filter). Channel hopping demo if Person C had time.
- **14:00** — Application layer built on top of `Delivery` and `Mesh`.
- **16:00** — Dry-run the demo end to end.

## Recommended crates

Used in our stack:

- `raptorq` — fountain code FEC
- `chacha20poly1305` — AEAD
- `hkdf`, `sha2` — key derivation
- `blake3` — hashing where speed matters
- `snow` — Noise IK for the pairwise join handshake only
- `tokio` — async runtime
- `postcard` — compact serialization for beacons
- `thiserror` — error enums in `common`
- `zeroize` — key material lifecycle
- `tracing` — structured logging

Deliberately **not** used:

- `foca` — SWIM-style membership; not a fit for a broadcast medium.
- `plumtree-rs` — gossip overlay; designed for IP unicast substrate.
- `anyhow` — we use one shared `Error` enum from `common`.

## Conventions

- **Edition:** 2024.
- **License:** GPL-3.0.
- **Errors:** every public function returns `litm_common::Result<T>`.
  Add variants to the shared `Error` if needed.
- **Async:** internal only. All trait methods are sync.
- **Channels:** `tokio::sync::mpsc` for cross-task data flow. Capacity
  is a deliberate decision per crate.
- **Logging:** `tracing` everywhere. Each crate sets its own target.
- **Testing:** each crate tests in isolation with a mock of the layer
  below. Avoid integration-test-first development; it hides regressions.
- **No global state.** Construct services explicitly and pass
  `Arc<dyn _>` references.
- **Zeroize key material.** Use the `zeroize` crate on all key buffers.
- **Interface drift:** any change to a `common` trait or shared struct
  requires sign-off from all three owners before merge.

## Non-goals

These were considered and explicitly excluded:

- Full MLS group encryption (too heavy for the weekend; future work).
- True kHz-rate frequency hopping (RTL8812AU channel switch is too slow).
- Per-node multi-radio diversity (only 3 adapters total per team).
- Application-layer features before the protocol stack is solid.
- BATMAN-adv / OLSR style routing complexity (overkill for 5–6 nodes).
- Cross-platform support beyond Linux.

## Demo plan (finalize Sunday morning)

The chosen application must showcase:

1. **Multi-hop delivery** — partition the topology and show data still
   flowing.
2. **Bandwidth efficiency** — show throughput vs naive flooding under
   loss.
3. **Resilience** — kill a relay node mid-demo; show recovery.
4. **One novel application** — to be picked Sunday based on what works.

Candidates for the novel application:

- Coordinated swarm-state consensus (leader election + state replication).
- Progressive image streaming from a "scout" node with multi-path FEC.
- Distributed target tracking with sensor fusion across nodes.
- Spectrum-aware jamming detection with automatic reroute.

## Glossary

- **AEAD** — Authenticated Encryption with Associated Data. We use
  ChaCha20-Poly1305.
- **ESI** — Encoded Symbol Identifier. RaptorQ's per-symbol index.
- **FEC** — Forward Error Correction.
- **FHSS** — Frequency-Hopping Spread Spectrum.
- **FS / PCS** — Forward Secrecy / Post-Compromise Security.
- **LPI / LPD** — Low Probability of Intercept / Detection.
- **MCS** — Modulation and Coding Scheme (802.11 rate index).
- **OTI** — Object Transmission Information. RaptorQ's per-object
  metadata blob.
- **PRR** — Packet Reception Ratio. Our rolling estimate of link
  quality.
- **SID** — Sender ID. 32-bit node identifier on the wire.
- **SWIM** — Scalable Weakly-consistent Infection-style process group
  Membership.
