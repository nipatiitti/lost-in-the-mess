---
name: lost-in-the-meshs-design
description: Use this skill to generate well-branded interfaces and assets for "Lost in the mes(h)s" (a mesh drone-swarm telemetry console), either for production or throwaway prototypes/mocks/etc. Contains essential design guidelines, colors, type, fonts, brand assets, and a UI kit recreating the Mesh Console dashboard. Best for tactical telemetry, mesh/networking, and aerospace-engineering aesthetics that lean dark, mono, and instrument-like.
user-invocable: true
---

# Lost in the mes(h)s — design skill

Read `README.md` in this skill first — it's the canonical brief. Then
explore:

| File / Folder | Use it when… |
|---|---|
| `colors_and_type.css` | You need design tokens. Drop `@import "<skill>/colors_and_type.css";` into any new CSS, then use the `--ink-*`, `--bone-*`, `--signal-300`, `--uplink-300`, `--drift-300`, `--lost-300`, `--space-*`, `--radius-*`, `--font-*`, `--glow-*` vars. |
| `fonts/` | Font swap notes — currently Google Fonts substitutes. |
| `assets/` | Brand wordmark, node mark, mesh topology illustration, signal-ring illustration, favicon. Copy these out; never trace them. |
| `preview/` | Token preview cards — copy markup patterns from here for buttons, chips, node cards, packet logs, inputs, tabs, etc. |
| `ui_kits/dashboard/` | A working React prototype of the Mesh Console. `App.jsx` shows screen routing, `Primitives.jsx` has shared atoms (`Stamp`, `Chip`, `Button`, `IconButton`, `Panel`, `I` icon map, `Sparkline`), `MeshState.jsx` has fake telemetry helpers. Use these components/patterns when building new screens. |

## Workflow

If asked to **make a visual artifact** (slides, mocks, prototypes,
throwaway UI):
1. Copy any brand asset you reference into your output directory; don't
   reach across the skill in production bundles.
2. Import `colors_and_type.css` so the tokens cascade. Don't hand-pick
   hex codes.
3. Use the patterns in `preview/` and `ui_kits/dashboard/` as your
   visual library. Match cap, casing, mono usage, and the dry voice
   guide in `README.md` §2.
4. Use Lucide icons (16/20px, 1.5px stroke, `currentColor`) — see
   `preview/brand-icons.html` for the curated set.

If working on **production code**, this folder is a reference, not a
library. Read the rules, then implement them in your stack — the tokens
are portable to Tailwind config, CSS-in-JS, or anything else.

## Default questions to ask

If the user invokes this skill without context, ask:
1. **What surface?** (a single screen, a deck, a marketing page, a hero illu, a CLI screenshot?)
2. **What state is the swarm in?** (nominal, partially degraded, full meltdown — this drives the color palette load.)
3. **Hero element?** (topology graph, single node, packet log, swarm flying — picks the illu motif.)
4. **Audience?** (operator-facing → terminal density; pitch deck → display sizes + brand mark; field report → mono + ALL CAPS stamps.)

Then act as an expert designer who outputs HTML artifacts OR production
code, depending on need.
