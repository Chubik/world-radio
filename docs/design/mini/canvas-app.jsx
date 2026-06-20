/* canvas-app.jsx — assembles the full World Radio Mini design canvas. */
const { useState } = React;

const THEMES = window.WR_THEMES, ORDER = window.WR_THEME_ORDER;
const STATES = ["playing", "stopped", "buffering", "error"];
const STATE_LABEL = { playing: "Playing", stopped: "Stopped / idle", buffering: "Buffering", error: "Error / offline" };

/* small caption under a sub-group inside an artboard */
function Cap({ children, c = "#8a7f64" }) {
  return <div style={{ fontFamily: window.WR_MONO, fontSize: 10.5, color: c, letterSpacing: "0.04em", marginBottom: 12, textTransform: "uppercase" }}>{children}</div>;
}
function Note({ children }) {
  return <p style={{ fontFamily: window.WR_SANS, fontSize: 12.5, lineHeight: 1.55, color: "#b8ac90", margin: "0 0 10px", maxWidth: 560 }}>{children}</p>;
}
function Row({ children, gap = 22, wrap = true, align = "flex-start" }) {
  return <div style={{ display: "flex", flexWrap: wrap ? "wrap" : "nowrap", gap, alignItems: align }}>{children}</div>;
}
function Stack({ label, children }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
      <Cap>{label}</Cap>
      {children}
    </div>
  );
}

/* ============================ INTRO ============================ */
function Intro() {
  return (
    <DCArtboard id="intro" label="Read me first" width={760} height={486}>
      <div style={{ padding: "34px 38px", fontFamily: window.WR_SANS, color: "#cfc4ad", background: "#15100b", height: "100%", boxSizing: "border-box" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 6 }}>
          <window.LauncherIcon themeKey="amber" size={46} radius={11} />
          <div>
            <div style={{ fontFamily: window.WR_MONO, fontSize: 12, color: "#d49a3a", letterSpacing: "0.14em" }}>▌WR · MINI</div>
            <h1 style={{ fontFamily: window.WR_SANS, fontWeight: 700, fontSize: 24, margin: "2px 0 0", color: "#ece2cb", letterSpacing: "-0.01em" }}>World Radio Mini — design pass</h1>
          </div>
        </div>
        <p style={{ fontSize: 13.5, lineHeight: 1.6, color: "#b8ac90", maxWidth: 640, marginTop: 14 }}>
          A tray / menu-bar companion to World Radio. The headline action is <b style={{ color: "#ffc457" }}>Shuffle</b> — one tap and a station plays.
          Everything below treats the mini window as one reusable CRT panel (~260×120), wrapped in each platform's native chrome.
        </p>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "14px 30px", marginTop: 18, fontSize: 12.5, lineHeight: 1.5 }}>
          <div><b style={{ color: "#ffc457" }}>System.</b> <span style={{ color: "#b8ac90" }}>IBM Plex Mono for all data/controls, Plex Sans only for station titles. One glow + scanline overlay per theme. Primary = filled <span style={{ color: "#ffc457" }}>hi</span> color; everything else is hairline <span style={{ color: "#8a7f64" }}>rule</span> outlines.</span></div>
          <div><b style={{ color: "#ffc457" }}>States.</b> <span style={{ color: "#b8ac90" }}>Each surface is drawn in all four: playing, stopped/idle, buffering (the crossfade "connecting…" moment), and error/offline.</span></div>
          <div><b style={{ color: "#ffc457" }}>Themes.</b> <span style={{ color: "#b8ac90" }}>Amber CRT is default. Shown also in Mainframe Blue, Cyber Neon, Shortwave Green, Hi-Fi Paper (light), plus new Nord & Dracula.</span></div>
          <div><b style={{ color: "#ffc457" }}>Scope.</b> <span style={{ color: "#b8ac90" }}>A segmented ALL / ★ FAVS control sets shuffle scope, mirrored as two menu items for menu-only (GNOME).</span></div>
        </div>
        <div style={{ marginTop: 18, fontFamily: window.WR_MONO, fontSize: 11, color: "#6f6650", borderTop: "1px solid #3a2c17", paddingTop: 12 }}>
          open questions flagged at the bottom · spectrum animates in the <span style={{ color: "#9ec074" }}>playing</span> state only · sync is a single placeholder menu item, never required
        </div>
      </div>
    </DCArtboard>
  );
}

