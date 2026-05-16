# Lost in the Mes(h)s — Design System

> Mesh drone-swarm telemetry, with a sense of humor about the wires.

This is the design system for **Lost in the mes(h)s** — a futuristic mesh
networking dashboard for drone swarms. Each node (a radio link in the air)
reports its **ID**, **PRR** (packet reception ratio / signal strength), and
its place in the **graph topology** of the swarm. On top of that mesh you can
send images, text, video, and eventually a whole catalog of applications.

The product is half air-traffic-control, half terminal — and the brand winks
at the inevitable chaos of routing packets through 14 flying radios that all
hate weather. That's the **(h)** in *mes(h)s*: the wiring is the mess.

> Status: greenfield. No existing codebase, repo, or Figma was attached when
> this system was generated — every visual decision below is a proposal,
> intentionally bold, and meant to be edited together with you.

---

## Sources

| Source | Status |
|---|---|
| Codebase (GitHub / local) | ⛔ none provided |
| Figma file | ⛔ none provided |
| Brand guide / wordmark | ⛔ none provided |
| Product copy / decks | ⛔ none provided |

Everything in `assets/`, `colors_and_type.css`, `preview/`, and `ui_kits/` is
a generated proposal we can iterate on once you share what you already have.

---

## Index

| File / Folder | What's in it |
|---|---|
| `colors_and_type.css` | All design tokens — colors, type, spacing, radii, shadows, motion, semantic aliases |
| `fonts/` | Notes on the font choices and how to swap in licensed files |
| `assets/` | Wordmark, node-mark, mesh-topology hero illu, signal-ring illu, favicon |
| `preview/` | One small HTML card per token cluster — these populate the Design System tab |
| `ui_kits/dashboard/` | High-fidelity recreation of the Mesh Console (sidebar, node grid, topology view, app modules) |
| `SKILL.md` | Agent-Skill manifest so this folder doubles as a portable design skill |

---

## 1. Brand concept

**Lost in the mes(h)s** is the operator's console for a swarm of mesh-radio
drones. Operators are technical — engineers, RF folks, field-ops — so the
voice leans dry, precise, and a little gallows. We celebrate when packets
land. We forgive when they don't.

The wordmark itself is the tension model:

```
LOST IN THE MES (h) S
        ↑ the parenthetical is the bug,
          but also the whole point.
```

It signals a system that admits its own messiness — and a UI that surfaces
the mess instead of hiding it.

### Products

The system currently scopes one product surface:

1. **Mesh Console** — the web dashboard. Live topology graph, per-node
   telemetry (ID, PRR, RSSI, hops), uplink/downlink app slots (image,
   text, video, generic file), and a packet log. *See `ui_kits/dashboard/`.*

Future surfaces (not yet built) might include: a CLI (`mesh`), an Android
field-ops companion, and a printed quick-reference card. Token system is
ready for them.

---

## 2. Content fundamentals

### Voice
Dry, technical, observational. We write the way a field engineer
narrates a debug log to themself. No marketing puff. No "Welcome!".

### Person
**Second person is rare.** Prefer third-person/system voice:
> ✅ "Node N-07 dropped 3 packets."
> ✅ "Link to N-11 is degraded — switching to relay."
> ❌ "We've noticed your node is having trouble!"
> ❌ "You're connected!"

When we DO address the user, it's curt and command-grade:
> ✅ "Confirm: reroute through N-04?"

### Casing
- **Headlines, hero copy:** sentence case, no period.
- **All-caps stamps:** for metadata, status, section labels — `MESH.TOPO`,
  `PRR 0.87`, `UTC 22:14:07`, `LINK OK`. Always with letter-spacing.
- **Code / IDs:** monospace, no caps change — `N-07`, `2.4GHz`, `-68dBm`.

### Numbers, units, and IDs
Always monospaced with tabular numerals (`font-feature-settings: "tnum"`).
Units sit next to the number with no space (`-68dBm`, `2.4GHz`) **except**
for time (`22:14 UTC`). Node IDs use the pattern `N-NN` (zero-padded, two
digits, dash separator). Hub is the literal string `HUB`.

