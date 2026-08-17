import {
  flagFor, signalBars, isSearchable, resultHeading, stationCount, playedWhen,
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

/**
 * one list, three views of it.
 *
 * the segment decides where rows come from — the catalogue, the favourites, the
 * history — and nothing else about the layout, so switching one never moves
 * anything the eye is already resting on.
 */
export function mountLibrary(host, { head, count, segrow, statebar, search, onPlayed, onScope, onBlocked }) {
  let segment = "all";
  let rows = [];
  let favourites = new Set();
  let term = "";
  let at = 0;
  let failed = false;
  let capped = false;
  let loading = false;
  let timer = null;
  // the reply to an earlier keystroke can land after a later one; only the
  // newest query may paint, or a slow search overwrites a fresher list.
  let queryId = 0;
  // the phase of whatever is playing, so the row for it can say "buffering"
  // without every row polling the backend for itself.
  let playingPhase = "idle";
  // the chips above the list. null means "Any", which the design draws dimmed
  // rather than hiding — the rule is that nothing narrowing the list is unseen.
  const filters = { genre: null, codec: null, bitrateMin: null };
  // how the whole catalogue is ordered, not just the page on screen — the sort
  // travels to sqlite with the query.
  const SORTS = [
    ["name", "A-Z"],
    ["popular", "popular"],
    ["bitrate", "quality"],
    ["country", "country"],
  ];
  let sort = "name";
  // rows already fetched beyond the first page, and whether another page is
  // worth asking for. paging exists because "All" is 50,000 stations: drawing
  // them at once is a wall, and stopping at 200 hides the rest for good.
  let more = false;
  let loadingMore = false;

  async function loadFavouriteIds() {
    try {
      favourites = new Set(await invoke("favourite_ids"));
    } catch (e) {
      console.error("favourite_ids failed", e);
    }
  }

  async function load() {
    const mine = ++queryId;
    loading = true;
    failed = false;
    capped = false;
    paint();
    try {
      if (segment === "favourites") {
        rows = await invoke("favourites");
      } else if (segment === "history") {
        rows = await invoke("history");
      } else {
        // an empty search is not an empty list: "All" means the catalogue, and
        // it opens showing it. the backend caps a page at 200 rows, so asking
        // for everything costs the same as asking for one word.
        const page = await invoke("search", {
          name: term.trim(),
          genre: filters.genre,
          country: null,
          codec: filters.codec,
          bitrateMin: filters.bitrateMin,
          sort,
        });
        rows = page.stations ?? [];
        capped = !!page.capped;
        more = !!page.capped;
      }
    } catch (e) {
      console.error(`load ${segment} failed`, e);
      if (mine !== queryId) return;
      rows = [];
      failed = true;
    }
    if (mine !== queryId) return;
    loading = false;
    await loadFavouriteIds();
    clampCursor();
    cursorToPlaying();
    paint();
  }

  function clampCursor() {
    at = rows.length === 0 ? 0 : Math.min(Math.max(at, 0), rows.length - 1);
  }

  /** puts the cursor on the station that is playing, when this list holds it.
   *  the row you are looking at and the one you are hearing should be the same
   *  row, so ↓ moves on from what is on air rather than from wherever the
   *  cursor happened to be left. */
  function cursorToPlaying() {
    const live = rows.findIndex((row) => row.is_playing);
    if (live >= 0) {
      at = live;
    }
  }

  function hasFilter() {
    return Object.values(filters).some((v) => v !== null);
  }

  function isFavourite(row) {
    return favourites.has(row.uuid) || !!row.is_favorite;
  }

  // ── painting ──────────────────────────────────────────────────────────────

  function paintChrome() {
    host.addEventListener("scroll", () => {
    const left = host.scrollHeight - host.scrollTop - host.clientHeight;
    // one screen of slack, so the next page is already in by the time the user
    // reaches the bottom rather than after they stop and wait.
    if (left < host.clientHeight) {
      loadMore();
    }
  });

  segrow.querySelectorAll("span").forEach((node) =>
      node.classList.toggle("on", node.dataset.seg === segment)
    );

    count.textContent = loading || failed ? "" : resultHeading(rows.length, capped);

    // every filter in one row, active and inactive alike: the design's rule is
    // that nothing narrowing the list may hide behind a panel.
    statebar.replaceChildren();
    if (segment !== "all") {
      statebar.appendChild(
        el("span", "chip on", segment === "favourites" ? "★ favourites only" : "recently played")
      );
      return;
    }

    statebar.appendChild(el("span", "lbl", "FILTERS"));

    // a chip is either a value the user set, which can be cleared, or the word
    // "Any" dimmed — so the row reads the same whether or not anything is on.
    const chip = (label, value, clear, choose) => {
      const on = value !== null;
      const node = el("span", `chip ${on ? "on" : "off"}`, on ? `${label}: ${value}` : `${label}: Any`);
      node.addEventListener("click", () => (on ? clear() : choose()));
      if (on) node.appendChild(el("span", "x", "✕"));
      statebar.appendChild(node);
    };

    chip("Genre", filters.genre, () => setFilter("genre", null), () => askGenre());
    chip(
      "Min",
      filters.bitrateMin ? `${filters.bitrateMin}k` : null,
      () => setFilter("bitrateMin", null),
      () => setFilter("bitrateMin", 128)
    );
    chip("Codec", filters.codec, () => setFilter("codec", null), () => setFilter("codec", "MP3"));

    // sort is not a filter — it narrows nothing — but it belongs on the same row
    // because it is the other thing that decides what the list shows first.
    const label = SORTS.find(([key]) => key === sort)?.[1] ?? "A-Z";
    const sorter = el("span", `chip${sort === "name" ? " off" : " on"}`, `Sort: ${label}`);
    sorter.title = "Cycle how the list is ordered";
    sorter.addEventListener("click", () => {
      const i = SORTS.findIndex(([key]) => key === sort);
      sort = SORTS[(i + 1) % SORTS.length][0];
      at = 0;
      load();
    });
    statebar.appendChild(sorter);

    if (hasFilter() || isSearchable(term)) {
      const clear = el("span", "clearall", "Clear all");
      clear.addEventListener("click", () => {
        filters.genre = null;
        filters.codec = null;
        filters.bitrateMin = null;
        term = "";
        search.value = "";
        load();
      });
      statebar.appendChild(clear);
    }
  }

  function setFilter(key, value) {
    filters[key] = value;
    at = 0;
    load();
  }

  /** the genre chip has no fixed set to cycle: radio-browser carries thousands
   *  of tags. the search box doubles as the way to name one, so pressing it
   *  takes whatever is typed there. */
  function askGenre() {
    const typed = search.value.trim();
    if (!typed) {
      search.focus();
      return;
    }
    search.value = "";
    term = "";
    setFilter("genre", typed.toLowerCase());
  }

  function paintHead() {
    head.replaceChildren();
    head.appendChild(el("span", "car", ""));
    head.appendChild(el("span", "star", ""));
    head.appendChild(el("span", "nm", "STATION"));
    head.appendChild(el("span", "flag", "CC"));
    head.appendChild(el("span", "cod", "CODEC"));
    if (segment === "history") {
      head.appendChild(el("span", "when", "WHEN"));
      return;
    }
    head.appendChild(el("span", "sig", "SIGNAL"));
  }

  function signalCell(row) {
    const cell = el("span", "sig");
    const on = signalBars(row.bitrate);
    if (on === 0) {
      cell.appendChild(el("span", "off", "—"));
      return cell;
    }
    cell.appendChild(el("span", "on", "●".repeat(on)));
    if (on < 5) cell.appendChild(el("span", "off", "○".repeat(5 - on)));
    return cell;
  }

  /** what the SIGNAL column shows. a station that keeps failing, or the one
   *  buffering right now, says so instead of showing a quality reading — the
   *  meter answers "how good is this stream", these answer "is there one". */
  function stateCell(row) {
    if (row.dead) {
      return el("span", "sig dead", "✗ dead");
    }
    if (row.is_playing && playingPhase === "buffering") {
      return el("span", "sig buffering", "⏳ buffering");
    }
    return signalCell(row);
  }

  function codecLabel(row) {
    const codec = (row.codec ?? "").trim();
    const rate = Number(row.bitrate) || 0;
    if (!codec && !rate) return "";
    // a handful of stations report bits per second rather than kilobits, so the
    // catalogue holds 512000 where every other row holds 512. printed raw it
    // reads "512000k", which looks like a broken number rather than a good one.
    const kbps = rate >= 10000 ? Math.round(rate / 1000) : rate;
    return kbps ? `${codec} ${kbps}k`.trim() : codec;
  }

  function rowNode(row, i) {
    const node = el("div", `row${i === at ? " sel" : ""}${row.is_playing ? " playing" : ""}`);
    node.appendChild(el("span", "car", i === at ? "▸" : ""));

    const starred = isFavourite(row);
    const star = el("span", `star${starred ? " on" : ""}`, starred ? "★" : "☆");
    star.title = starred ? "Remove from favourites" : "Add to favourites";
    star.addEventListener("click", (e) => {
      // the row itself plays; without this the star would also start the
      // station it is in the middle of saving.
      e.stopPropagation();
      toggleFavourite(row);
    });
    node.appendChild(star);

    const nm = el("span", "nm", row.name);
    if (row.genre) nm.appendChild(el("span", "genre", row.genre));
    node.appendChild(nm);
    node.appendChild(el("span", "flag", `${flagFor(row.country)} ${row.country ?? ""}`.trim()));
    node.appendChild(el("span", "cod", codecLabel(row)));
    if (segment === "history") {
      node.appendChild(el("span", "when", playedWhen(row.played_at, Date.now() / 1000)));
    } else {
      node.appendChild(stateCell(row));
    }

    node.addEventListener("click", () => {
      at = i;
      play(row);
    });
    return node;
  }

  function paint() {
    paintChrome();
    paintHead();
    host.replaceChildren();

    if (loading) {
      host.appendChild(el("div", "loading", "loading…"));
      return;
    }
    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "err", "⚠ could not read the catalogue"));
      box.appendChild(el("div", null, "it did not answer. try again in a moment."));
      host.appendChild(box);
      return;
    }
    if (rows.length === 0) {
      host.appendChild(el("div", "empty", emptyLine()));
      return;
    }
    rows.forEach((row, i) => host.appendChild(rowNode(row, i)));
    scrollCursorIntoView();
  }

  function emptyLine() {
    if (segment === "favourites") {
      return "no favourites yet. press F on a station and it lands here, synced to every device.";
    }
    if (segment === "history") {
      return "nothing played yet. every station you listen to shows up here.";
    }
    if (isSearchable(term) || hasFilter()) {
      return "nothing found. try a shorter word, or clear a filter.";
    }
    return "the catalogue has not been downloaded yet.";
  }

  /** fetches the next page when the list is scrolled near its end. it appends
   *  rather than replacing, so the cursor and everything above stay put. */
  async function loadMore() {
    if (!more || loadingMore || segment !== "all") {
      return;
    }
    loadingMore = true;
    try {
      const page = await invoke("search", {
        name: term.trim(),
        genre: filters.genre,
        country: null,
        codec: filters.codec,
        bitrateMin: filters.bitrateMin,
        sort,
        offset: rows.length,
      });
      const next = page.stations ?? [];
      // a page that repeats what we already hold means the end: appending it
      // would grow the list forever with the same rows.
      const known = new Set(rows.map((r) => r.uuid));
      const fresh = next.filter((r) => !known.has(r.uuid));
      rows = rows.concat(fresh);
      more = !!page.capped && fresh.length > 0;
      capped = more;
      paint();
    } catch (e) {
      console.error("load more failed", e);
      more = false;
    }
    loadingMore = false;
  }

  function scrollCursorIntoView() {
    const node = host.children[at];
    if (node && node.scrollIntoView) {
      node.scrollIntoView({ block: "nearest" });
    }
  }

  // ── actions ───────────────────────────────────────────────────────────────

  async function play(row) {
    try {
      await invoke("play_uuid", { uuid: row.uuid });
    } catch (e) {
      console.error("play_uuid failed", e);
      return;
    }
    if (onPlayed) onPlayed();
    markPlaying();
  }

  /** bans a station outright. the row goes immediately rather than waiting for
   *  a reload: a blocked station reappearing for a second reads as a failed
   *  click. */
  async function block(row) {
    try {
      await invoke("block", { uuid: row.uuid });
    } catch (e) {
      console.error("block failed", e);
      return;
    }
    rows = rows.filter((r) => r.uuid !== row.uuid);
    clampCursor();
    paint();
    if (onBlocked) onBlocked();
  }

  async function toggleFavourite(row) {
    const cmd = isFavourite(row) ? "remove_favourite" : "add_favourite";
    try {
      await invoke(cmd, { uuid: row.uuid });
    } catch (e) {
      console.error(`${cmd} failed`, e);
      return;
    }
    await loadFavouriteIds();
    // the favourites segment is a list of exactly this, so unstarring a row
    // there has to remove it rather than leave a hollow star behind.
    if (segment === "favourites") {
      await load();
      return;
    }
    paint();
  }

  /** re-reads what a play changes: which row is live, and the stars.
   *
   *  when the station that started is not in this list at all — which is what
   *  shuffle does, drawing from tens of thousands — the list is reloaded so the
   *  backend can pin it at the top. marking rows that do not contain it would
   *  leave the cursor sitting on whatever was there before. */
  async function markPlaying() {
    if (rows.length === 0) {
      return;
    }
    let live = null;
    try {
      live = await invoke("now_state");
    } catch (e) {
      console.error("now_state failed", e);
      return;
    }
    playingPhase = live.phase;
    // shuffle draws from tens of thousands, so the station that just started is
    // almost never among the rows on screen. marking rows that do not contain it
    // would leave the cursor on whatever was there before; the catalogue view
    // reloads instead, and the backend pins the station at the top.
    if (live.uuid && !rows.some((row) => row.uuid === live.uuid)) {
      if (segment !== "all") {
        return;
      }
      // put it at the top rather than reloading: a reload would throw away
      // every page the user has scrolled in, and shuffle is pressed far more
      // often than the list is re-read.
      let station = null;
      try {
        const page = await invoke("search", {
          name: "", genre: null, country: null, codec: null, bitrateMin: null, sort, offset: 0,
        });
        station = page.stations?.find((s) => s.uuid === live.uuid) ?? null;
      } catch (e) {
        console.error("could not resolve the playing station", e);
      }
      if (!station) return;
      rows = [station, ...rows.map((row) => ({ ...row, is_playing: false }))];
      at = 0;
      await loadFavouriteIds();
      paint();
      return;
    }
    rows = rows.map((row) => ({ ...row, is_playing: !!live.uuid && row.uuid === live.uuid }));
    // the row the user is looking at and the one they are hearing should be the
    // same row, so ↓ moves on from what is on air.
    cursorToPlaying();
    await loadFavouriteIds();
    paint();
  }

  function refreshMarks() {
    if (segment === "all") {
      markPlaying();
      return;
    }
    load();
  }

  async function showSegment(next) {
    if (segment === next) return;
    segment = next;
    at = 0;
    // the segment is also what shuffle draws from: a user looking at All and
    // pressing shuffle means "surprise me from everything", not "from the
    // favourites I am not looking at". history has no scope of its own, so it
    // leaves shuffle where it was.
    if (next === "all" || next === "favourites") {
      try {
        await invoke("set_scope", { scope: next === "all" ? "all" : "favorites" });
      } catch (e) {
        console.error("set_scope failed", e);
      }
      if (onScope) onScope();
    }
    load();
  }

  function focusSearch() {
    search.focus();
    search.select();
  }

  function onKey(key) {
    if (key === "1") return showSegment("all"), true;
    if (key === "2") return showSegment("favourites"), true;
    if (key === "3") return showSegment("history"), true;
    if (rows.length === 0) return false;
    if (key === "ArrowDown") {
      at = Math.min(at + 1, rows.length - 1);
      paint();
      return true;
    }
    if (key === "ArrowUp") {
      at = Math.max(at - 1, 0);
      paint();
      return true;
    }
    if (key === "Enter") {
      play(rows[at]);
      return true;
    }
    if (key === "f" || key === "F") {
      toggleFavourite(rows[at]);
      return true;
    }
    if (key === "b" || key === "B") {
      block(rows[at]);
      return true;
    }
    return false;
  }

  host.addEventListener("scroll", () => {
    const left = host.scrollHeight - host.scrollTop - host.clientHeight;
    // one screen of slack, so the next page is already in by the time the user
    // reaches the bottom rather than after they stop and wait.
    if (left < host.clientHeight) {
      loadMore();
    }
  });

  segrow.querySelectorAll("span").forEach((node) =>
    node.addEventListener("click", () => showSegment(node.dataset.seg))
  );
  search.addEventListener("input", () => {
    term = search.value;
    // typing is a search of the whole catalogue, which is what "all" means —
    // it must not silently filter the favourites the user is looking at.
    segment = "all";
    if (timer) clearTimeout(timer);
    if (!isSearchable(term)) {
      queryId++;
      rows = [];
      loading = false;
      paint();
      return;
    }
    timer = setTimeout(load, DEBOUNCE_MS);
  });

  load();

  return { showSegment, focusSearch, onKey, markPlaying, refreshMarks };
}
