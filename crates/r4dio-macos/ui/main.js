import { activeSection, filterSummary } from "./labels.js";
import { mountAccount } from "./views/account.js";
import { mountFavourites } from "./views/favourites.js";

const invoke = window.__TAURI__.core.invoke;
const listen = window.__TAURI__.event.listen;

const account = mountAccount(document.getElementById("account_host"));
const favourites = mountFavourites(document.getElementById("pane_favourites"));

// the window reads on demand rather than on a timer: favourites only change when
// the user changes them, and sign_in holds the backend mutex across a blocking
// http sync, so a poll would stall the whole window for those seconds.
const REFRESH = {
  favourites: () => favourites.refresh(),
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