### Emoji
**No emoji.** Ever. We use sharp icon glyphs or mono symbols (`◇ ◆ ▲ ▼ ●
○ ▣ ▢ →`) when a glyph is needed.

### Tone examples
| Situation | Copy |
|---|---|
| Empty state | `NO NODES IN RANGE. WAITING ON BEACON.` |
| Success toast | `ROUTE COMMITTED → N-04 → HUB. 6 HOPS.` |
| Error toast | `PACKET LOST. RETRYING IN 200ms.` |
| Tooltip | `PRR — Packet Reception Ratio. 1.0 = lossless.` |
| CTA | `OPEN UPLINK` (not "Get started") |
| Section header | `02 / TOPOLOGY` |

### Punctuation
- En-dash for ranges (`-72 – -65 dBm`).
- Arrows `→` for flow (`N-04 → HUB`).
- Slashes `/` to namespace metadata (`MESH.TOPO / N=14 / PRR.AVG 0.87`).
- Periods end full prose sentences; never end stamps or labels.

---

## 3. Visual foundations

### Color
Three accent families, plus ink + bone for the surface/text tier. Tokens
in `colors_and_type.css`.

| Token | Hex | Role |
|---|---|---|
| `--ink-050` | `#070B12` | Primary surface (the void) |
| `--ink-100` | `#0B1119` | Panel |
| `--ink-200` | `#111824` | Elevated panel |
| `--bone-100` | `#F4F7F2` | Primary text |
| `--bone-300` | `#9BA59E` | Tertiary / labels |
| `--signal-300` | `#6DF7B5` | **Brand mint** — link health, primary accent |
| `--uplink-300` | `#FFB347` | **Brand amber** — active command, mid-PRR |
| `--drift-300` | `#6FC3FF` | Info / data |
| `--lost-300` | `#FF4A8B` | Packet loss / node down |
| `--warn-300` | `#E8D55C` | Caution / latency |

**PRR scale** (used in the topology graph and node cards) is a single
gradient pinned at signal-300 (≥76%), uplink-300 (26–75%), and lost-300
(0–25%), with warn-300 marking the threshold band.

**Imagery vibe:** cool. Slight green-cyan cast in the ink. No warm
imagery anywhere — even the amber accent is held in check, used for
*actionable* states only (active route, command pending, etc).

### Type
- **Display & UI:** Space Grotesk 500–600 (tight tracking on display sizes).
- **Body fallback:** Inter Tight 400–500.
- **Data, IDs, code, stamps:** JetBrains Mono 400–500 (wide tracking when
  all-caps).

All three are pulled from Google Fonts via `@import` in
`colors_and_type.css`. Substitutions to flag once a real type license is
chosen (e.g. Berkeley Mono, GT Sectra Mono, ABC Diatype Mono).

### Spacing
4-px base. Use the `--space-*` ramp; no arbitrary pixel values in
component code. Cards typically pad `var(--space-5)` (20px). Dense data
tables pack to `var(--space-2)` (8px) row height.

### Layout rules
- **Fixed left rail** (240px) holds nav; never collapses on desktop.
- **Top status bar** (48px, full-bleed, sticky) shows mesh-wide health.
- **Content grid** is 12-col with 24px gutter on the main canvas.
- The **topology graph** is always full-bleed inside its panel — no
  inner padding around the SVG canvas; corner brackets do the framing.
- Numbers / IDs / stamps right-align in tables. Labels left-align.

### Backgrounds
- Solid `--ink-050` with an optional **24px dotted grid overlay**
  (`.grid-overlay` utility). The grid uses `rgba(109,247,181,0.06)` so
  it whispers — it's not a UI element, it's air.
- Hero / heavy panels add a 3-px scanline texture (`.scanlines`) at ~1%
  opacity. Subtle CRT memory.
- **No gradients** as backgrounds. The only gradient we use is the
  radial halo around active nodes in the topology graph.

### Borders
1px hairlines everywhere — `var(--border)` for in-panel dividers,
`var(--border-strong)` for the outer rule of a panel. We rarely use
rounded panels: **default radius is 2px (`--radius-1`)**, sometimes
4px on cards. Pills exist only for status chips.

