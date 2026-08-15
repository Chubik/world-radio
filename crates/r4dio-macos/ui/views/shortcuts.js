import { el } from "./stationlist.js";

/** the shortcut list. it is written here rather than derived from the footer
 *  hints: the footer shows only what the current tab answers, while this is the
 *  whole set, including the one key that works when the window is not even
 *  in front. */
const KEYS = [
  ["⌥⇧R", "shuffle — all stations, works from any app"],
  ["SPACE", "play / stop"],
  ["↑ ↓", "move the cursor in a list"],
  ["↵", "play the selected station"],
  ["F", "favorite the selected station"],
  ["B", "block the selected station"],
  ["⌘F", "search in browse"],
  ["⌘1–4", "switch tab"],
];

export function mountShortcuts(host) {
  host.replaceChildren();
  host.appendChild(el("div", "sectlbl", "KEYBOARD SHORTCUTS"));
  for (const [key, desc] of KEYS) {
    const row = el("div", "kbdrow");
    row.appendChild(el("span", "k", key));
    row.appendChild(el("span", "d", desc));
    host.appendChild(row);
  }
}
