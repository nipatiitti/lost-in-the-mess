# Mesh Console — UI Kit

The operator dashboard for **Lost in the mes(h)s**. Live mesh topology,
per-node telemetry (ID / PRR / RSSI / hops), packet log, and a slot for
"applications" that ride the mesh (image relay, text channel, video
stream, etc).

## What's in here

| File | Purpose |
|---|---|
| `index.html` | Interactive prototype — open this first |
| `App.jsx` | Top-level shell, screen routing, fake mesh state |
| `Sidebar.jsx` | Left rail nav + mesh-wide health |
| `StatusBar.jsx` | Top sticky status (mesh health, time, ops) |
| `TopologyView.jsx` | The graph — nodes + PRR-coloured links |
| `NodeGrid.jsx` | Grid of node cards (the nominal/degraded/lost states) |
| `NodeInspector.jsx` | Per-node drawer — telemetry detail + sparkline |
| `PacketLog.jsx` | Mono-formatted live packet stream |
| `UplinkApps.jsx` | Mesh applications launcher |
| `Composer.jsx` | "Send a thing through the mesh" composer |
| `Primitives.jsx` | Shared atoms — Stamp, Chip, Button, Panel, IconButton |

## Click-thru flows

1. **Topology → Inspect node** — click any node on the graph or grid → drawer opens with sparkline + per-link state.
2. **Compose & send** — switch to *Uplink*, pick a node + an app (image / text), hit Send, watch the packet appear in the log with `OK` or `LOST`.
3. **Apps tab** — see currently mounted mesh applications + an empty slot.

The interactions are faked (no real radios, no backend) — the goal is
visual fidelity and feel, not function.
