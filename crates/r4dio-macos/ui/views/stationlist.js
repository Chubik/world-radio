import { flagFor, signalBars } from "../labels.js";

/** the station list every tab draws: a header row of column labels, then one
 *  TUI row per station — cursor, name, flag, codec, signal.
 *
 *  it exists because favourites, blocked and browse all show the same row and
 *  all answer the same keys; three copies would drift the moment one of them
 *  gained a column. */

export function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

export function codecLabel(station) {
  const codec = (station.codec ?? "").trim();
  const rate = Number(station.bitrate) || 0;
  if (!codec && !rate) {
    return "";
  }
  if (!rate) {
    return codec;
  }
  return `${codec} ${rate}k`;
}

/** ●●●●○ — filled marks for the bitrate, hollow ones for the rest, so the
 *  column keeps one width and the eye compares position, not length. */
export function signalCell(station) {
  const cell = el("span", "sig");
  const on = signalBars(station.bitrate);
  if (on === 0) {
    cell.appendChild(el("span", "off", "—"));
    return cell;
  }
  cell.appendChild(el("span", "on", "●".repeat(on)));
  if (on < 5) {
    cell.appendChild(el("span", "off", "○".repeat(5 - on)));
  }
  return cell;
}

export function headRow(actionLabel) {
  const head = el("div", "listhead");
  head.appendChild(el("span", "car", ""));
  head.appendChild(el("span", "nm", "STATION"));
  head.appendChild(el("span", "flag", "CC"));
  head.appendChild(el("span", "cod", "CODEC"));
  head.appendChild(el("span", "sig", "SIGNAL"));
  if (actionLabel) head.appendChild(el("span", "act", actionLabel));
  return head;
}

/**
 * keeps the cursor for one list.
 *
 * the index is held rather than a uuid: a refresh can drop the selected station
 * (unfavourited, unblocked) and the cursor must stay where the user's eye is,
 * on the row that took its place, not jump to the top.
 */
export function cursor() {
  let index = 0;
  return {
    get value() {
      return index;
    },
    clamp(length) {
      if (length === 0) {
        index = 0;
        return;
      }
      index = Math.min(Math.max(index, 0), length - 1);
    },
    move(delta, length) {
      if (length === 0) return false;
      const next = Math.min(Math.max(index + delta, 0), length - 1);
      const moved = next !== index;
      index = next;
      return moved;
    },
    set(next) {
      index = next;
    },
  };
}

/** builds one row. `station.is_playing` is drawn apart from the cursor: the
 *  station playing and the row you are pointing at are different facts, and a
 *  driver of this window needs both at once. */
export function stationRow(station, { selected, action, onPlay, onAction }) {
  const row = el(
    "div",
    `row${selected ? " sel" : ""}${station.is_playing ? " playing" : ""}`
  );
  row.appendChild(el("span", "car", selected ? "▸" : ""));
  row.appendChild(el("span", "nm", station.name));
  row.appendChild(el("span", "flag", `${flagFor(station.country)} ${station.country ?? ""}`.trim()));
  row.appendChild(el("span", "cod", codecLabel(station)));
  row.appendChild(signalCell(station));

  if (action) {
    const act = el("span", "act", action.label);
    act.title = action.title ?? "";
    act.addEventListener("click", (e) => {
      // the row itself plays; without this the action would also start the
      // station it is in the middle of removing.
      e.stopPropagation();
      onAction();
    });
    row.appendChild(act);
  }

  row.addEventListener("click", onPlay);
  return row;
}
