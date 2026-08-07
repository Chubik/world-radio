import { stateLabels, volumeSegments, showsStar } from "./labels.js";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

const SPECTRUM_SEED = [5, 7, 4, 8, 6, 3, 7, 5, 8, 4, 6, 7, 3, 5, 6, 4];

function buildSpectrum() {
  const host = $("spectrum");
  SPECTRUM_SEED.forEach((h, i) => {
    const bar = document.createElement("span");
    bar.style.height = `${(h / 8) * 16}px`;
    bar.style.animationDelay = `${(i % 7) * 90}ms`;
    host.appendChild(bar);
  });
}

function buildVolume() {
  const host = $("vol");
  for (let i = 0; i < 6; i++) {
    const seg = document.createElement("i");
    seg.addEventListener("click", () => invoke("set_volume", { v: (i + 1) / 6 }));
    host.appendChild(seg);
  }
}

let lastPhase = null;

function render(s) {
  const labels = stateLabels(s.phase);

  const state = $("state");
  state.className = `state ${labels.tone}${labels.pulse ? " pulse" : ""}`;
  $("state_text").textContent = labels.text;

  $("meta").textContent = s.meta || "";
  $("station").textContent = s.station || "Nothing playing";
  $("station").classList.toggle("idle", s.phase === "idle");

  const star = $("star");
  star.classList.toggle("hidden", !showsStar(s.phase));
  star.classList.toggle("on", s.is_favorite);
  star.textContent = s.is_favorite ? "★" : "☆";

  $("primary").textContent = `⇄ ${labels.primary}`;
  $("play").textContent = s.phase === "playing" ? "⏸" : "▶";

  $("spectrum").classList.toggle("live", s.phase === "playing");

  const filled = volumeSegments(s.volume);
  [...$("vol").children].forEach((seg, i) => seg.classList.toggle("on", i < filled));

  [...$("scope").children].forEach((seg) =>
    seg.classList.toggle("on", seg.dataset.scope === s.scope)
  );

  lastPhase = s.phase;
}

async function poll() {
  try {
    render(await invoke("now_state"));
  } catch (e) {
    console.error("now_state failed", e);
  }
}

function wire() {
  $("primary").addEventListener("click", () => invoke("shuffle").then(poll));
  $("play").addEventListener("click", () => {
    const cmd = lastPhase === "playing" ? "stop" : "resume";
    invoke(cmd).then(poll);
  });
  $("star").addEventListener("click", () => invoke("toggle_favorite").then(poll));
  [...$("scope").children].forEach((seg) =>
    seg.addEventListener("click", () => invoke("set_scope", { scope: seg.dataset.scope }).then(poll))
  );
}

let timer = null;
// a hidden popover must not wake the cpu once a second for a panel nobody sees.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    clearInterval(timer);
    timer = null;
    return;
  }
  poll();
  timer = timer ?? setInterval(poll, 1000);
});

buildSpectrum();
buildVolume();
wire();
poll();
timer = setInterval(poll, 1000);
