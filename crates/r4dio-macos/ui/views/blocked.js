import { blockedName } from "../labels.js";
import { el, headRow, stationRow, cursor } from "./stationlist.js";

const invoke = window.__TAURI__.core.invoke;

export function mountBlocked(host, onChange) {
  let rows = [];
  let failed = false;
  const at = cursor();

  async function refresh() {
    try {
      rows = await invoke("blocked");
      failed = false;
    } catch (e) {
      // an empty list and a failed read must not look alike: one is an account
      // that has blocked nothing, the other is a bug the user should be told about.
      failed = true;
      console.error("blocked failed", e);
    }
    at.clamp(rows.length);
    render();
  }

  async function unblock(station) {
    try {
      rows = await invoke("unblock", { uuid: station.uuid });
    } catch (e) {
      console.error("unblock failed", e);
      return;
    }
    at.clamp(rows.length);
    render();
    // the count the settings pane shows is derived from this list, so it is told
    // rather than left stale until the window is reopened.
    if (onChange) onChange();
  }

  function render() {
    host.replaceChildren();
    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "err", "⚠ could not read your blocked stations"));
      box.appendChild(el("div", null, "the station catalogue did not answer. reopen the window to try again."));
      host.appendChild(box);
      return;
    }
    if (rows.length === 0) {
      const box = el("div", "empty");
      box.appendChild(el("div", null, "⛌ nothing blocked"));
      box.appendChild(
        el("div", null, "block a station from the menubar panel and it stops turning up in shuffle, on every device.")
      );
      host.appendChild(box);
      return;
    }

    host.appendChild(
      el("div", "sectlbl", `⛌ ${rows.length} BLOCKED`)
    );
    host.appendChild(headRow("ACTION"));
    const list = el("div", "list");
    rows.forEach((station, i) => {
      // a blocked station is not playable from here — the row's action is the
      // only thing it does, so the click that would play elsewhere unblocks.
      const named = { ...station, name: blockedName(station) };
      list.appendChild(
        stationRow(named, {
          selected: i === at.value,
          action: { label: "↵ unblock", title: "Let this station play again" },
          onPlay: () => {
            at.set(i);
            render();
          },
          onAction: () => unblock(station),
        })
      );
    });
    host.appendChild(list);
  }

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
    if (key === "Enter" || key === "b" || key === "B") {
      unblock(rows[at.value]);
      return true;
    }
    return false;
  }

  render();
  refresh();

  return { refresh, onKey };
}
