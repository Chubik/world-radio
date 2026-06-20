/* World Radio Mini — theme token sets.
   Lifted from the main app's 14-theme palette system; these are the
   amber-crt default plus the alternates requested in the brief.
   Each theme: bg, panel, fg, hi (primary/highlight), dim (muted),
   ok (positive/play), warn (buffering), err (error), accent (now-playing ♪),
   scan (scanline overlay strength), glow (text glow), light (true if light bg). */
window.WR_THEMES = {
  amber: {
    name: "Amber CRT", id: "amber-crt", tag: "default",
    bg: "#15100b", panel: "#1b1510", fg: "#d49a3a", hi: "#ffc457", dim: "#6e5430",
    rule: "#3a2c17", ok: "#9ec074", warn: "#ffc457", err: "#d96a5a", accent: "#ff8a3d",
    bright: "#fff0c0", scan: 0.16, glow: "#d49a3a", light: false,
  },
  blue: {
    name: "Mainframe Blue", id: "mainframe", tag: "alt",
    bg: "#081a3a", panel: "#0d2347", fg: "#d8e8ff", hi: "#66c0ff", dim: "#3a5a8a",
    rule: "#1d3a64", ok: "#66e8a0", warn: "#ffd54a", err: "#ff7070", accent: "#ffd54a",
    bright: "#ffffff", scan: 0.12, glow: "#66c0ff", light: false,
  },
  neon: {
    name: "Cyber Neon", id: "cyber-neon", tag: "alt",
    bg: "#07041a", panel: "#100a2e", fg: "#c7c0e8", hi: "#ff2bd5", dim: "#463860",
    rule: "#2a1f4a", ok: "#6dff7f", warn: "#ffe14a", err: "#ff5050", accent: "#00ffe1",
    bright: "#ffffff", scan: 0.2, glow: "#ff2bd5", light: false,
  },
  green: {
    name: "Shortwave Green", id: "shortwave", tag: "alt",
    bg: "#061008", panel: "#0b1d0e", fg: "#7fda7f", hi: "#5fff9c", dim: "#2d6633",
    rule: "#15401d", ok: "#5fff9c", warn: "#ff9d3d", err: "#ff5c5c", accent: "#ff9d3d",
    bright: "#d6ffc8", scan: 0.22, glow: "#5fff9c", light: false,
  },
  paper: {
    name: "Hi-Fi Paper", id: "hifi-paper", tag: "light",
    bg: "#efe6cc", panel: "#e6dab8", fg: "#2e2517", hi: "#c5872a", dim: "#8a7a5a",
    rule: "#cdbf99", ok: "#5a7a3a", warn: "#c5872a", err: "#b14d2d", accent: "#a13e2d",
    bright: "#0f0a04", scan: 0.05, glow: "transparent", light: true,
  },
  nord: {
    name: "Nord", id: "nord", tag: "new",
    bg: "#2e3440", panel: "#3b4252", fg: "#d8dee9", hi: "#88c0d0", dim: "#4c566a",
    rule: "#434c5e", ok: "#a3be8c", warn: "#ebcb8b", err: "#bf616a", accent: "#81a1c1",
    bright: "#eceff4", scan: 0.0, glow: "transparent", light: false,
  },
  dracula: {
    name: "Dracula", id: "dracula", tag: "new",
    bg: "#282a36", panel: "#343746", fg: "#f8f8f2", hi: "#bd93f9", dim: "#6272a4",
    rule: "#44475a", ok: "#50fa7b", warn: "#f1fa8c", err: "#ff5555", accent: "#ff79c6",
    bright: "#ffffff", scan: 0.0, glow: "#bd93f9", light: false,
  },
};
window.WR_THEME_ORDER = ["amber", "blue", "neon", "green", "paper", "nord", "dracula"];
