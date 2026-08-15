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
export function mountLibrary(host, { head, count, segrow, statebar, search, onPlayed }) {
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
      } else if (isSearchable(term)) {
        const page = await invoke("search", { name: term.trim() });
        rows = page.stations ?? [];
        capped = !!page.capped;
      } else {
        rows = [];
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
    paint();
  }

  function clampCursor() {
    at = rows.length === 0 ? 0 : Math.min(Math.max(at, 0), rows.length - 1);
  }

  function isFavourite(row) {
    return favourites.has(row.uuid) || !!row.is_favorite;
  }

  // ── painting ──────────────────────────────────────────────────────────────

  function paintChrome() {
    segrow.querySelectorAll("span").forEach((node) =>
      node.classList.toggle("on", node.dataset.seg === segment)
    );

    count.textContent = (() => {
      if (loading || failed) return "";
      if (segment === "all" && !isSearchable(term)) return "";
      return resultHeading(rows.length, capped);
    })();

    // every filter in one row, active and inactive alike: the design's rule is
    // that nothing narrowing the list may hide behind a panel.
    statebar.replaceChildren();
    const chips = [];
    if (segment === "favourites") chips.push(["★ favourites only", true]);
    if (segment === "history") chips.push(["recently played", true]);
    if (segment === "all") {
      chips.push([
        isSearchable(term) ? `search: ${term.trim()}` : "every station",
        isSearchable(term),
      ]);
    }
    for (const [text, on] of chips) {
      const chip = el("span", `chip ${on ? "on" : "off"}`, text);
      if (on && segment === "all") {
        chip.appendChild(el("span", "x", "✕"));
        chip.addEventListener("click", () => {
          term = "";
          search.value = "";
          load();
        });
      }
      statebar.appendChild(chip);
    }
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

  function codecLabel(row) {
    const codec = (row.codec ?? "").trim();
    const rate = Number(row.bitrate) || 0;
    if (!codec && !rate) return "";
    return rate ? `${codec} ${rate}k`.trim() : codec;
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

    node.appendChild(el("span", "nm", row.name));
    node.appendChild(el("span", "flag", `${flagFor(row.country)} ${row.country ?? ""}`.trim()));
    node.appendChild(el("span", "cod", codecLabel(row)));
    if (segment === "history") {
      node.appendChild(el("span", "when", playedWhen(row.played_at, Date.now() / 1000)));
    } else {
      node.appendChild(signalCell(row));
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
    if (isSearchable(term)) {
      return "nothing found. try a shorter word.";
    }
    return "type to search every station by name, or press r to shuffle one.";
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

  /** re-reads only what a play changes: which row is live, and the stars. a
   *  full reload would clear a typed search and lose the cursor. */
  async function markPlaying() {
    if (segment === "all" && !isSearchable(term)) {
      return;
    }
    let live = null;
    try {
      live = await invoke("now_state");
    } catch (e) {
      console.error("now_state failed", e);
      return;
    }
    rows = rows.map((row) => ({ ...row, is_playing: row.name === live.station }));
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

  function showSegment(next) {
    if (segment === next) return;
    segment = next;
    at = 0;
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
    return false;
  }

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