### Shadows + glows
- `--shadow-1/2/3` for surface depth (used sparingly — this is a flat
  system).
- **Glow tokens** are the signature: `--glow-signal`, `--glow-uplink`,
  `--glow-lost`. They're a 1px outline plus a 16-18px blur in the
  accent color, used to mark "live" elements (the active node, an
  in-flight packet, a fresh alert). Glow is energy; ordinary state has
  none.

### Transparency + blur
- Tooltips: `rgba(7,11,18,0.92)` with 8px backdrop-blur.
- Overlays / modals: `rgba(4,6,10,0.7)` with 16px blur.
- Cards never blur — they're solid panels.

### Motion
- **Default easing:** `--ease-snap` (`cubic-bezier(0.2, 0.8, 0.2, 1)`).
  Things settle quickly with a sliver of overshoot.
- **Default duration:** 200ms. Most state changes are 120ms (fast) or
  200ms (base). Never longer than 420ms.
- **No bounces.** No spring physics. Snap, settle, done.
- **Live data** ticks in via a 120ms opacity-fade on the changing digits
  (`.ticker`); no number-flip animation, no roulette.
- **Pulsing** only on the active node halo (2s opacity loop). Reserved.

### Hover, press, focus
- **Hover:** background lightens to `--ink-300`. No color change on text.
- **Press:** background goes to `--ink-200`, scales to `0.99`.
- **Focus:** 1px outline in `--signal-300`, 2px offset. Never a glow on
  focus — glow is reserved for active mesh state.
- **Selected:** 1px left rule in `--signal-300`, no fill change.

### Cards
Solid `--bg-panel`, 1px hairline border, `--radius-2`. No drop shadow by
default — depth comes from value contrast, not blur. Optional corner
brackets (top-left + bottom-right) for "framed" emphasis. Card titles use
the `.h-eyebrow` style (mono, ALL CAPS, wide tracking, muted).

### Corner brackets
A recurring motif: four 16px `┏┓┗┛`-style corner brackets in `--bone-100`
at 50% opacity. They frame the brand mark, the topology hero, and any
panel that wants "instrument" weight. They are decorative — never wrap
real content.

---

## 4. Iconography

We use **[Lucide](https://lucide.dev)** as the icon system, loaded from
its CDN. Reasons: 1.5px stroke matches our hairline-everything aesthetic;
the geometric grid feels engineered; the set is comprehensive enough to
cover a telemetry UI (Radio, Signal, Wifi, Activity, Send, Image, FileText,
Film, Plus, Grid, etc).

> **Substitution note:** If "Lost in the mes(h)s" already has a proprietary
> icon set, drop the SVGs in `assets/icons/` and switch UI kit imports.

**Rules:**
- Always 16px or 20px. Never bigger inline. The brand mark is the only
  "big" mark.
- Stroke `currentColor`. Inherit from text color.
- Icon-only buttons get a tooltip — no exceptions.
- No emoji, ever. No Unicode dingbats *except* the curated set used as
  data glyphs in mono runs (`◇ ◆ ▲ ▼ ● ○ → /`).
- Lucide's `Radio`, `Wifi`, `Activity`, `RadioTower`, and `Signal` icons
  are reserved for actual mesh state — don't use them decoratively.

---

## 5. Component primitives (preview cards)

See `preview/`. Each card is a self-contained HTML file at ~700×variable
that the Design System tab renders as a tile. Groups:

- **Type** — display scale, mono scale, semantic stamps
- **Colors** — ink, bone, signal, uplink, drift, lost, PRR scale
- **Spacing** — 4px ramp, radii, shadow + glow system
- **Components** — buttons, status chips, node cards, packet log row,
  inputs, tabs, toolbar
- **Brand** — wordmark, node mark, mesh topology, signal rings

---

## 6. SKILL packaging

`SKILL.md` makes this folder usable as a portable Agent Skill — drop the
whole project into `~/.claude/skills/lost-in-the-meshs/` (or equivalent)
and Claude Code can use it to brand-correct mocks, slides, and
prototypes.
