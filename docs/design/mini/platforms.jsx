/* Platform wrappers around MiniWindow + the context/popover menu.
   Exports: MacPopover, LinuxWindow, WinTray, ContextMenu, MenuOnly,
            AndroidNotif, AndroidWidget, AndroidCompact, MenuBarStrip, TaskbarStrip. */

/* ── shared right-click / popover menu ── */
function ContextMenu({ themeKey = "amber", scope = "all", w = 200, native = false }) {
  const t = window.WR_THEMES[themeKey];
  const item = (label, glyph, opts = {}) => (
    <div style={{
      display: "flex", alignItems: "center", gap: 9, padding: "6px 11px",
      fontFamily: window.WR_MONO, fontSize: 11.5, color: opts.danger ? t.err : opts.dim ? t.dim : t.fg,
      background: opts.active ? (native ? t.hi + "22" : t.hi) : "transparent",
      borderRadius: native ? 4 : 0, cursor: "pointer", whiteSpace: "nowrap",
    }}>
      <span style={{ width: 14, textAlign: "center", color: opts.active ? (native ? t.hi : t.bg) : t.hi, opacity: opts.dim ? 0.6 : 1 }}>{glyph}</span>
      <span style={{ color: opts.active && !native ? t.bg : "inherit", fontWeight: opts.active ? 600 : 400 }}>{label}</span>
      {opts.right && <span style={{ marginLeft: "auto", fontSize: 9.5, color: t.dim }}>{opts.right}</span>}
    </div>
  );
  const sep = <div style={{ height: 1, background: t.rule, margin: "4px 8px" }} />;
  return (
    <div style={{ width: w, background: t.panel, borderRadius: native ? 9 : 5, padding: "5px 0", boxShadow: `inset 0 0 0 1px ${t.rule}, 0 10px 30px -8px rgba(0,0,0,0.6)`, fontFamily: window.WR_MONO, overflow: "hidden" }}>
      {item("Shuffle — all stations", "⇄", { active: scope === "all", right: "⏎" })}
      {item("Shuffle — favorites", "★", { active: scope === "favs" })}
      {sep}
      {item("Play / Stop", "⏯")}
      {item("Volume", "▮", { right: "60%" })}
      {sep}
      {item("Open World Radio", "▌", { right: "↗" })}
      {item("Settings · Connect key", "⚙", { dim: true })}
      {item("Theme", "◐", { dim: true, right: "›" })}
      {sep}
      {item("Quit", "✕", { danger: true })}
    </div>
  );
}

