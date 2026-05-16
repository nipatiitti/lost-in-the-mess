/* MeshState.jsx — fake live mesh data + helpers */

const NODE_KIND_OK = "ok";
const NODE_KIND_RELAY = "relay";
const NODE_KIND_LOST = "lost";

/* 14 nodes positioned around a hub. x/y in % of the canvas. */
const INITIAL_NODES = [
  { id: "HUB",  label: "HUB",  x: 50, y: 50, prr: 1.00, dbm: -42, hops: 0, state: "ok",    role: "hub" },
  { id: "N-02", label: "N-02", x: 50, y: 18, prr: 0.93, dbm: -64, hops: 1, state: "ok",    role: "scout" },
  { id: "N-03", label: "N-03", x: 78, y: 26, prr: 0.88, dbm: -67, hops: 2, state: "ok",    role: "scout" },
  { id: "N-04", label: "N-04", x: 26, y: 26, prr: 0.94, dbm: -68, hops: 1, state: "ok",    role: "relay" },
  { id: "N-05", label: "N-05", x: 88, y: 50, prr: 0.81, dbm: -71, hops: 2, state: "ok",    role: "scout" },
  { id: "N-06", label: "N-06", x: 12, y: 50, prr: 0.76, dbm: -74, hops: 2, state: "ok",    role: "scout" },
  { id: "N-07", label: "N-07", x: 75, y: 75, prr: 0.86, dbm: -69, hops: 1, state: "ok",    role: "relay" },
  { id: "N-08", label: "N-08", x: 50, y: 82, prr: 0.79, dbm: -73, hops: 2, state: "ok",    role: "scout" },
  { id: "N-09", label: "N-09", x: 22, y: 78, prr: 0.51, dbm: -81, hops: 3, state: "relay", role: "scout" },
  { id: "N-10", label: "N-10", x: 92, y: 30, prr: 0.66, dbm: -77, hops: 3, state: "relay", role: "scout" },
  { id: "N-11", label: "N-11", x: 93, y: 80, prr: 0.18, dbm: -94, hops: 9, state: "lost",  role: "scout" },
  { id: "N-12", label: "N-12", x: 8,  y: 84, prr: 0.62, dbm: -78, hops: 3, state: "relay", role: "scout" },
  { id: "N-13", label: "N-13", x: 36, y: 64, prr: 0.91, dbm: -66, hops: 1, state: "ok",    role: "scout" },
  { id: "N-14", label: "N-14", x: 64, y: 38, prr: 0.84, dbm: -70, hops: 2, state: "ok",    role: "scout" },
];

/* Links — { from, to, q } where q ≈ PRR of the link */
const INITIAL_LINKS = [
  ["HUB","N-02",0.93],["HUB","N-03",0.85],["HUB","N-04",0.94],["HUB","N-05",0.81],
  ["HUB","N-06",0.76],["HUB","N-07",0.86],["HUB","N-08",0.79],["HUB","N-13",0.91],["HUB","N-14",0.84],
  ["N-04","N-06",0.71],["N-04","N-09",0.55],["N-04","N-12",0.58],
  ["N-03","N-10",0.62],["N-05","N-10",0.69],["N-05","N-11",0.22],
  ["N-07","N-08",0.74],["N-07","N-11",0.31],
  ["N-08","N-09",0.66],["N-13","N-14",0.88],
];

function stateOfPrr(p) {
  if (p < 0.25) return "lost";
  if (p < 0.6)  return "relay";
  return "ok";
}
function chipToneForState(s) {
  return s === "ok" ? "ok" : s === "relay" ? "warm" : "lost";
}
function labelForState(s) {
  return s === "ok" ? "LINK OK" : s === "relay" ? "RELAY" : "LOST";
}
function colorForState(s) {
  return s === "ok" ? "var(--signal-300)" : s === "relay" ? "var(--uplink-300)" : "var(--lost-300)";
}
function colorForPrr(p) {
  return colorForState(stateOfPrr(p));
}

/* Fake sparkline series (24 ticks) per node */
function sparkFor(id, baselinePrr) {
  let arr = [];
  let v = baselinePrr;
  const seed = id.charCodeAt(id.length - 1) || 5;
  for (let i = 0; i < 24; i++) {
    const noise = (Math.sin(i * seed * 0.7) + Math.cos(i * 0.4)) * 0.04;
    v = Math.max(0, Math.min(1, baselinePrr + noise + (Math.random() - 0.5) * 0.02));
    arr.push(v);
  }
  return arr;
}

Object.assign(window, {
  INITIAL_NODES, INITIAL_LINKS,
  stateOfPrr, chipToneForState, labelForState, colorForState, colorForPrr,
  sparkFor,
});
