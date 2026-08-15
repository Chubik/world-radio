/** every key the window answers, and nothing it does not: a shortcut list that
 *  promises a key nothing handles is worse than no list. the footer shows the
 *  subset that applies to the current tab; this is the whole set, including the
 *  one that works when the window is not even in front. */
const KEYS = [
  ["⌥⇧R", "shuffle — works system-wide, from any app"],
  ["r", "shuffle a station"],
  ["SPACE", "play / stop"],
  ["↑ ↓", "move the cursor"],
  ["↵", "play the selected station"],
  ["F", "favourite the selected station"],
  ["1 2 3", "all / favourites / history"],
  ["⌘F", "search"],
  ["ESC", "leave the search field"],
  ["⌘1 ⌘2", "library / settings"],
];

export function mountShortcuts(host) {
  host.replaceChildren();
  const label = document.createElement("div");
  label.className = "sectlbl";
  label.textContent = "KEYBOARD SHORTCUTS";
  host.appendChild(label);
  for (const [key, desc] of KEYS) {
    const row = document.createElement("div");
    row.className = "kbdrow";
    const k = document.createElement("span");
    k.className = "k";
    k.textContent = key;
    const d = document.createElement("span");
    d.className = "d";
    d.textContent = desc;
    row.append(k, d);
    host.appendChild(row);
  }
}
