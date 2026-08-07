// pure decisions, kept out of the DOM so they can be checked without a browser
// harness — the same reason HomeState.kt and WidgetState.kt exist on android.

// the panel's status word, its colour token, and whether it pulses.
export function stateLabels(phase) {
  switch (phase) {
    case "playing": return { text: "LIVE", tone: "ok", pulse: true, primary: "SHUFFLE" };
    case "buffering": return { text: "···", tone: "warn", pulse: true, primary: "SHUFFLE" };
    case "error": return { text: "OFFLINE", tone: "err", pulse: false, primary: "RETRY" };
    default: return { text: "IDLE", tone: "dim", pulse: false, primary: "SHUFFLE" };
  }
}

// six segments; clicking segment n sets n/6, so the filled count must round the
// same way on the way back or the bar jumps under the cursor.
export function volumeSegments(volume, count = 6) {
  const v = Math.min(1, Math.max(0, volume));
  return Math.round(v * count);
}

// nothing to star when nothing is playing.
export function showsStar(phase) {
  return phase !== "idle";
}

// a key arrives pasted, so it carries whatever the clipboard had around it.
// lowercasing matches the backend's accepted alphabet rather than rejecting a
// key the user typed in caps.
export function normalizeKey(raw) {
  return (raw ?? "").trim().toLowerCase();
}

// mirrors radio_core::sync::is_valid_format, so the window can refuse an obvious
// typo without a round trip. the backend stays the authority.
export function isValidKey(key) {
  return /^r4-[a-z0-9]+$/.test(key);
}

// the sync window must never echo the key back, so the status line reports only
// whether one is stored.
export function keyStatus(hasKey) {
  return hasKey
    ? { text: "KEY SET", tone: "ok" }
    : { text: "NO KEY", tone: "dim" };
}

// what the window says after an action; the backend's save can fail on a bad
// format or an unwritable data dir, and that must reach the user.
export function actionResult(action, ok) {
  switch (action) {
    case "save":
      return ok
        ? { text: "key saved", tone: "ok" }
        : { text: "could not save that key", tone: "err" };
    case "clear":
      return { text: "key cleared", tone: "dim" };
    case "sync":
      return ok
        ? { text: "sync requested", tone: "ok" }
        : { text: "sync needs a key first", tone: "err" };
    default:
      return { text: "", tone: "dim" };
  }
}
