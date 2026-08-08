import { stateLabels, showsStar } from "./labels.js";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

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
  // webkit can throttle a hidden window's interval to a standstill without
  // clearing it, so reusing a non-null timer here would leave the panel frozen.
  clearInterval(timer);
  timer = setInterval(poll, 1000);
});

wire();
poll();
timer = setInterval(poll, 1000);
