const invoke = window.__TAURI__.core.invoke;

/** the meter's five looks, in the order the terminal client cycles them, so a
 *  user who knows one knows the other. */
const STYLES = [
  ["bars", "one bar per band, from the floor"],
  ["mirror", "bars grown from the middle, both ways"],
  ["dots", "a single mark riding each band"],
  ["wave", "neighbours averaged — the shape, not the peaks"],
  ["off", "no meter at all"],
];

/** gain is the analyser's divisor inverted for the user: a quiet station needs
 *  a *smaller* divisor to fill the meter, which reads backwards on a slider.
 *  the slider goes low-to-high sensitivity and the divisor follows it down. */
const GAIN_MIN = 2;
const GAIN_MAX = 40;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function mountAppearance(host, { onStyle }) {
  let style = "bars";
  let gain = 12;
  let at = 0;

  async function refresh() {
    try {
      const eq = await invoke("eq_settings");
      style = eq.style;
      gain = eq.gain;
      at = Math.max(0, STYLES.findIndex(([name]) => name === style));
    } catch (e) {
      console.error("eq_settings failed", e);
    }
    render();
  }

  async function save() {
    try {
      await invoke("set_eq", { style, gain });
    } catch (e) {
      console.error("set_eq failed", e);
      return;
    }
    if (onStyle) onStyle(style);
    render();
  }

  function choose(name) {
    style = name;
    at = STYLES.findIndex(([s]) => s === name);
    save();
  }

  function render() {
    host.replaceChildren();

    const head = el("div", "paneh");
    head.appendChild(el("h3", null, "◫ Equalizer"));
    head.appendChild(el("span", "cnt", style));
    host.appendChild(head);
    host.appendChild(
      el("p", "panesub", "How the meter beside the station is drawn, and how hard it is driven. Kept on this Mac.")
    );

    host.appendChild(el("div", "sectlbl", "STYLE"));
    STYLES.forEach(([name, desc], i) => {
      const row = el("div", `themerow${i === at ? " sel" : ""}`);
      row.appendChild(el("span", "car", i === at ? "▸" : ""));
      row.appendChild(el("span", "nm", name));
      row.appendChild(el("span", "desc", desc));
      row.addEventListener("click", () => choose(name));
      host.appendChild(row);
    });

    host.appendChild(el("div", "sectlbl", "SENSITIVITY"));
    const row = el("div", "gainrow");
    row.appendChild(el("span", "lbl", "quiet"));
    const slider = el("input");
    slider.type = "range";
    slider.min = String(GAIN_MIN);
    slider.max = String(GAIN_MAX);
    slider.step = "1";
    // the slider reads left-to-right as "more sensitive", so it carries the
    // divisor reversed: dragging right lowers it.
    slider.value = String(GAIN_MAX + GAIN_MIN - Math.round(gain));
    slider.addEventListener("input", () => {
      gain = GAIN_MAX + GAIN_MIN - Number(slider.value);
      save();
    });
    row.appendChild(slider);
    row.appendChild(el("span", "lbl", "loud"));
    host.appendChild(row);
    host.appendChild(
      el("p", "panesub", "Drag towards quiet if the meter barely moves on soft stations, towards loud if it is pinned at the top.")
    );
  }

  render();
  refresh();

  return { refresh };
}
