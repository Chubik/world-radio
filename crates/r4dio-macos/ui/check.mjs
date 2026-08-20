// checks the window can be loaded and driven, without launching the app.
//
// it exists because a duplicate `const` shipped a blank window: every rust test
// passed, the build was clean, and nothing looked at the javascript until it was
// on screen. these are the two questions a build cannot answer — does every
// module parse, and does the window still paint what the backend sends.
//
//   node crates/r4dio-macos/ui/check.mjs
//
// it needs no browser: the dom is stubbed just far enough for the views to run.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
let failures = 0;

function fail(what, detail) {
  failures++;
  console.log(`FAIL ${what}\n     ${detail}`);
}

function ok(what) {
  console.log(`ok   ${what}`);
}

// ── 1. every module parses ────────────────────────────────────────────────────
// a syntax error here is what a blank window looks like from the outside.

const modules = [
  "labels.js",
  "main.js",
  "app.js",
  ...readdirSync(join(here, "views"))
    .filter((f) => f.endsWith(".js"))
    .map((f) => `views/${f}`),
];

// `new Module` is the only way to parse module syntax without running it —
// stripping `export` by hand corrupts the source and reports errors that are
// not there, which is a check worse than no check.
const { SourceTextModule } = await import("node:vm").then((vm) => vm);

for (const file of modules) {
  const source = readFileSync(join(here, file), "utf8");
  try {
    new SourceTextModule(source, { identifier: file });
    ok(`parses ${file}`);
  } catch (e) {
    fail(`parses ${file}`, e.message);
  }
}

// ── 2. the html and the code agree on every id ───────────────────────────────
// getElementById returning null is the other way this window goes blank, and it
// only shows up at runtime.

const html = readFileSync(join(here, "main.html"), "utf8");
const declared = new Set([...html.matchAll(/id="([^"]+)"/g)].map((m) => m[1]));

for (const file of modules) {
  const source = readFileSync(join(here, file), "utf8");
  for (const [, id] of source.matchAll(/getElementById\("([^"]+)"\)/g)) {
    if (!declared.has(id)) {
      fail(`${file} wants #${id}`, "no element with that id in main.html");
    }
  }
}
ok("every getElementById has an element");

// ── 3. the window paints what the backend sends ──────────────────────────────
// a fake tauri bridge and enough dom to mount the panel, so a rename in
// NowState shows up here rather than on screen.

const NOW = {
  station: "Radio Swiss Jazz",
  track: "Chuck Wayne — What A Difference A Day Made",
  phase: "playing",
  scope: "all",
  is_favorite: true,
  meta: "CH · MP3 192k",
  filter: "",
  uptime: 842,
  buffer: 0.72,
  next: "FIP",
  muted: false,
  retries: 0,
  genre: "jazz",
  url: "http://stream.example/jazz",
};

installDom();
globalThis.window.__TAURI__ = {
  core: {
    invoke: async (cmd) => {
      if (cmd === "now_state") return NOW;
      if (cmd === "spectrum") return Array.from({ length: 34 }, (_, i) => (i % 7) / 7);
      if (cmd === "eq_settings") return { style: "bars", gain: 12 };
      return null;
    },
  },
  event: { listen: async () => () => {} },
};

const { mountNowPanel } = await import(pathToFileURL(join(here, "views/nowpanel.js")));
const panel = mountNowPanel({ onChanged: () => {} });
await panel.refresh();

const text = (id) => globalThis.document.getElementById(id)?.textContent ?? "";
const expect = (what, got, want) => {
  if (String(got).includes(want)) {
    ok(what);
    return;
  }
  fail(what, `got ${JSON.stringify(String(got))}, wanted it to contain ${JSON.stringify(want)}`);
};

expect("the panel names the station", text("np_name"), "Radio Swiss Jazz");
expect("the panel shows the track", text("np_track"), "Chuck Wayne");
expect("the panel shows the genre", text("np_meta"), "jazz");
expect("the buffer gauge reads the level", text("np_buf"), "72%");
expect("uptime is worded, not raw seconds", text("np_up"), "14m");
expect("the next station is named", text("np_scope"), "FIP");
expect("the clock says what state we are in", text("clock"), "LIVE");

// muted is the one state the panel can contradict: the stream is live and the
// meter has data, so without this the window says "playing" through silence.
// that is exactly what shipped once, and the user had no way to tell mute from
// a broken stream.
NOW.muted = true;
await panel.refresh();
expect("a muted panel says so", text("np_status"), "MUTED");
NOW.muted = false;
await panel.refresh();

console.log(failures === 0 ? "\nall window checks pass" : `\n${failures} FAILED`);
process.exit(failures === 0 ? 0 : 1);

/** the smallest dom these views actually touch. it is hand-written rather than
 *  pulled in as a dependency: the views use a dozen calls, and a real dom
 *  implementation would be a far larger thing to keep working. */
function installDom() {
  const byId = new Map();

  class El {
    constructor(tag) {
      this.tagName = tag;
      this.children = [];
      this.style = {};
      this.dataset = {};
      this.classList = new ClassList();
      this._text = "";
    }
    get textContent() {
      if (this.children.length === 0) return this._text;
      return this._text + this.children.map((c) => c.textContent ?? String(c)).join("");
    }
    set textContent(v) {
      this._text = v;
      this.children = [];
    }
    set className(v) {
      this.classList.set(v);
    }
    get className() {
      return [...this.classList.names].join(" ");
    }
    appendChild(child) {
      this.children.push(child);
      return child;
    }
    append(...items) {
      this.children.push(...items);
    }
    replaceChildren(...items) {
      this._text = "";
      this.children = items;
    }
    addEventListener() {}
    removeChild() {}
    querySelector(sel) {
      // only the one shape the panel uses: a class inside this element.
      const want = sel.replace(".", "");
      return this.children.find((c) => c.classList?.has?.(want)) ?? null;
    }
    querySelectorAll() {
      return [];
    }
    getBoundingClientRect() {
      return { top: 0, left: 0, width: 100, height: 10, bottom: 10, right: 100 };
    }
    get firstElementChild() {
      return this.children[0] ?? null;
    }
  }

  class ClassList {
    constructor() {
      this.names = new Set();
    }
    set(v) {
      this.names = new Set(String(v).split(/\s+/).filter(Boolean));
    }
    add(...n) {
      n.forEach((x) => this.names.add(x));
    }
    remove(...n) {
      n.forEach((x) => this.names.delete(x));
    }
    toggle(n, on) {
      if (on === undefined) on = !this.names.has(n);
      on ? this.names.add(n) : this.names.delete(n);
    }
    contains(n) {
      return this.names.has(n);
    }
    has(n) {
      return this.names.has(n);
    }
  }

  // every id the html declares gets an element, so the views find what they
  // reach for — which is exactly what check 2 above verifies.
  for (const id of declared) {
    const el = new El("div");
    el.id = id;
    byId.set(id, el);
  }
  // the panel builds its star inside the action div.
  const starIcon = new El("span");
  starIcon.classList.add("ic");
  byId.get("np_star")?.appendChild(starIcon);

  globalThis.document = {
    getElementById: (id) => byId.get(id) ?? null,
    createElement: (tag) => new El(tag),
    querySelectorAll: () => [],
    querySelector: () => null,
    addEventListener: () => {},
  };
  globalThis.window = { addEventListener: () => {} };
  globalThis.setInterval = () => 0;
  globalThis.clearInterval = () => {};
}
