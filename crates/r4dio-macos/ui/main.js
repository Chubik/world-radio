import { TABS, targetFor, hintsFor, stateLabels } from "./labels.js";
import { mountAccount } from "./views/account.js";
import { mountFavourites } from "./views/favourites.js";
import { mountBlocked } from "./views/blocked.js";
import { mountCountries } from "./views/countries.js";
import { mountBrowse } from "./views/browse.js";
import { mountShortcuts } from "./views/shortcuts.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const account = mountAccount(document.getElementById("account_host"), () => countries.refresh());
const favourites = mountFavourites(document.getElementById("pane_favourites"));
const blocked = mountBlocked(document.getElementById("pane_blocked"));
const countries = mountCountries(document.getElementById("pane_countries"));
const browse = mountBrowse(document.getElementById("pane_browse"));
mountShortcuts(document.getElementById("pane_shortcuts"));

// the window reads on demand rather than on a timer: favourites only change when
// the user changes them, and sign_in holds the backend mutex across a blocking
// http sync, so a poll would stall the whole window for those seconds.
const REFRESH = {
  favourites: () => favourites.refresh(),
  // browse re-reads only the star marks: a full reload would close every country
  // the user opened and clear the search they typed.
  browse: () => browse.syncStars(),
  blocked: () => blocked.refresh(),
  countries: () => countries.refresh(),
  account: () => account.refresh(),
};

// the sub-view each tab opens on, remembered so returning to a tab lands where
// the user left it rather than resetting to the first sub-tab.
const SUB = { library: "favourites", settings: "countries" };

let tab = null;

function paintTabs() {
  document.querySelectorAll(".tab").forEach((node) =>
    node.classList.toggle("active", node.dataset.tab === tab)
  );
  document.querySelectorAll(".pane").forEach((pane) =>
    pane.classList.toggle("active", pane.dataset.pane === tab)
  );
  const foot = document.getElementById("footerbar");
  foot.replaceChildren();
  for (const [key, desc] of hintsFor(tab)) {
    const line = document.createElement("span");
    const k = document.createElement("span");
    k.className = "k";
    k.textContent = key;
    line.append(k, ` ${desc}`);
    foot.appendChild(line);
  }
}

function paintSubs() {
  for (const [owner, active] of Object.entries(SUB)) {
    const pane = document.querySelector(`.pane[data-pane="${owner}"]`);
    if (!pane) continue;
    pane.querySelectorAll(".subtabs span").forEach((node) =>
      node.classList.toggle("on", node.dataset.sub === active)
    );
    pane.querySelectorAll(".sub").forEach((node) =>
      node.classList.toggle("active", node.dataset.sub === active)
    );
  }
}

/** opens a tab, and the sub-view inside it when one was named. everything the
 *  window shows goes through here, so a tray section and a click cannot end up
 *  painting different things. */
function show(id) {
  const target = targetFor(id);
  tab = target.tab;
  if (target.sub && SUB[tab] !== undefined) {
    SUB[tab] = target.sub;
  }
  paintTabs();
  paintSubs();
  const refresh = REFRESH[SUB[tab] ?? tab];
  if (refresh) refresh();
}

function showSub(owner, sub) {
  SUB[owner] = sub;
  paintSubs();
  const refresh = REFRESH[sub];
  if (refresh) refresh();
}

// ── the now-playing strip ────────────────────────────────────────────────

function bars(host, values) {
  host.replaceChildren();
  for (const v of values) {
    const bar = document.createElement("i");
    bar.style.height = `${Math.max(2, Math.round(v * 100))}%`;
    host.appendChild(bar);
  }
}

async function paintNow() {
  let now;
  try {
    now = await invoke("now_state");
  } catch (e) {
    console.error("now_state failed", e);
    return;
  }
  const idle = !now.station;
  const label = stateLabels(now.phase);

  const name = document.getElementById("now_name");
  name.textContent = now.station ?? "idle";
  name.classList.toggle("idle", idle);
  document.getElementById("now_meta").textContent = now.meta ?? "";

  const clock = document.getElementById("clock");
  clock.replaceChildren();
  const dot = document.createElement("span");
  dot.className = `dot${idle ? " idle" : ""}`;
  dot.textContent = "●";
  clock.append(dot, ` ${label.text}`);

  document.getElementById("big_name").textContent = now.station ?? "idle";
  document.getElementById("big_name").classList.toggle("idle", idle);
  document.getElementById("big_meta").textContent = now.meta ?? "";
  document.getElementById("big_sub").textContent = now.filter ?? "";
  const foot = document.getElementById("big_foot");
  foot.replaceChildren();
  foot.append(now.is_favorite ? "★ saved" : "☆ not saved");

  // the spectrum only reads while something plays; asking for it when idle
  // paints a flat row that looks like a stalled stream rather than silence.
  if (idle) {
    bars(document.getElementById("now_spec"), []);
    bars(document.getElementById("big_spec"), []);
    return;
  }
  try {
    const spec = await invoke("spectrum");
    bars(document.getElementById("now_spec"), spec);
    bars(document.getElementById("big_spec"), spec);
  } catch (e) {
    console.error("spectrum failed", e);
  }
}

// ── keyboard ─────────────────────────────────────────────────────────────

const TAB_KEYS = { 1: "now", 2: "browse", 3: "library", 4: "settings" };

document.addEventListener("keydown", (e) => {
  if (e.metaKey && TAB_KEYS[e.key]) {
    e.preventDefault();
    show(TAB_KEYS[e.key]);
    return;
  }
  if (e.metaKey && e.key.toLowerCase() === "f") {
    e.preventDefault();
    show("browse");
    browse.focusSearch();
    return;
  }
  // a list key typed into the search field is a character, not a command.
  if (e.target instanceof HTMLInputElement) {
    return;
  }
  const list = ACTIVE_LIST();
  if (!list) return;
  const handled = list.onKey(e.key);
  if (handled) e.preventDefault();
});

function ACTIVE_LIST() {
  if (tab === "browse") return browse;
  if (tab === "library" && SUB.library === "favourites") return favourites;
  if (tab === "library" && SUB.library === "blocked") return blocked;
  return null;
}

document.querySelectorAll(".tab").forEach((node) =>
  node.addEventListener("click", () => show(node.dataset.tab))
);
document.querySelectorAll(".subtabs").forEach((bar) =>
  bar.querySelectorAll("span").forEach((node) =>
    node.addEventListener("click", () => {
      const owner = bar.closest(".pane").dataset.pane;
      showSub(owner, node.dataset.sub);
    })
  )
);

// the tray reuses this window rather than recreating it, so the section to open
// on arrives as an event each time it is shown.
listen("show-section", (event) => show(event.payload));

// reopening a hidden window does not reload it; anything changed from the
// menubar panel meanwhile would otherwise still be on screen.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  paintNow();
  const refresh = REFRESH[SUB[tab] ?? tab];
  if (refresh) refresh();
});

show("now");
paintNow();
setInterval(paintNow, 1000);
