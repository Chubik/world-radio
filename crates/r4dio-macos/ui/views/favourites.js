import { flagFor, rowSubtitle, favouritesHeading } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountFavourites(host) {
  let rows = [];
  let failed = false;

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
    render();
  }

  function renderRow(station) {
    const row = el("div", `srow${station.is_playing ? " live" : ""}`);
    row.appendChild(el("span", "flag", flagFor(station.country)));

    const meta = el("div", "meta");
    meta.appendChild(el("div", "nm", station.name));
    meta.appendChild(
      el("div", `sub${station.is_playing ? " on" : ""}`, rowSubtitle(station, station.is_playing))
    );
    row.appendChild(meta);

    const act = el("span", "act", "★");
    act.title = "Remove from favorites";
    act.addEventListener("click", async (e) => {
      // the row itself plays; without this the star would play the station it
      // is in the middle of removing.
      e.stopPropagation();
      try {
        rows = await invoke("remove_favourite", { uuid: station.uuid });
        render();
      } catch (err) {
        console.error("remove_favourite failed", err);
      }
    });
    row.appendChild(act);

    row.addEventListener("click", async () => {
      try {
        await invoke("play_uuid", { uuid: station.uuid });
        await refresh();
      } catch (err) {
        console.error("play_uuid failed", err);
      }
    });
    return row;
  }

  function renderEmpty(root) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark", "★"));
    box.appendChild(el("div", "empty_head", "No favorites yet"));
    box.appendChild(
      el(
        "p",
        "empty_lede",
        "Star a station while it plays — from the menubar panel or from Browse — and it lands here, synced to every device."
      )
    );
    root.appendChild(box);
  }

  function renderFailed(root) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark err", "⚠"));
    box.appendChild(el("div", "empty_head", "Could not read your favorites"));
    box.appendChild(el("p", "empty_lede", "The station catalogue did not answer. Reopen the window to try again."));
    root.appendChild(box);
  }

  function render() {
    host.textContent = "";
    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "★ Favorites"));
    head.appendChild(el("span", "cnt", favouritesHeading(rows.length)));
    host.appendChild(head);
    host.appendChild(
      el("p", "panesub", "Stations synced with your account. Click a row to play it, ★ to remove it.")
    );

    if (failed) {
      renderFailed(host);
      return;
    }
    if (rows.length === 0) {
      renderEmpty(host);
      return;
    }

    const list = el("div", "rowlist");
    rows.forEach((s) => list.appendChild(renderRow(s)));
    host.appendChild(list);

    const shuffle = el("button", "footbtn", "⇄ Shuffle favorites");
    shuffle.addEventListener("click", async () => {
      try {
        await invoke("shuffle_favourites");
        await refresh();
      } catch (e) {
        console.error("shuffle_favourites failed", e);
      }
    });
    host.appendChild(shuffle);
  }

  render();
  refresh();

  return { refresh };
}
