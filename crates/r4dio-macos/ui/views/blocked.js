import { flagFor, rowSubtitle, blockedName, blockedHeading } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountBlocked(host, onChange) {
  let rows = [];
  let failed = false;

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
    render();
  }

  function renderRow(station) {
    const row = el("div", "srow");
    row.appendChild(el("span", "flag", flagFor(station.country)));

    const meta = el("div", "meta");
    meta.appendChild(el("div", "nm", blockedName(station)));
    meta.appendChild(el("div", "sub", rowSubtitle(station, false)));
    row.appendChild(meta);

    const act = el("span", "act unblock", "Unblock");
    act.title = "Let this station play again";
    act.addEventListener("click", async () => {
      try {
        rows = await invoke("unblock", { uuid: station.uuid });
        render();
        onChange?.();
      } catch (err) {
        console.error("unblock failed", err);
      }
    });
    row.appendChild(act);
    return row;
  }

  function renderEmpty(root) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark", "⛌"));
    box.appendChild(el("div", "empty_head", "Nothing blocked"));
    box.appendChild(
      el(
        "p",
        "empty_lede",
        "Block a station you never want to hear again and it lands here, synced to every device."
      )
    );
    root.appendChild(box);
  }

  function renderFailed(root) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark err", "⚠"));
    box.appendChild(el("div", "empty_head", "Could not read your blocked stations"));
    box.appendChild(
      el("p", "empty_lede", "The station catalogue did not answer. Reopen the window to try again.")
    );
    root.appendChild(box);
  }

  function render() {
    host.textContent = "";
    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "⛌ Blocked stations"));
    head.appendChild(el("span", "cnt", blockedHeading(rows.length)));
    host.appendChild(head);
    host.appendChild(
      el("p", "panesub", "Stations marked never to play. Unblock one to let it back into shuffle and search.")
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
  }

  render();
  refresh();

  return { refresh };
}
