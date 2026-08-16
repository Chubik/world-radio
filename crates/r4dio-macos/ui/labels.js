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

// the window's tabs, in the order they sit on the bar. the list is the authority
// on what a valid tab is, so a tab and a pane cannot drift apart.
export const TABS = ["library", "settings"];

const LANDING = "library";

// the tray still asks for the old section names, and so do older windows
// restored by macos. everything that is not settings is now one screen, so
// those names select a segment of the list rather than a tab of their own.
const SECTION_TAB = {
  library: ["library", "all"],
  now: ["library", "all"],
  browse: ["library", "all"],
  all: ["library", "all"],
  favourites: ["library", "favourites"],
  history: ["library", "history"],
  // blocked sits in settings beside the country exclusions: both answer "what
  // never plays", and neither is opened while choosing something to listen to.
  blocked: ["settings", "blocked"],
  settings: ["settings", "countries"],
  countries: ["settings", "countries"],
  appearance: ["settings", "appearance"],
  sync: ["settings", "account"],
  account: ["settings", "account"],
  shortcuts: ["settings", "shortcuts"],
};

/** the three views of one list. they filter rows; they never change the layout,
 *  so switching one cannot move anything the eye is already resting on. */
export const SEGMENTS = ["all", "favourites", "history"];

/** where a section id lands: the tab to open, and the sub-view inside it (or
 *  null when the tab has none). an unknown id lands on a real tab — a window
 *  blank because an id was misspelled looks broken rather than empty. */
export function targetFor(id) {
  const hit = SECTION_TAB[id];
  if (!hit) {
    return { tab: LANDING, sub: null };
  }
  return { tab: hit[0], sub: hit[1] };
}

export function activeTab(id) {
  return targetFor(id).tab;
}

/** the signal column. there is no per-station signal measurement anywhere in
 *  this project, so the scale is bitrate — which is what actually differs
 *  between two streams of the same station — and never a guess dressed as one. */
export function signalBars(bitrate) {
  // a few stations report bits per second, so 512000 means 512k. read raw it
  // would be off the scale for the meter and misprinted in the codec column.
  const raw = Number(bitrate) || 0;
  const kbps = raw >= 10000 ? Math.round(raw / 1000) : raw;
  if (kbps <= 0) {
    return 0;
  }
  if (kbps >= 256) return 5;
  if (kbps >= 192) return 4;
  if (kbps >= 128) return 3;
  if (kbps >= 64) return 2;
  return 1;
}

/** the keyboard hints under each tab. they change per tab because a hint for a
 *  key that does nothing here teaches the user to stop reading the row.
 *  every key listed here is handled — a hint for a key that does nothing is
 *  worse than no hint at all. */
const HINTS = {
  library: [
    ["↑ ↓", "select"], ["↵", "play"], ["SPACE", "play / stop"], ["F", "favourite"],
    ["r", "shuffle"], ["R", "retry"], ["M", "mute"], ["1–3", "segment"], ["⌘F", "search"],
  ],
  settings: [["SPACE", "play / stop"], ["r", "shuffle"], ["M", "mute"], ["⌘1–2", "switch tab"]],
};

export function hintsFor(tab) {
  return HINTS[activeTab(tab)] ?? HINTS[LANDING];
}

/** "18 min ago" — history rows carry a unix stamp in **seconds** (radio-core's
 *  now_secs), and a raw one tells the user nothing at a glance. `nowSeconds` is
 *  passed in rather than read here so the wording can be tested without
 *  freezing the clock. */
export function playedWhen(at, nowSeconds) {
  const seconds = Math.floor(nowSeconds - (Number(at) || 0));
  if (!at || seconds < 0) {
    return "";
  }
  if (seconds < 90) {
    return "just now";
  }
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) {
    return `${minutes} min ago`;
  }
  const hours = Math.round(minutes / 60);
  if (hours < 24) {
    return `${hours}h ago`;
  }
  const days = Math.round(hours / 24);
  if (days === 1) {
    return "yesterday";
  }
  if (days < 7) {
    return `${days}d ago`;
  }
  const weeks = Math.round(days / 7);
  return weeks === 1 ? "last week" : `${weeks}w ago`;
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

// a blocked station the catalogue can no longer resolve still needs a row to be
// unblocked from, so a placeholder stands in rather than leaving it blank.
export function blockedName(station) {
  const name = (station?.name ?? "").trim();
  return name === "" ? "Unknown station" : name;
}

// "191 of 194" — how many of the countries on offer are on air. it counts what
// plays rather than what is blocked, to match the switch beside each row.
export function countryHeading(playing, total) {
  return `${playing ?? 0} of ${total ?? 0}`;
}

// the intl table is the browser's, so it stays correct without shipping a list
// of 194 names. an unmappable code shows as itself rather than as a guess.
// fallback:"code" matters — without it an unassigned code resolves to the string
// "Unknown Region", which would sit in the list looking like a real country.
const REGION_NAMES = (() => {
  try {
    return new Intl.DisplayNames(["en"], { type: "region", fallback: "code" });
  } catch {
    return null;
  }
})();

export function countryName(code) {
  const c = (code ?? "").trim().toUpperCase();
  if (!/^[A-Z]{2}$/.test(c)) {
    return c;
  }
  let name;
  try {
    name = REGION_NAMES?.of(c);
  } catch {
    return c;
  }
  // CLDR answers "Unknown Region" for codes it reserves rather than refusing
  // them, and that placeholder would sit in the list looking like a country.
  if (!name || name === UNKNOWN_REGION) {
    return c;
  }
  return name;
}

const UNKNOWN_REGION = "Unknown Region";

// the filter box matches the name the user reads and the code they might type,
// so "swi", "CH" and "Switzerland" all find the same row.
export function matchesCountry(row, term) {
  const q = (term ?? "").trim().toLowerCase();
  if (q === "") {
    return true;
  }
  const code = (row?.code ?? "").toLowerCase();
  return code.includes(q) || countryName(row?.code).toLowerCase().includes(q);
}

// a new account has no favourites, and that is a normal state — the header says

// the offline catalogue answers 671 rows for "jazz" and 7,666 for one country,
// and only the first slice is ever drawn. saying so is what stops the list
// reading as the whole truth about the catalogue.
export function resultHeading(shown, capped) {
  const n = shown ?? 0;
  if (n === 0) {
    return "nothing found";
  }
  if (capped) {
    return `first ${stationCount(n)} results`;
  }
  return `${stationCount(n)} result${n === 1 ? "" : "s"}`;
}

const MIN_TERM = 2;

// one letter matches most of a 58k-row catalogue, which costs a full scan to
// produce a list nobody can use. two is where a search starts being a search.
export function isSearchable(term) {
  return (term ?? "").trim().length >= MIN_TERM;
}

export function stationCount(n) {
  return (n ?? 0).toLocaleString("en");
}