/* menu-only variant (GNOME) — adds the now-playing line at the top as a header */
function MenuOnly({ themeKey = "amber", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  return (
    <div style={{ width: 222, background: t.panel, borderRadius: 6, padding: "0 0 5px", boxShadow: `inset 0 0 0 1px ${t.rule}, 0 10px 30px -8px rgba(0,0,0,0.6)`, overflow: "hidden", fontFamily: window.WR_MONO }}>
      <div style={{ padding: "9px 11px", background: t.bg, borderBottom: `1px solid ${t.rule}`, display: "flex", flexDirection: "column", gap: 3 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <window.WRMark t={t} size={10} />
          <window.StateDot t={t} state="playing" />
        </div>
        <div style={{ fontFamily: window.WR_SANS, fontWeight: 600, fontSize: 12, color: t.bright, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>Smooth Jazz Café</div>
        <div style={{ fontSize: 9.5, color: t.accent, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>♪ Chuck Wayne — What A Difference…</div>
      </div>
      <div style={{ padding: "5px 0" }}>
        <ContextMenu themeKey={themeKey} scope={scope} w={222} />
      </div>
    </div>
  );
}

/* ── macOS menu-bar popover ── */
function MenuBarStrip({ t, iconOnly = false }) {
  return (
    <div style={{ width: iconOnly ? "auto" : 360, height: 24, background: "rgba(245,245,247,0.72)", backdropFilter: "blur(8px)", borderRadius: 6, display: "flex", alignItems: "center", padding: "0 9px", gap: 13, boxShadow: "inset 0 0 0 0.5px rgba(0,0,0,0.12)" }}>
      {!iconOnly && <>
        <span style={{ fontWeight: 700, fontSize: 12, color: "#1d1d1f", fontFamily: window.WR_SANS }}></span>
        <span style={{ marginLeft: "auto", display: "flex", gap: 13, alignItems: "center", color: "#3a3a3c", fontSize: 11, fontFamily: window.WR_SANS }}>
          <span>􀙇</span><span>􀊝</span>
        </span>
      </>}
      {/* the WR tray icon, highlighted/active */}
      <span style={{ display: "inline-flex", alignItems: "center", justifyContent: "center", width: 22, height: 18, borderRadius: 4, background: "rgba(0,0,0,0.10)" }}>
        <WRTrayGlyph color="#1d1d1f" size={13} state="playing" />
      </span>
      {!iconOnly && <><span style={{ fontSize: 11, color: "#3a3a3c", fontFamily: window.WR_SANS }}>{nowClock()}</span></>}
    </div>
  );
}
function nowClock() { return "9:41"; }

function MacPopover({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", width: 300 }}>
      <MenuBarStrip t={t} />
      {/* arrow */}
      <div style={{ width: 0, height: 0, marginRight: 18, marginTop: 6, borderLeft: "8px solid transparent", borderRight: "8px solid transparent", borderBottom: `8px solid ${t.bg}`, filter: "drop-shadow(0 -1px 0 rgba(0,0,0,0.3))" }} />
      <div style={{ borderRadius: 14, boxShadow: "0 18px 50px -12px rgba(0,0,0,0.55)", marginTop: -1 }}>
        <window.MiniWindow themeKey={themeKey} state={state} scope={scope} w={272} radius={14} />
      </div>
    </div>
  );
}

/* ── Linux indicator window (KDE/GNOME) ── */
function TaskbarStrip({ t, os = "linux" }) {
  return (
    <div style={{ width: 320, height: 28, background: "#1c1c1c", borderRadius: 6, display: "flex", alignItems: "center", padding: "0 10px", gap: 12, boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.06)" }}>
      <span style={{ color: "#cfcfcf", fontSize: 11, fontFamily: window.WR_SANS, fontWeight: 600 }}>Activities</span>
      <span style={{ marginLeft: "auto", display: "flex", gap: 11, alignItems: "center", color: "#cfcfcf", fontSize: 11, fontFamily: window.WR_MONO }}>
        <span style={{ display: "inline-flex", width: 20, height: 20, borderRadius: 4, alignItems: "center", justifyContent: "center", background: "rgba(255,255,255,0.10)" }}>
          <WRTrayGlyph color="#e6e6e6" size={13} state="playing" />
        </span>
        <span>en</span><span>{nowClock()}</span>
      </span>
    </div>
  );
}

function LinuxWindow({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", width: 320 }}>
      <TaskbarStrip t={t} />
      <div style={{ marginTop: 8, marginRight: 6, borderRadius: 10, boxShadow: "0 14px 40px -12px rgba(0,0,0,0.6)" }}>
        <window.MiniWindow themeKey={themeKey} state={state} scope={scope} w={264} radius={10} />
      </div>
    </div>
  );
}

/* ── Windows tray window ── */
function WinTray({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", width: 300 }}>
      <div style={{ marginBottom: 8, borderRadius: 8, overflow: "hidden", boxShadow: "0 14px 40px -12px rgba(0,0,0,0.6)" }}>
        <window.MiniWindow themeKey={themeKey} state={state} scope={scope} w={272} radius={8} />
      </div>
      {/* windows 11 taskbar (bottom, centered tray at right) */}
      <div style={{ width: 300, height: 30, background: "rgba(243,243,243,0.85)", backdropFilter: "blur(10px)", borderRadius: 7, display: "flex", alignItems: "center", padding: "0 10px", gap: 12, boxShadow: "inset 0 0 0 1px rgba(0,0,0,0.06)" }}>
        <span style={{ marginLeft: "auto", display: "flex", gap: 10, alignItems: "center", color: "#202020", fontSize: 11, fontFamily: window.WR_SANS }}>
          <span style={{ display: "inline-flex", width: 20, height: 20, borderRadius: 4, alignItems: "center", justifyContent: "center", background: "rgba(0,0,0,0.06)" }}>
            <WRTrayGlyph color="#202020" size={13} state="playing" colored themeKey={themeKey} />
          </span>
          <span style={{ lineHeight: 1.1, textAlign: "right" }}>{nowClock()}<br/><span style={{ fontSize: 9 }}>6/19/2026</span></span>
        </span>
      </div>
    </div>
  );
}

/* small tray glyph reused in strips — full def lives in icons.jsx as WRTrayGlyph */
Object.assign(window, { ContextMenu, MenuOnly, MacPopover, LinuxWindow, WinTray, MenuBarStrip, TaskbarStrip, nowClock });