/* ============================ macOS (HERO) ============================ */
function MacSection() {
  return (
    <DCSection id="macos" title="macOS · menu-bar popover" subtitle="HERO — monochrome template icon top-right; click opens a popover anchored under it">
      <DCArtboard id="mac-states" label="Popover · all 4 states · Amber CRT" width={1380} height={384}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Row gap={26}>
            {STATES.map((s) => (
              <Stack key={s} label={STATE_LABEL[s]}>
                <window.MacPopover themeKey="amber" state={s} scope={s === "playing" ? "all" : "favs"} />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="mac-menu" label="Right-click menu + scope selection" width={560} height={400}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Row gap={34}>
            <Stack label="Menu · scope = ALL">
              <window.ContextMenu themeKey="amber" scope="all" native w={210} />
            </Stack>
            <Stack label="Scope = ★ FAVS">
              <window.ContextMenu themeKey="amber" scope="favs" native w={210} />
            </Stack>
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="mac-themes" label="Popover themed · playing" width={1380} height={384}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Row gap={24}>
            {["amber", "blue", "neon", "nord"].map((k) => (
              <Stack key={k} label={THEMES[k].name}>
                <window.MacPopover themeKey={k} state="playing" scope="all" />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ THEMES ============================ */
function ThemeSection() {
  return (
    <DCSection id="themes" title="The mini window, every theme" subtitle="The reusable ~260px CRT panel — playing state — across all seven palettes">
      <DCArtboard id="theme-grid" label="7 themes · playing" width={1180} height={840}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Row gap={24}>
            {ORDER.map((k) => (
              <Stack key={k} label={`${THEMES[k].name}${THEMES[k].tag === "default" ? " · default" : THEMES[k].tag === "new" ? " · new" : ""}`}>
                <window.MiniWindow themeKey={k} state="playing" scope="all" w={260} />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ LINUX ============================ */
function LinuxSection() {
  return (
    <DCSection id="linux" title="Linux · AppIndicator / StatusNotifier" subtitle="Left-click opens the window (KDE); GNOME gets a fully-usable menu-only variant">
      <DCArtboard id="lin-states" label="Indicator window · all 4 states" width={1460} height={372}>
        <div style={{ padding: 30, background: "#16181c", height: "100%", boxSizing: "border-box" }}>
          <Row gap={26}>
            {STATES.map((s) => (
              <Stack key={s} label={STATE_LABEL[s]}>
                <window.LinuxWindow themeKey="amber" state={s} scope="all" />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="lin-menu" label="GNOME menu-only variant (no left-click window)" width={560} height={440}>
        <div style={{ padding: 30, background: "#16181c", height: "100%", boxSizing: "border-box" }}>
          <Row gap={34}>
            <Stack label="Menu-only · header + actions">
              <window.MenuOnly themeKey="amber" scope="all" />
            </Stack>
            <Stack label="Themed · Dracula">
              <window.MenuOnly themeKey="dracula" scope="favs" />
            </Stack>
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ WINDOWS ============================ */
function WindowsSection() {
  return (
    <DCSection id="windows" title="Windows · system tray" subtitle="Colored tray icon; left-click opens the window, right-click the context menu">
      <DCArtboard id="win-states" label="Tray window · all 4 states" width={1380} height={400}>
        <div style={{ padding: 30, background: "#0a3a6b", height: "100%", boxSizing: "border-box", backgroundImage: "linear-gradient(160deg,#0a4a8b,#06203f)" }}>
          <Row gap={26}>
            {STATES.map((s) => (
              <Stack key={s} label={STATE_LABEL[s]}>
                <window.WinTray themeKey="amber" state={s} scope="all" />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="win-menu" label="Context menu + themed window" width={620} height={420}>
        <div style={{ padding: 30, background: "#06203f", height: "100%", boxSizing: "border-box" }}>
          <Row gap={30}>
            <Stack label="Right-click menu">
              <window.ContextMenu themeKey="amber" scope="all" w={210} />
            </Stack>
            <Stack label="Window · Shortwave Green">
              <window.MiniWindow themeKey="green" state="playing" scope="favs" w={264} radius={8} />
            </Stack>
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ ANDROID ============================ */
function AndroidSection() {
  return (
    <DCSection id="android" title="Android · notification, widget & compact screen" subtitle="Notification is the hero surface; widget mirrors it; compact screen is what they open into">
      <DCArtboard id="and-notif" label="Media notification · all 4 states (HERO surface)" width={1380} height={400}>
        <div style={{ padding: 30, background: "#101216", height: "100%", boxSizing: "border-box" }}>
          <Row gap={24}>
            {STATES.map((s) => (
              <Stack key={s} label={STATE_LABEL[s]}>
                <window.AndroidNotif themeKey="amber" state={s} scope={s === "stopped" ? "favs" : "all"} />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="and-widget" label="Home-screen widget · themed" width={720} height={320}>
        <div style={{ padding: 30, background: "#101216", height: "100%", boxSizing: "border-box" }}>
          <Row gap={24}>
            {["amber", "neon", "paper"].map((k) => (
              <Stack key={k} label={THEMES[k].name}>
                <window.AndroidWidget themeKey={k} state="playing" scope="all" />
              </Stack>
            ))}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="and-compact" label="Compact app screen · states + theme" width={840} height={540}>
        <div style={{ padding: 30, background: "#101216", height: "100%", boxSizing: "border-box" }}>
          <Row gap={24}>
            <Stack label="Playing · Amber"><window.AndroidCompact themeKey="amber" state="playing" scope="all" /></Stack>
            <Stack label="Buffering · Amber"><window.AndroidCompact themeKey="amber" state="buffering" scope="all" /></Stack>
            <Stack label="Playing · Mainframe Blue"><window.AndroidCompact themeKey="blue" state="playing" scope="favs" /></Stack>
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ ICONS ============================ */
function IconSection() {
  const tray = (state, label) => (
    <window.IconPlate label={label} sub={state}>
      <window.WRTrayGlyph color="#d49a3a" size={30} state={state} />
    </window.IconPlate>
  );
  return (
    <DCSection id="icons" title="Icon set" subtitle="Tray / menu-bar marks in each state, macOS monochrome template, colored variants, launcher">
      <DCArtboard id="ico-tray" label="Tray mark · states + treatments" width={760} height={360}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Cap>macOS monochrome template (system-tinted)</Cap>
          <Row gap={20} align="center">
            <window.IconPlate label="Idle" sub="template"><window.WRTrayGlyph color="#e6e6e6" size={30} state="idle" /></window.IconPlate>
            <window.IconPlate label="Playing" sub="template"><window.WRTrayGlyph color="#e6e6e6" size={30} state="playing" /></window.IconPlate>
            <window.IconPlate label="Buffering" sub="template"><window.WRTrayGlyph color="#e6e6e6" size={30} state="buffering" /></window.IconPlate>
            <window.IconPlate label="On light bar" sub="template" bg="#e8e8ea" light><window.WRTrayGlyph color="#1d1d1f" size={30} state="playing" /></window.IconPlate>
          </Row>
          <div style={{ height: 22 }} />
          <Cap>Colored (Windows / Linux / Android notification small-icon)</Cap>
          <Row gap={20} align="center">
            {tray("idle", "Idle")}
            {tray("playing", "Playing")}
            {tray("buffering", "Buffering")}
          </Row>
        </div>
      </DCArtboard>

      <DCArtboard id="ico-launcher" label="Launcher / app icon · themed" width={620} height={300}>
        <div style={{ padding: 30, background: "#1f1a12", height: "100%", boxSizing: "border-box" }}>
          <Cap>Launcher icon — squircle, themed, glowing ▌WR over scanlines</Cap>
          <Row gap={26} align="center">
            {["amber", "blue", "neon", "green", "dracula"].map((k) => (
              <div key={k} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10 }}>
                <window.LauncherIcon themeKey={k} size={88} radius={20} />
                <span style={{ fontFamily: window.WR_MONO, fontSize: 10, color: "#8a7f64" }}>{THEMES[k].name}</span>
              </div>
            ))}
          </Row>
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ SPEC NOTES ============================ */
function SpecSection() {
  const swatch = (k) => {
    const t = THEMES[k];
    const chip = (c, l) => (
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <span style={{ width: 13, height: 13, borderRadius: 3, background: c, boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.15)" }} />
        <span style={{ fontFamily: window.WR_MONO, fontSize: 9.5, color: "#8a7f64" }}>{l} {c}</span>
      </div>
    );
    return (
      <div key={k} style={{ background: "#15100b", borderRadius: 8, padding: 14, boxShadow: "inset 0 0 0 1px #3a2c17" }}>
        <div style={{ fontFamily: window.WR_MONO, fontSize: 11, color: "#ece2cb", marginBottom: 10 }}>{t.name} <span style={{ color: "#6f6650" }}>· {t.id}</span></div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "7px 14px" }}>
          {chip(t.bg, "bg")}{chip(t.fg, "fg")}{chip(t.hi, "hi")}{chip(t.accent, "accent")}{chip(t.ok, "ok")}{chip(t.err, "err")}
        </div>
      </div>
    );
  };
  return (
    <DCSection id="spec" title="Engineering notes" subtitle="Type, color, spacing & the CRT effect, so the look reproduces exactly">
      <DCArtboard id="spec-notes" label="Spec" width={760} height={470}>
        <div style={{ padding: "30px 36px", background: "#1b1610", height: "100%", boxSizing: "border-box", overflow: "hidden" }}>
          <Note><b style={{ color: "#ffc457" }}>Type.</b> IBM Plex Mono for all data, labels, controls (ligatures off). IBM Plex Sans 600 only for station titles. Window body 10–13.5px; never below 9px for meta.</Note>
          <Note><b style={{ color: "#ffc457" }}>Layout.</b> Window padding 11–13px. 7px vertical rhythm between rows. Shuffle is the only filled control; play/stop & scope use 1px <span style={{ color: "#8a7f64" }}>rule</span> outlines. Hit targets ≥36px on touch (Android), ≥28px on desktop.</Note>
          <Note><b style={{ color: "#ffc457" }}>CRT effect.</b> Scanlines = repeating 1px dark line every 3px at theme <code style={{ color: "#9ec074" }}>scan</code> opacity (0.05 light → 0.22 green), <code style={{ color: "#9ec074" }}>mix-blend-mode: overlay</code>. Top radial glow in the <code style={{ color: "#9ec074" }}>glow</code> color at ~13% alpha. Station title gets a 8px text-shadow glow when playing. Light themes (Hi-Fi Paper, Nord, Dracula) drop the glow.</Note>
          <Note><b style={{ color: "#ffc457" }}>Motion.</b> Spectrum bars animate only while playing; buffering dots/⏳ pulse; now-playing marquees if it overflows. All honor <code style={{ color: "#9ec074" }}>prefers-reduced-motion</code>.</Note>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 12, marginTop: 16 }}>
            {["amber", "blue", "green"].map(swatch)}
          </div>
        </div>
      </DCArtboard>

      <DCArtboard id="spec-questions" label="Open questions for the team" width={420} height={470}>
        <div style={{ padding: "30px 32px", background: "#1b1610", height: "100%", boxSizing: "border-box", fontFamily: window.WR_SANS }}>
          <Cap c="#d49a3a">Flagged in the brief</Cap>
          {[
            ["Tray-icon legibility", "The scanline/glow doesn't survive at 16px. The tray mark drops both and uses just the ▌ bar + equalizer/arcs. Approve the simplified mark?"],
            ["GNOME menu-only", "Menu-only includes a now-playing header so it reads on its own. Good enough, or push a separate small window on GNOME too?"],
            ["Android hero", "Drawn notification as primary, widget as mirror. CRT survives via the album-art tile + spectrum; Android tints the rest. Agree notification > widget?"],
            ["Spectrum at this size", "Kept it, animated only when playing. Too noisy in the 260px window, or keep?"],
          ].map(([h, b], i) => (
            <div key={i} style={{ marginBottom: 14 }}>
              <div style={{ fontFamily: window.WR_MONO, fontSize: 11.5, color: "#ffc457", marginBottom: 3 }}>{i + 1}. {h}</div>
              <div style={{ fontSize: 12, lineHeight: 1.5, color: "#b8ac90" }}>{b}</div>
            </div>
          ))}
        </div>
      </DCArtboard>
    </DCSection>
  );
}

/* ============================ APP ============================ */
function App() {
  // NOTE: DesignCanvas only detects DCSection/DCArtboard as direct literal
  // children, so we invoke the section builders as functions (not <Comp/>).
  return (
    <DesignCanvas>
      <DCSection id="overview" title="World Radio Mini" subtitle="Tray / menu-bar companion · assumptions, system & reading order">
        {Intro()}
      </DCSection>
      {MacSection()}
      {ThemeSection()}
      {LinuxSection()}
      {WindowsSection()}
      {AndroidSection()}
      {IconSection()}
      {SpecSection()}
    </DesignCanvas>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App />);
