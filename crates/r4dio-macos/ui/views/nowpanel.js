import { stateLabels } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

/** the spectrum is read more often than the rest of the state — it is the one
 *  thing on screen meant to move, and a once-a-second read makes it lurch.
 *
 *  every read is an ipc round trip, so the rate is a cpu cost, not a free one:
 *  at 10/s this window sat at ~7% cpu. reading 5 times a second and letting css
 *  carry each bar to its next value looks the same and costs half. the state
 *  behind it changes at human speed and stays at 1s. */
const SPECTRUM_MS = 200;
const STATE_MS = 1000;
const BARS = 34;

export function mountNowPanel({ onChanged }) {
  const art = document.getElementById("art");
  const name = document.getElementById("np_name");
  const meta = document.getElementById("np_meta");
  const status = document.getElementById("np_status");
  const play = document.getElementById("np_play");
  const star = document.getElementById("np_star");
  const shuffleBtn = document.getElementById("np_shuffle");
  const volbar = document.getElementById("np_volbar");
  const vol = document.getElementById("np_vol");
  const scopeLine = document.getElementById("np_scope");

  let state = null;
  let specTimer = null;
  let stateTimer = null;
  // whether the bars are already parked at their silent height, so a stopped
  // stream repaints them once instead of on every tick.
  let atRest = false;
  let style = "bars";

  const bars = [];
  for (let i = 0; i < BARS; i++) {
    const bar = document.createElement("i");
    bar.style.height = "6%";
    art.appendChild(bar);
    bars.push(bar);
  }

  /** each style is the same levels drawn a different way, so switching one
   *  never touches the analyser — only what the bars do with what it says. */
  const SHAPES = {
    // a level per bar, growing from the floor.
    bars: (v) => ({ height: pct(v), offset: 0 }),
    // the same, hung from the middle so it grows both ways.
    mirror: (v) => ({ height: pct(v), offset: (100 - clamp(v)) / 2 }),
    // a single mark riding the top of where the bar would end.
    dots: (v) => ({ height: 8, offset: Math.max(0, clamp(v) - 8) }),
    // neighbours averaged into the level, which rounds the profile off.
    wave: (v) => ({ height: pct(v), offset: 0 }),
  };

  const clamp = (v) => Math.max(6, Math.min(100, Math.round(v * 100)));
  const pct = (v) => clamp(v);

  function smooth(values) {
    // wave reads the shape rather than the peaks, so each bar is pulled towards
    // its neighbours — the same levels, with the spikes taken off.
    return values.map((v, i) => {
      const a = values[i - 1] ?? v;
      const b = values[i + 1] ?? v;
      return (a + v * 2 + b) / 4;
    });
  }

  function paintSpectrum(input) {
    // an idle station keeps the bars at rest rather than at zero: a flat row of
    // nothing reads as a broken meter, a low flat row reads as silence. writing
    // that row once is enough — repainting it on a timer is work with no pixels
    // to show for it.
    if (!input || input.length === 0) {
      atRest = true;
      art.classList.add("idle");
      bars.forEach((bar) => {
        bar.style.height = "6%";
        bar.style.marginBottom = "0";
      });
      return;
    }
    atRest = false;
    art.classList.remove("idle");
    const values = style === "wave" ? smooth(input) : input;
    const shape = SHAPES[style] ?? SHAPES.bars;
    bars.forEach((bar, i) => {
      const { height, offset } = shape(values[i % values.length] ?? 0);
      bar.style.height = `${height}%`;
      bar.style.marginBottom = `${offset}%`;
    });
  }

  function paint() {
    const s = state ?? {};
    const idle = !s.station;
    const label = stateLabels(s.phase);

    name.textContent = s.station ?? "nothing playing";
    name.classList.toggle("idle", idle);

    meta.replaceChildren();
    for (const part of (s.meta ?? "").split("·").map((p) => p.trim()).filter(Boolean)) {
      const span = document.createElement("span");
      span.textContent = part;
      meta.appendChild(span);
    }

    const tone = { playing: "", buffering: "buffering", error: "error" }[s.phase] ?? "idle";
    status.className = `status ${tone}`;
    status.replaceChildren();
    const dot = document.createElement("span");
    dot.className = "dot";
    dot.textContent = "●";
    status.append(dot, label.text);

    play.textContent = s.phase === "playing" ? "⏸" : "▶";
    star.textContent = s.is_favorite ? "★" : "☆";
    star.classList.toggle("on", !!s.is_favorite);
    // nothing playing is nothing to star, and a button that silently does
    // nothing is worse than one that looks unavailable.
    star.classList.toggle("off", idle);

    const level = Math.round((s.volume ?? 0) * 100);
    volbar.firstElementChild.style.width = `${level}%`;
    vol.textContent = `${level}`;

    scopeLine.replaceChildren();
    const scope = s.scope === "favorites" ? "★ favourites" : "all stations";
    const b = document.createElement("b");
    b.textContent = scope;
    scopeLine.append("shuffle draws from ", b);
    if (s.filter) {
      scopeLine.append(` · ${s.filter}`);
    }
  }

  async function refresh() {
    let next;
    try {
      next = await invoke("now_state");
    } catch (e) {
      console.error("now_state failed", e);
      return;
    }
    const was = state?.station;
    state = next;
    paint();
    if (was !== next.station && onChanged) onChanged();
  }

  /** the meter's own settings, told to it by the settings pane. `off` stops the
   *  polling outright rather than drawing an empty meter on a timer. */
  function setStyle(next) {
    style = next;
    art.classList.toggle("off", next === "off");
    if (next === "off") {
      paintSpectrum(null);
      return;
    }
    atRest = false;
  }

  async function tickSpectrum() {
    if (style === "off") {
      return;
    }
    // nothing playing means nothing to meter: the bars are already at rest, so
    // asking the backend five times a second for silence is pure cpu.
    if (state?.phase !== "playing") {
      if (!atRest) paintSpectrum(null);
      return;
    }
    try {
      paintSpectrum(await invoke("spectrum"));
    } catch (e) {
      console.error("spectrum failed", e);
    }
  }

  async function send(cmd, args) {
    try {
      await invoke(cmd, args);
    } catch (e) {
      console.error(`${cmd} failed`, e);
    }
    await refresh();
  }

  play.addEventListener("click", () => toggle());
  shuffleBtn.addEventListener("click", () => shuffle());
  star.addEventListener("click", () => {
    if (!state?.station) return;
    send("toggle_favorite");
  });
  volbar.addEventListener("click", (e) => {
    const box = volbar.getBoundingClientRect();
    const v = Math.max(0, Math.min(1, (e.clientX - box.left) / box.width));
    send("set_volume", { v });
  });

  function toggle() {
    send(state?.phase === "playing" ? "stop" : "resume");
  }

  function shuffle() {
    send("shuffle");
  }

  /** a hidden window must not wake the cpu ten times a second for a panel
   *  nobody is looking at. */
  function sleep() {
    clearInterval(specTimer);
    clearInterval(stateTimer);
    specTimer = null;
    stateTimer = null;
  }

  function wake() {
    if (specTimer) return;
    refresh();
    tickSpectrum();
    specTimer = setInterval(tickSpectrum, SPECTRUM_MS);
    stateTimer = setInterval(refresh, STATE_MS);
  }

  paint();
  paintSpectrum(null);
  invoke("eq_settings")
    .then((eq) => setStyle(eq.style))
    .catch((e) => console.error("eq_settings failed", e));

  return { refresh, toggle, shuffle, sleep, wake, setStyle };
}
