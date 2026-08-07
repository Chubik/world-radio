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
