import {
  flagFor, countryName, browseSubtitle, resultHeading, isSearchable, stationCount,
} from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

// a search over the offline cache costs ~0.29s, so typing "jazz" unthrottled
// would run four of them and paint three lists nobody asked to see.
export const DEBOUNCE_MS = 250;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountBrowse(host) {
  let countries = [];
  // uuid set, kept alongside the rows so starring one row can repaint every row
  // that shows the same station without refetching a 200-row page.
  let favourites = new Set();
  let failed = false;
  let term = "";
  let results = null;
  let timer = null;
  // country code -> { open, page }. `page` stays cached once loaded, so closing
  // and reopening a node costs nothing.
  const nodes = new Map();

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

  async function addFavourite(uuid) {
    try {
      favourites = new Set(await invoke("add_favourite", { uuid }));
    } catch (e) {
      console.error("add_favourite failed", e);
      return;
    }
    // only the subtitles change, so the lists are repainted in place rather than
    // rebuilt — rebuilding would collapse every open country node.
    paintResults();
    paintOpenNodes();
  }

  async function play(uuid) {
    try {
      await invoke("play_uuid", { uuid });
    } catch (e) {
      console.error("play_uuid failed", e);
    }
  }

  function renderRow(station) {
    const row = el("div", `srow${station.is_playing ? " live" : ""}`);
    row.appendChild(el("span", "flag", flagFor(station.country)));

    const meta = el("div", "meta");
    meta.appendChild(el("div", "nm", station.name));
    meta.appendChild(el("div", "sub", browseSubtitle(station, favourites.has(station.uuid))));
    row.appendChild(meta);

    const act = el("span", "act", "☆");
    act.title = "Add to favorites";
    act.addEventListener("click", (e) => {
      // the row itself plays; without this the star would also start the station
      // it is in the middle of adding.
      e.stopPropagation();
      addFavourite(station.uuid);
    });
    row.appendChild(act);

    row.addEventListener("click", () => play(station.uuid));
    return row;
  }

  function renderList(page) {
    const box = el("div");
    const head = el("div", "listhead");
    head.appendChild(el("span", "cnt", resultHeading(page.stations.length, page.capped)));
    box.appendChild(head);
    const list = el("div", "rowlist");
    page.stations.forEach((s) => list.appendChild(renderRow(s)));
    box.appendChild(list);
    return box;
  }

  function renderEmpty(root, head, lede) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark", "⌕"));
    box.appendChild(el("div", "empty_head", head));
    box.appendChild(el("p", "empty_lede", lede));
    root.appendChild(box);
  }

  // ── search ────────────────────────────────────────────────────────────────

  let resultsHost = null;
  let treeHost = null;
  // the reply to an earlier keystroke can land after a later one; only the newest
  // query may paint, or a slow search overwrites a fresher list.
  let queryId = 0;

  async function runSearch() {
    const mine = ++queryId;
    const name = term.trim();
    try {
      const page = await invoke("search", { name });
      if (mine !== queryId) return;
      results = page;
    } catch (e) {
      console.error("search failed", e);
      if (mine !== queryId) return;
      results = { stations: [], capped: false, failed: true };
    }
    paintResults();
  }

  function onTermChanged() {
    if (timer) clearTimeout(timer);
    if (!isSearchable(term)) {
      // a discarded term must also discard whatever a search already in flight
      // is about to answer with.
      queryId++;
      results = null;
      paintResults();
      return;
    }
    timer = setTimeout(runSearch, DEBOUNCE_MS);
  }

  function paintResults() {
    if (!resultsHost) return;
    resultsHost.textContent = "";
    if (results === null) return;
    if (results.failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "empty_mark err", "⚠"));
      box.appendChild(el("div", "empty_head", "Could not search the catalogue"));
      box.appendChild(el("p", "empty_lede", "The station catalogue did not answer. Try the search again."));
      resultsHost.appendChild(box);
      return;
    }
    if (results.stations.length === 0) {
      renderEmpty(
        resultsHost,
        "Nothing found",
        "No station in the catalogue answers to that. Try a shorter word, or browse by country below."
      );
      return;
    }
    resultsHost.appendChild(renderList(results));
  }

  function renderSearch(root) {
    const field = el("div", "searchfield");
    field.appendChild(el("span", "ic", "⌕"));
    const input = el("input");
    input.type = "search";
    input.placeholder = "Search every station by name…";
    input.value = term;
    input.addEventListener("input", () => {
      term = input.value;
      onTermChanged();
    });
    field.appendChild(input);
    root.appendChild(field);
  }

  // ── the country tree ──────────────────────────────────────────────────────

  function nodeFor(code) {
    let node = nodes.get(code);
    if (!node) {
      node = { open: false, page: null };
      nodes.set(code, node);
    }
    return node;
  }

  async function toggleCountry(code, body, caret, wrapper) {
    const node = nodeFor(code);
    node.open = !node.open;
    wrapper.classList.toggle("open", node.open);
    caret.textContent = node.open ? "▾" : "▸";
    body.textContent = "";
    if (!node.open) {
      body.remove();
      return;
    }
    wrapper.appendChild(body);
    // a country is only ever loaded the first time it is opened; 240 countries
    // loaded upfront would be the whole 58k-row catalogue.
    if (node.page === null) {
      body.appendChild(el("div", "loading", "loading…"));
      try {
        node.page = await invoke("stations_in", { country: code });
      } catch (e) {
        console.error("stations_in failed", e);
        node.page = { stations: [], capped: false };
      }
      // the user may have closed it again while it loaded.
      if (!node.open) return;
      body.textContent = "";
    }
    paintNode(code, body);
  }

  function paintNode(code, body) {
    const node = nodeFor(code);
    body.textContent = "";
    if (node.page.stations.length === 0) {
      renderEmpty(body, "No stations here", "This country has no station the filters let through.");
      return;
    }
    body.appendChild(renderList(node.page));
  }

  // a star pressed in one place changes the subtitle everywhere that station is
  // drawn, so every open node is repainted from the page it already has.
  function paintOpenNodes() {
    if (!treeHost) return;
    treeHost.querySelectorAll(".treenode.open").forEach((wrapper) => {
      const body = wrapper.querySelector(".treebody");
      if (body) paintNode(wrapper.dataset.code, body);
    });
  }

  function renderTree(root) {
    root.appendChild(el("div", "glabel", "Browse by country"));
    countries.forEach((country) => {
      const wrapper = el("div", "treenode");
      wrapper.dataset.code = country.code;

      const head = el("div", "treehead");
      const caret = el("span", "car", "▸");
      head.appendChild(caret);
      head.appendChild(el("span", "flag", flagFor(country.code)));
      head.appendChild(el("span", "cname", countryName(country.code)));
      head.appendChild(el("span", "cnt", stationCount(country.count)));

      const body = el("div", "treebody");
      head.addEventListener("click", () => toggleCountry(country.code, body, caret, wrapper));
      wrapper.appendChild(head);
      root.appendChild(wrapper);
    });
  }

  function render() {
    host.textContent = "";
    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "⌕ Browse"));
    head.appendChild(el("span", "cnt", countryHeading()));
    host.appendChild(head);
    host.appendChild(
      el("p", "panesub", "Search the whole catalogue by name, or open a country to see what it carries.")
    );

    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "empty_mark err", "⚠"));
      box.appendChild(el("div", "empty_head", "Could not read the catalogue"));
      box.appendChild(el("p", "empty_lede", "The station catalogue did not answer. Reopen the window to try again."));
      host.appendChild(box);
      return;
    }

    renderSearch(host);
    resultsHost = el("div", "results");
    host.appendChild(resultsHost);
    paintResults();

    if (countries.length === 0) {
      renderEmpty(
        host,
        "No stations yet",
        "The station catalogue has not been downloaded yet. Once it syncs, every country it covers appears here."
      );
      return;
    }

    treeHost = el("div", "tree");
    host.appendChild(treeHost);
    renderTree(treeHost);
    // an open node cannot survive a full repaint, so the cached pages are the
    // only thing carried across; the tree reopens closed.
    nodes.forEach((node) => {
      node.open = false;
    });
  }

  function countryHeading() {
    const total = countries.reduce((sum, c) => sum + c.count, 0);
    return `${stationCount(total)} stations · ${countries.length} countries`;
  }

  // re-entering the section must not throw away a typed search or a country the
  // user left open, so only the star marks are re-read — a star removed from the
  // Favorites section has to stop showing "already in ★" here.
  async function syncStars() {
    try {
      favourites = new Set(await invoke("favourite_ids"));
    } catch (e) {
      console.error("favourite_ids failed", e);
      return;
    }
    paintResults();
    paintOpenNodes();
  }

  render();
  refresh();

  return { refresh, syncStars };
}
