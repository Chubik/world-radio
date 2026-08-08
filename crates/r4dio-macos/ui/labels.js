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
  if (parts.length >= 4) {
    return `${parts[0]}-${parts[1]}-${MASK}-${parts[parts.length - 1]}`;
  }
  // real keys are one long run, not the segmented shape the mockup drew. showing
  // both ends tells two accounts apart; the body stays far too short to rebuild.
  const body = parts.slice(1).join("-");
  if (body.length <= EDGE * 2) {
    return `${parts[0]}-${MASK}`;
  }
  return `${parts[0]}-${body.slice(0, EDGE)}${MASK}${body.slice(-EDGE)}`;
}

const EDGE = 4;

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

// the window's sections, in sidebar order. the list is the authority on what a
// valid section is, so a nav item and a pane cannot drift apart.
export const SECTIONS = ["favourites", "browse", "countries", "blocked", "sync"];

const LANDING = "favourites";

// an unknown id must resolve to a real section: a window whose content pane is
// blank because a nav id was misspelled looks broken rather than empty.
export function activeSection(id) {
  return SECTIONS.includes(id) ? id : LANDING;
}

const A = "A".codePointAt(0);
const REGIONAL_A = 0x1f1e6;

// a two-letter country code maps onto the regional-indicator pair that renders
// as a flag. anything else is left blank rather than guessed — a wrong flag is
// worse than none, and radio-browser leaves the field empty often.
export function flagFor(code) {
  const c = (code ?? "").trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(c)) {
    return "";
  }
  return String.fromCodePoint(
    REGIONAL_A + c.codePointAt(0) - A,
    REGIONAL_A + c.codePointAt(1) - A
  );
}

// one line, one job: the live row announces itself, every other row shows the
// format. showing both would put the format where the eye looks for the marker.
export function rowSubtitle(station, isPlaying) {
  if (isPlaying) {
    return "● now playing";
  }
  const s = station ?? {};
  const codec = s.codec ?? "";
  const rate = s.bitrate ? `${s.bitrate}k` : "";
  return [codec, rate].filter((p) => p !== "").join(" ");
}

export function filterSummary(excluded, blocked) {
  return `◔ ${excluded ?? 0} excluded · ⛌ ${blocked ?? 0} blocked`;
}

// a new account has no favourites, and that is a normal state — the header says
// "none yet" rather than a bare 0, which reads like a count that failed.
export function favouritesHeading(n) {
  return n ? String(n) : "none yet";
}
