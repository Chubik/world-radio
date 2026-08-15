import {
  flagFor, countryName, resultHeading, isSearchable, stationCount,
} from "../labels.js";
import { el, headRow, stationRow, cursor } from "./stationlist.js";

const invoke = window.__TAURI__.core.invoke;

// a search over the offline cache costs ~0.29s, so typing "jazz" unthrottled
// would run four of them and paint three lists nobody asked to see.
export const DEBOUNCE_MS = 250;

export function mountBrowse(host) {
  let countries = [];
  // uuid set, kept alongside the rows so starring one row can repaint every row
  // that shows the same station without refetching a 200-row page.
  let favourites = new Set();
  let failed = false;
  let term = "";
  let results = null;
  let timer = null;
  // the country whose stations fill the list. null means the list shows search
  // results instead — the two never share it, so the cursor always has one
  // list to walk.
  let country = null;
  let page = null;
  const at = cursor();
  let searchInput = null;
  let listHost = null;
  let filterHost = null;
  // the reply to an earlier keystroke can land after a later one; only the newest
  // query may paint, or a slow search overwrites a fresher list.
  let queryId = 0;

  async function refresh() {
    try {
      const [rows, ids] = await Promise.all([invoke("countries"), invoke("favourite_ids")]);
      countries = rows;
      favourites = new Set(ids);
      failed = false;
    } catch (e) {
      // an empty catalogue and a failed read must not look alike.
      failed = true;
      console.error("browse load failed", e);
    }
    render();
  }

  /** the rows the cursor walks: whichever of the two sources is showing. */
  function rows() {
    if (results !== null) return results.stations ?? [];
    if (page !== null) return page.stations ?? [];
    return [];
  }

  async function play(station) {
    try {
      await invoke("play_uuid", { uuid: station.uuid });
    } catch (e) {
      console.error("play_uuid failed", e);
    }
  }

  async function star(station) {
    try {
      favourites = new Set(await invoke("add_favourite", { uuid: station.uuid }));
    } catch (e) {
      console.error("add_favourite failed", e);
      return;
    }
    paintList();
  }

  // ── search ────────────────────────────────────────────────────────────────

  async function runSearch() {
    const mine = ++queryId;
    const name = term.trim();
    try {
      const found = await invoke("search", { name });
      if (mine !== queryId) return;
      results = found;
    } catch (e) {
      console.error("search failed", e);
      if (mine !== queryId) return;
      results = { stations: [], capped: false, failed: true };
    }
    at.clamp(rows().length);
    paintList();
  }

  function onTermChanged() {
    if (timer) clearTimeout(timer);
    if (!isSearchable(term)) {
      // a discarded term must also discard whatever a search already in flight
      // is about to answer with.
      queryId++;
      results = null;
      at.set(0);
      paintList();
      return;
    }
    // a search takes over the list from whatever country was open, so the
    // country stops being highlighted the moment the term counts.
    country = null;
    page = null;
    timer = setTimeout(runSearch, DEBOUNCE_MS);
  }

  // ── the country filter column ─────────────────────────────────────────────

  async function openCountry(code) {
    country = code;
    // a country replaces the search results rather than filtering them: two
    // lists at once is the modal-and-list shape this layout exists to remove.
    results = null;
    term = "";
    queryId++;
    if (searchInput) searchInput.value = "";
    page = null;
    at.set(0);
    paintFilters();
    paintList();
    try {
      page = await invoke("stations_in", { country: code });
    } catch (e) {
      console.error("stations_in failed", e);
      page = { stations: [], capped: false, failed: true };
    }
    // the user may have typed a search while it loaded.
    if (country !== code) return;
    at.clamp(rows().length);
    paintList();
  }

  function paintFilters() {
    if (!filterHost) return;
    filterHost.replaceChildren();
    const group = el("div", "fgroup");
    group.appendChild(el("div", "flabel", "COUNTRY"));
    countries.forEach((row) => {
      const opt = el("div", `fopt${row.code === country ? " on" : ""}`);
      opt.appendChild(el("span", "car", row.code === country ? "▸" : ""));
      opt.appendChild(el("span", "box", row.code === country ? "[✓]" : "[ ]"));
      opt.appendChild(el("span", null, `${flagFor(row.code)} ${countryName(row.code)}`));
      opt.appendChild(el("span", "cnt", ""));
      opt.title = `${stationCount(row.count)} stations`;
      opt.addEventListener("click", () => openCountry(row.code));
      group.appendChild(opt);
    });
    filterHost.appendChild(group);
  }

  // ── the list ──────────────────────────────────────────────────────────────

  function paintList() {
    if (!listHost) return;
    listHost.replaceChildren();

    const showing = results ?? page;
    const count = el("span", "resultcount");
    if (showing && !showing.failed) {
      count.textContent = resultHeading(rows().length, showing.capped);
    }
    if (searchInput) {
      searchInput.parentElement.querySelector(".resultcount")?.replaceWith(count);
    }

    if (showing === null) {
      listHost.appendChild(
        el("div", "empty", "type to search every station by name, or pick a country on the left.")
      );
      return;
    }
    if (showing.failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "err", "⚠ could not read the catalogue"));
      box.appendChild(el("div", null, "the station catalogue did not answer. try again."));
      listHost.appendChild(box);
      return;
    }
    if (rows().length === 0) {
      listHost.appendChild(
        el("div", "empty", "nothing here. try a shorter word, or another country.")
      );
      return;
    }

    listHost.appendChild(headRow("ACTION"));
    const list = el("div", "list");
    rows().forEach((station, i) =>
      list.appendChild(
        stationRow(station, {
          selected: i === at.value,
          action: {
            label: favourites.has(station.uuid) ? "★ saved" : "↵ star",
            title: "Add to favorites",
          },
          onPlay: () => {
            at.set(i);
            play(station);
            paintList();
          },
          onAction: () => star(station),
        })
      )
    );
    listHost.appendChild(list);
  }

  function render() {
    host.replaceChildren();
    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "err", "⚠ could not read the catalogue"));
      box.appendChild(el("div", null, "the station catalogue did not answer. reopen the window to try again."));
      host.appendChild(box);
      return;
    }

    const grid = el("div", "browsegrid");
    filterHost = el("div");
    const right = el("div");

    const search = el("div", "search");
    search.appendChild(el("span", "cursor", "▌"));
    searchInput = el("input");
    searchInput.type = "search";
    searchInput.placeholder = "search every station by name…";
    searchInput.value = term;
    searchInput.addEventListener("input", () => {
      term = searchInput.value;
      onTermChanged();
    });
    search.appendChild(searchInput);
    search.appendChild(el("span", "resultcount"));
    right.appendChild(search);
    right.appendChild(el("hr", "rule"));
    listHost = el("div");
    right.appendChild(listHost);

    grid.append(filterHost, right);
    host.appendChild(grid);
    paintFilters();
    paintList();
  }

  function focusSearch() {
    if (searchInput) searchInput.focus();
  }

  function onKey(key) {
    const list = rows();
    if (list.length === 0) return false;
    if (key === "ArrowDown") {
      at.move(1, list.length);
      paintList();
      return true;
    }
    if (key === "ArrowUp") {
      at.move(-1, list.length);
      paintList();
      return true;
    }
    if (key === "Enter") {
      play(list[at.value]);
      return true;
    }
    if (key === "f" || key === "F") {
      star(list[at.value]);
      return true;
    }
    return false;
  }

  // re-entering the tab must not throw away a typed search or the country the
  // user opened, so only the star marks are re-read — a star removed in Library
  // has to stop showing "★ saved" here.
  async function syncStars() {
    try {
      favourites = new Set(await invoke("favourite_ids"));
    } catch (e) {
      console.error("favourite_ids failed", e);
      return;
    }
    paintList();
  }

  render();
  refresh();

  return { refresh, syncStars, focusSearch, onKey };
}
