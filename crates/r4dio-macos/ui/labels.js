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

const MASK = "····";

// after the save-this screen the id may only appear masked, so this keeps the
// first and last segments — enough to recognise the account — and drops every
// segment between them. anything that is not segmented is masked whole rather
// than shown, because failing towards hiding is the only safe direction here.
export function maskKey(key) {
  const raw = (key ?? "").trim();
  if (raw === "") {
    return "";
  }
  const parts = raw.split("-");
  if (parts.length < 4) {
    return `${parts[0]}-${MASK}`;
  }
  return `${parts[0]}-${parts[1]}-${MASK}-${parts[parts.length - 1]}`;
}

// the account section is one flow in three states; this maps the backend's
// state onto the copy each one shows, so the view only places strings.
export function accountStatus(state) {
  const s = state ?? {};
  if (!s.signed_in) {
    return { state: "signed_out", pill: "", masked: "", detail: "" };
  }
  return {
    state: "signed_in",
    pill: "⊙ synced",
    masked: s.masked ?? "",
    detail: favouritesLine(s.favourites ?? 0),
  };
}

function favouritesLine(n) {
  if (n === 0) {
    return "★ no favorites yet.";
  }
  return `★ ${n} favorite${n === 1 ? "" : "s"} synced.`;
}
