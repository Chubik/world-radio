import { flagFor, countryName, countryHeading, matchesCountry } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountCountries(host, onChange) {
  let rows = [];
  let term = "";
  let failed = false;

  async function refresh() {
    try {
      rows = await invoke("countries");
      failed = false;
    } catch (e) {
      // an empty catalogue and a failed read must not look alike.
      failed = true;
      console.error("countries failed", e);
    }
    render();
  }

  // the whole set is sent, never the filtered view: a country hidden by the
  // search box is still excluded, and posting only what is on screen would
  // silently switch it back on.
  async function save() {
    const codes = rows.filter((r) => r.excluded).map((r) => r.code);
    try {
      rows = await invoke("set_excluded", { codes });
      failed = false;
    } catch (e) {
      console.error("set_excluded failed", e);
      // the backend is the authority on what is stored; re-read rather than
      // leaving the window showing a toggle that never took.
      await refresh();
      return;
    }
    render();
    onChange?.();
  }

  function renderRow(country) {
    const row = el("div", "srow");
    row.appendChild(el("span", "flag", flagFor(country.code)));

    const meta = el("div", "meta");
    meta.appendChild(el("div", "nm", countryName(country.code)));
    meta.appendChild(el("div", "sub", `${country.count.toLocaleString("en")} stations`));
    row.appendChild(meta);

    // the switch reads as "this country plays", which is the way round a switch
    // is understood: on means on air. what is stored is still the *exclusions*
    // — the account, the tui and the phone all speak that list — so only the
    // display is inverted, never the data.
    const plays = !country.excluded;
    const toggle = el("div", `toggle${plays ? " on" : ""}`);
    toggle.appendChild(el("i"));
    toggle.setAttribute("role", "switch");
    toggle.setAttribute("aria-checked", String(plays));
    toggle.setAttribute("aria-label", `Play stations from ${countryName(country.code)}`);
    row.appendChild(toggle);

    row.addEventListener("click", () => {
      country.excluded = !country.excluded;
      save();
    });
    return row;
  }

  function renderEmpty(root, head, lede) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark", "◔"));
    box.appendChild(el("div", "empty_head", head));
    box.appendChild(el("p", "empty_lede", lede));
    root.appendChild(box);
  }

  function renderFailed(root) {
    const box = el("div", "empty");
    box.appendChild(el("div", "empty_mark err", "⚠"));
    box.appendChild(el("div", "empty_head", "Could not read the country list"));
    box.appendChild(
      el("p", "empty_lede", "The station catalogue did not answer. Reopen the window to try again.")
    );
    root.appendChild(box);
  }

  function renderSearch(root) {
    const field = el("div", "searchfield");
    field.appendChild(el("span", "ic", "⌕"));
    const input = el("input");
    input.type = "search";
    input.placeholder = "Filter countries…";
    input.value = term;
    input.addEventListener("input", () => {
      term = input.value;
      renderList();
      // rebuilding the list steals focus from the box the user is typing into.
      input.focus();
    });
    field.appendChild(input);
    root.appendChild(field);
  }

  let listHost = null;

  function renderList() {
    listHost.textContent = "";
    const visible = rows.filter((r) => matchesCountry(r, term));
    if (visible.length === 0) {
      renderEmpty(
        listHost,
        "No country matches",
        "Nothing here answers to that. Clear the filter to see the whole list."
      );
      return;
    }
    const list = el("div", "rowlist");
    visible.forEach((c) => list.appendChild(renderRow(c)));
    listHost.appendChild(list);
  }

  function render() {
    host.textContent = "";
    const playing = rows.filter((r) => !r.excluded).length;

    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "◔ Countries on air"));
    head.appendChild(el("span", "cnt", countryHeading(playing, rows.length)));
    host.appendChild(head);
    host.appendChild(
      el(
        "p",
        "panesub",
        "Switch a country off and its stations stop playing — not in shuffle, not in search."
      )
    );

    if (failed) {
      renderFailed(host);
      return;
    }
    if (rows.length === 0) {
      renderEmpty(
        host,
        "No countries yet",
        "The station catalogue has not been downloaded yet. Once it syncs, every country it covers appears here."
      );
      return;
    }

    renderSearch(host);
    listHost = el("div");
    host.appendChild(listHost);
    renderList();
  }

  render();
  refresh();

  return { refresh };
}
