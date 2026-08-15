import { blockedName, flagFor } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

/**
 * the stations that never play, and the only way back.
 *
 * it sits beside the country exclusions rather than in the library: both answer
 * "what never plays", and neither is something you open while choosing what to
 * listen to. a blocked station is not playable, so this list has no cursor and
 * no play — the row's one action is to undo it.
 */
export function mountBlocked(host, onChange) {
  let rows = [];
  let failed = false;

  async function refresh() {
    try {
      rows = await invoke("blocked");
      failed = false;
    } catch (e) {
      // an empty list and a failed read must not look alike: one is an account
      // that has blocked nothing, the other is a bug worth telling about.
      failed = true;
      console.error("blocked failed", e);
    }
    render();
  }

  async function unblock(station) {
    try {
      rows = await invoke("unblock", { uuid: station.uuid });
    } catch (e) {
      console.error("unblock failed", e);
      return;
    }
    render();
    if (onChange) onChange();
  }

  function renderRow(station) {
    const row = el("div", "srow");
    row.appendChild(el("span", "flag", flagFor(station.country)));

    const meta = el("div", "meta");
    meta.appendChild(el("div", "nm", blockedName(station)));
    const codec = [station.codec, station.bitrate ? `${station.bitrate}k` : ""]
      .filter(Boolean)
      .join(" ");
    meta.appendChild(el("div", "sub", codec));
    row.appendChild(meta);

    const act = el("span", "act unblock", "unblock");
    act.title = "Let this station play again";
    act.addEventListener("click", () => unblock(station));
    row.appendChild(act);
    return row;
  }

  function render() {
    host.replaceChildren();

    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "⛌ Blocked stations"));
    head.appendChild(el("span", "cnt", rows.length ? `${rows.length}` : "none"));
    host.appendChild(head);
    host.appendChild(
      el(
        "p",
        "panesub",
        "Stations you blocked never play — not in shuffle, not in search. Unblock one and it comes back everywhere."
      )
    );

    if (failed) {
      const box = el("div", "empty");
      box.appendChild(el("div", "empty_mark err", "⚠"));
      box.appendChild(el("div", "empty_head", "Could not read your blocked stations"));
      box.appendChild(
        el("p", "empty_lede", "The station catalogue did not answer. Reopen the window to try again.")
      );
      host.appendChild(box);
      return;
    }
    if (rows.length === 0) {
      const box = el("div", "empty");
      box.appendChild(el("div", "empty_mark", "⛌"));
      box.appendChild(el("div", "empty_head", "Nothing blocked"));
      box.appendChild(
        el("p", "empty_lede", "Block a station from the menubar panel and it stops turning up, on every device.")
      );
      host.appendChild(box);
      return;
    }

    const list = el("div", "rowlist");
    rows.forEach((station) => list.appendChild(renderRow(station)));
    host.appendChild(list);
  }

  render();
  refresh();

  return { refresh };
}
