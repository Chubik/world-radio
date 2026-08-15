import { targetFor, hintsFor, stateLabels } from "./labels.js";
import { mountAccount } from "./views/account.js";
import { mountCountries } from "./views/countries.js";
import { mountShortcuts } from "./views/shortcuts.js";
import { mountLibrary } from "./views/library.js";
import { mountNowPanel } from "./views/nowpanel.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const countries = mountCountries(document.getElementById("pane_countries"));
const account = mountAccount(document.getElementById("account_host"), () => countries.refresh());
mountShortcuts(document.getElementById("pane_shortcuts"));

// the two halves of the library are separate because they answer separate
// questions — "what is this" and "what next" — and only the list reloads when
// the user types. they meet here: playing a row repaints the panel at once
// rather than waiting for the next poll.
const now = mountNowPanel({ onChanged: () => library.markPlaying() });
const library = mountLibrary(document.getElementById("listbody"), {
  head: document.getElementById("listhead"),
  count: document.getElementById("count"),
  segrow: document.getElementById("segrow"),
  statebar: document.getElementById("statebar"),
  search: document.getElementById("search"),
  onPlayed: () => now.refresh(),
});

const REFRESH = { countries: () => countries.refresh(), account: () => account.refresh() };
const SUB = { settings: "countries" };

let tab = null;

function paint() {
  document.querySelectorAll(".tab").forEach((node) =>
    node.classList.toggle("active", node.dataset.tab === tab)
  );
  document.querySelectorAll(".pane").forEach((pane) =>
    pane.classList.toggle("active", pane.dataset.pane === tab)
  );
  const pane = document.querySelector('.pane[data-pane="settings"]');
  pane.querySelectorAll(".subtabs span").forEach((node) =>
    node.classList.toggle("on", node.dataset.sub === SUB.settings)
  );
  pane.querySelectorAll(".sub").forEach((node) =>
    node.classList.toggle("active", node.dataset.sub === SUB.settings)
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

/** everything that opens a view goes through here, so a tray section and a
 *  click cannot end up painting different things. */
function show(id) {
  const target = targetFor(id);
  tab = target.tab;
  if (tab === "settings" && target.sub) {
    SUB.settings = target.sub;
  }
  if (tab === "library" && target.sub) {
    library.showSegment(target.sub);
  }
  paint();
  const refresh = REFRESH[SUB.settings];
  if (tab === "settings" && refresh) refresh();
}

function showSub(sub) {
  SUB.settings = sub;
  paint();
  const refresh = REFRESH[sub];
  if (refresh) refresh();
}

// ── keyboard ─────────────────────────────────────────────────────────────

const TAB_KEYS = { 1: "library", 2: "settings" };

document.addEventListener("keydown", (e) => {
  if (e.metaKey && TAB_KEYS[e.key]) {
    e.preventDefault();
    show(TAB_KEYS[e.key]);
    return;
  }
  if (e.metaKey && e.key.toLowerCase() === "f") {
    e.preventDefault();
    show("library");
    library.focusSearch();
    return;
  }
  if (e.metaKey || e.ctrlKey || e.altKey) {
    return;
  }
  // a key typed into the search field is a character, not a command — except
  // escape, which is how the user gets back out of it.
  if (e.target instanceof HTMLInputElement) {
    if (e.key === "Escape") {
      e.target.blur();
    }
    return;
  }
  // space and shuffle work from either tab: they are about what is playing,
  // which the window always shows.
  if (e.key === " ") {
    e.preventDefault();
    now.toggle();
    return;
  }
  if (e.key === "r" || e.key === "R") {
    e.preventDefault();
    now.shuffle();
    return;
  }
  if (tab !== "library") {
    return;
  }
  if (library.onKey(e.key)) {
    e.preventDefault();
  }
});

document.querySelectorAll(".tab").forEach((node) =>
  node.addEventListener("click", () => show(node.dataset.tab))
);
document.querySelectorAll('.pane[data-pane="settings"] .subtabs span').forEach((node) =>
  node.addEventListener("click", () => showSub(node.dataset.sub))
);

// the tray reuses this window rather than recreating it, so the section to open
// on arrives as an event each time it is shown.
listen("show-section", (event) => show(event.payload));

// reopening a hidden window does not reload it; anything changed from the
// menubar panel meanwhile would otherwise still be on screen.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    now.sleep();
    return;
  }
  now.wake();
  library.refreshMarks();
  const refresh = REFRESH[SUB.settings];
  if (tab === "settings" && refresh) refresh();
});

show("library");
now.wake();
