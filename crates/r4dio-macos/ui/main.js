import { activeSection, filterSummary } from "./labels.js";
import { mountAccount } from "./views/account.js";
import { mountFavourites } from "./views/favourites.js";
import { mountBlocked } from "./views/blocked.js";
import { mountCountries } from "./views/countries.js";
import { mountBrowse } from "./views/browse.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

// a sign-in or a sync can bring a new country filter down, and the sidebar row
// that names it is outside this pane — so it is told, exactly as the two filter
// sections below tell it.
const account = mountAccount(document.getElementById("account_host"), loadFilterSummary);
const favourites = mountFavourites(document.getElementById("pane_favourites"));
// both filter sections change the counts the sidebar shows, so they say when
// they did rather than leaving it stale until the window is reopened.
const blocked = mountBlocked(document.getElementById("pane_blocked"), loadFilterSummary);
const countries = mountCountries(document.getElementById("pane_countries"), loadFilterSummary);
const browse = mountBrowse(document.getElementById("pane_browse"));

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
  sync: () => account.refresh(),
};

let current = null;

function show(id) {
  const section = activeSection(id);
  document.querySelectorAll(".navitem").forEach((item) =>
    item.classList.toggle("active", item.dataset.section === section)
  );
  document.querySelectorAll(".pane").forEach((pane) =>
    pane.classList.toggle("active", pane.dataset.pane === section)
  );
  current = section;
  const refresh = REFRESH[section];
  if (refresh) refresh();
}

async function loadFilterSummary() {
  try {
    const counts = await invoke("filter_counts");
    document.getElementById("filter_summary").textContent =
      filterSummary(counts.excluded, counts.blocked);
    // the backend words this one. empty means no filter to announce, which has
    // to hide the row — an empty "FILTER:" reads like a setting that failed.
    const active = document.getElementById("filter_active");
    active.textContent = counts.filter || "";
    active.classList.toggle("hidden", !counts.filter);
  } catch (e) {
    console.error("filter_counts failed", e);
  }
}

document.querySelectorAll(".navitem").forEach((item) =>
  item.addEventListener("click", () => show(item.dataset.section))
);

// the tray reuses this window rather than recreating it, so the section to open
// on arrives as an event each time it is shown.
listen("show-section", (event) => show(event.payload));

// reopening a hidden window does not reload it; anything changed from the
// menubar panel meanwhile would otherwise still be on screen.
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  loadFilterSummary();
  const refresh = REFRESH[current];
  if (refresh) refresh();
});

show("favourites");
loadFilterSummary();
