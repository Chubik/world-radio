import { stateLabels, showsStar } from "./labels.js";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

let lastPhase = null;

// px per second: slow enough to read a long name, fast enough that the end of
// it arrives before the station changes.
const ROAM_SPEED = 22;

function setStation(name) {
  const box = $("station");
  const text = $("station_text");
  if (text.textContent !== name) {
    text.textContent = name;
  }
  // measuring after the write is what decides between roaming and sitting still;
  // scrollWidth is the laid-out text, clientWidth the room the panel gives it.
  const over = text.scrollWidth - box.clientWidth;
  box.classList.toggle("roams", over > 0);
  if (over > 0) {
    box.style.setProperty("--roam-to", `${-over}px`);
    box.style.setProperty("--roam", `${(over / ROAM_SPEED) * 2 + 4}s`);
  }
}

function render(s) {
  const labels = stateLabels(s.phase);

  const state = $("state");
  state.className = `state ${labels.tone}${labels.pulse ? " pulse" : ""}`;
  $("state_text").textContent = labels.text;

  $("meta").textContent = s.meta || "";
  setStation(s.station || "Nothing playing");
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
  // the build never changes while the app runs, so it is fetched once here
  // rather than on every poll.
  invoke("app_version").then((v) => {
    $("ver").textContent = v;
  });
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
