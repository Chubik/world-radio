import { el, headRow, stationRow, cursor } from "./stationlist.js";

const invoke = window.__TAURI__.core.invoke;

export function mountFavourites(host) {
  let rows = [];
  let failed = false;
  const at = cursor();

  async function refresh() {
    try {
      rows = await invoke("favourites");
      failed = false;
    } catch (e) {
      // an empty list and a failed read must not look alike: one is a normal new
      // account, the other is a bug the user should be told about.
      failed = true;
      console.error("favourites failed", e);
    }
    at.clamp(rows.length);
    render();
  }

  async function play(station) {
    try {
      await invoke("play_uuid", { uuid: station.uuid });
      await refresh();
    } catch (e) {
      console.error("play_uuid failed", e);
    }
  }

  async function remove(station) {
    try {
      rows = await invoke("remove_favourite", { uuid: station.uuid });
    } catch (e) {
      console.error("remove_favourite failed", e);
      return;
    }
    at.clamp(rows.length);
    render();
  }

  function render() {
    host.replaceChildren();
    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "err", "⚠ could not read your favorites"));
      box.appendChild(el("div", null, "the station catalogue did not answer. reopen the window to try again."));
      host.appendChild(box);
      return;
    }
    if (rows.length === 0) {
      const box = el("div", "empty");
      box.appendChild(el("div", null, "★ no favorites yet"));
      box.appendChild(
        el("div", null, "star a station while it plays — from the menubar panel or from browse — and it lands here, synced to every device.")
      );
      host.appendChild(box);
      return;
    }

    // the label carries the noun as well as the count: the pane heading that
    // used to name it is gone, and a bare "4" over a list says nothing.
    host.appendChild(
      el("div", "sectlbl", `★ ${rows.length} FAVOURITE${rows.length === 1 ? "" : "S"}`)
    );
    host.appendChild(headRow("ACTION"));
    const list = el("div", "list");
    rows.forEach((station, i) =>
      list.appendChild(
        stationRow(station, {
          selected: i === at.value,
          action: { label: "↵ unstar", title: "Remove from favorites" },
          onPlay: () => {
            at.set(i);
            play(station);
          },
          onAction: () => remove(station),
        })
      )
    );
    host.appendChild(list);
  }

  /** returns whether the key was ours, so a key this list ignores still reaches
   *  the window (and the menu) instead of being swallowed. */
  function onKey(key) {
    if (rows.length === 0) return false;
    if (key === "ArrowDown") {
      at.move(1, rows.length);
      render();
      return true;
    }
    if (key === "ArrowUp") {
      at.move(-1, rows.length);
      render();
      return true;
    }
    if (key === "Enter") {
      play(rows[at.value]);
      return true;
    }
    if (key === "f" || key === "F") {
      remove(rows[at.value]);
      return true;
    }
    return false;
  }

  render();
  refresh();

  return { refresh, onKey };
}
