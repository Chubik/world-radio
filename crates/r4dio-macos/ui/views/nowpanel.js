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
  const track = document.getElementById("np_track");
  const bufrow = document.getElementById("np_bufrow");
  const buf = document.getElementById("np_buf");
  const bufbar = document.getElementById("np_bufbar");
  const uprow = document.getElementById("np_uprow");
  const up = document.getElementById("np_up");
  const upbar = document.getElementById("np_upbar");
  const retryBtn = document.getElementById("np_retry");
  const info = document.getElementById("np_info");
  const mute = document.getElementById("np_mute");
  const meta = document.getElementById("np_meta");
  const status = document.getElementById("np_status");
  const play = document.getElementById("np_play");
  const star = document.getElementById("np_star");
  const shuffleBtn = document.getElementById("np_shuffle");
  const volbar = document.getElementById("np_volbar");
  const vol = document.getElementById("np_vol");
  const scopeLine = document.getElementById("np_scope");
  const clock = document.getElementById("clock");

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
    // many stations send no icy metadata at all, so this is empty far more
    // often than not — and an empty line collapses rather than holding a gap.
    track.textContent = s.track ? `♪ ${s.track}` : "";

    meta.replaceChildren();
    const parts = (s.meta ?? "").split("·").map((p) => p.trim()).filter(Boolean);
    if (s.genre) parts.push(s.genre);
    for (const part of parts) {
      const span = document.createElement("span");
      span.textContent = part;
      meta.appendChild(span);
    }

    // retrying is its own state: the stream is failing, not filling.
    const tone = { playing: "", buffering: "buffering", error: "error" }[s.phase] ?? "idle";
    status.className = `status ${tone}`;
    status.replaceChildren();
    const dot = document.createElement("span");
    dot.className = "dot";
    dot.textContent = "●";
    status.append(dot, s.retries > 0 ? `RETRYING ${s.retries}` : label.text);

    // the buffer is the one number that says a stutter is coming, so it reads
    // green when healthy and red as it drains rather than staying one colour.
    const filled = s.buffer;
    bufrow.classList.toggle("hidden", filled === null || filled === undefined);
    if (filled !== null && filled !== undefined) {
      const pctFull = Math.round(filled * 100);
      buf.textContent = `${pctFull}%`;
      bufbar.style.width = `${pctFull}%`;
      bufbar.style.background = pctFull < 15 ? "var(--err)" : "var(--ok)";
    }

    uprow.classList.toggle("hidden", !s.uptime && s.uptime !== 0);
    if (s.uptime || s.uptime === 0) {
      up.textContent = uptimeLabel(s.uptime);
      // an hour fills the bar; past that it simply stays full, because the
      // point is "this has held", not a precise fraction of nothing.
      upbar.style.width = `${Math.min(100, (s.uptime / 3600) * 100)}%`;
    }

    play.textContent = s.phase === "playing" ? "⏸" : "▶";
    const starIcon = star.querySelector(".ic");
    starIcon.textContent = s.is_favorite ? "★" : "☆";
    starIcon.classList.toggle("on", !!s.is_favorite);
    // nothing playing is nothing to star or retry, and a button that silently
    // does nothing is worse than one that looks unavailable.
    star.classList.toggle("off", idle);
    retryBtn.classList.toggle("off", idle);
    info.classList.toggle("off", idle);

    const level = Math.round((s.volume ?? 0) * 100);
    volbar.firstElementChild.style.width = `${level}%`;
    vol.textContent = `${level}`;
    mute.classList.toggle("muted", !!s.muted);
    mute.textContent = s.muted ? "╳" : "▮▮";

    // the header line: the state, and the wall clock the design puts beside it.
    clock.replaceChildren();
    const cdot = document.createElement("span");
    cdot.className = `dot${idle ? " idle" : ""}${s.phase === "error" ? " err" : ""}`;
    cdot.textContent = "●";
    clock.append(cdot, ` ${s.retries > 0 ? "RETRYING" : label.text} · ${wallClock()}`);

    scopeLine.replaceChildren();
    if (s.next) {
      const label = document.createElement("span");
      label.className = "lbl";
      label.textContent = "NEXT";
      const b = document.createElement("b");
      b.textContent = s.next;
      scopeLine.append(label, " ", b, " · queued via shuffle");
      scopeLine.appendChild(document.createElement("br"));
    }
    const scope = s.scope === "favorites" ? "★ favourites" : "all stations";
    const from = document.createElement("b");
    from.textContent = scope;
    scopeLine.append("shuffle draws from ", from);
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
  retryBtn.addEventListener("click", () => {
    if (!state?.station) return;
    send("retry");
  });
  mute.addEventListener("click", () => send("toggle_mute"));
  info.addEventListener("click", () => {
    if (!state?.station) return;
    showInfo();
  });
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

  /** the stream's own details, in place rather than in a dialog: a modal over a
   *  radio is a thing to dismiss, and the url is the only fact worth showing. */
  function showInfo() {
    const line = [state.meta, state.url].filter(Boolean).join(" · ");
    track.textContent = line;
    // it replaces the track line for a few seconds, then the next poll paints
    // whatever is actually playing back over it.
    setTimeout(() => paint(), 6000);
  }

  function wallClock() {
    const now = new Date();
    const pad = (n) => String(n).padStart(2, "0");
    return `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
  }

  function uptimeLabel(seconds) {
    if (seconds < 60) return `${seconds}s`;
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    return `${hours}h${minutes % 60 ? ` ${minutes % 60}m` : ""}`;
  }

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

  return { refresh, toggle, shuffle, sleep, wake, setStyle, mute: () => send("toggle_mute"), retry: () => send("retry") };
}
