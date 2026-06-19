/* Android surfaces — media notification, home-screen widget, compact app screen.
   Keeps the CRT identity inside Android's media conventions.
   Exports: AndroidNotif, AndroidWidget, AndroidCompact, AndroidShade. */

function AndroidArt({ t, size = 44 }) {
  // album-art tile standing in for the station: CRT square with ▌WR + mini spectrum
  return (
    <div style={{ width: size, height: size, borderRadius: 8, position: "relative", overflow: "hidden", background: t.bg, boxShadow: `inset 0 0 0 1px ${t.rule}`, flex: "none" }}>
      <div style={{ position: "absolute", inset: 0, backgroundImage: window.crtOverlay(t), mixBlendMode: "overlay", opacity: 0.7 }} />
      <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
        <span style={{ fontFamily: window.WR_MONO, fontWeight: 700, fontSize: size * 0.3, color: t.hi, textShadow: t.light ? "none" : `0 0 6px ${t.glow}` }}>▌WR</span>
      </div>
    </div>
  );
}

function NotifBtn({ t, glyph, label, primary }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 3 }}>
      <div style={{ width: primary ? 42 : 36, height: primary ? 42 : 36, borderRadius: "50%", display: "flex", alignItems: "center", justifyContent: "center", background: primary ? t.hi : "transparent", boxShadow: primary ? "none" : `inset 0 0 0 1.5px ${t.rule}`, color: primary ? t.bg : t.fg, fontSize: primary ? 18 : 15 }}>{glyph}</div>
      <span style={{ fontFamily: window.WR_MONO, fontSize: 8.5, color: t.dim }}>{label}</span>
    </div>
  );
}

/* media notification — the hero Android surface */
function AndroidNotif({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  const isPlaying = state === "playing";
  const isBuffering = state === "buffering";
  const isError = state === "error";
  const station = isError ? "Sapporo Jazz" : isBuffering ? "Pure Lounge Radio" : "Smooth Jazz Café";
  const now = isPlaying ? "Chuck Wayne — What A Difference A Day Made" : isBuffering ? "connecting…" : isError ? "stream offline" : "press shuffle to start";
  return (
    <div style={{ width: 300, boxSizing: "border-box", borderRadius: 18, background: t.panel, padding: 13, boxShadow: `inset 0 0 0 1px ${t.rule}, 0 12px 34px -10px rgba(0,0,0,0.5)`, fontFamily: window.WR_SANS }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7, marginBottom: 9 }}>
        <window.WRTrayGlyph color={t.hi} size={13} state={isPlaying ? "playing" : "idle"} />
        <span style={{ fontFamily: window.WR_MONO, fontSize: 10, color: t.dim, letterSpacing: "0.04em" }}>World Radio</span>
        <window.StateDot t={t} state={state} />
        <span style={{ marginLeft: "auto", fontFamily: window.WR_MONO, fontSize: 9, color: t.dim }}>now</span>
      </div>
      <div style={{ display: "flex", gap: 11 }}>
        <AndroidArt t={t} size={48} />
        <div style={{ minWidth: 0, flex: 1 }}>
          <div style={{ fontWeight: 600, fontSize: 13.5, color: isError ? t.err : t.bright, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{station}</div>
          <div style={{ fontFamily: window.WR_MONO, fontSize: 10.5, color: isError ? t.err : isBuffering ? t.warn : t.accent, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", marginTop: 2 }}>{isPlaying ? "♪ " : ""}{now}</div>
          <div style={{ marginTop: 6 }}><window.Spectrum t={t} active={isPlaying} dim={!isPlaying} bars={20} height={13} /></div>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-around", marginTop: 12, paddingTop: 11, borderTop: `1px solid ${t.rule}` }}>
        <NotifBtn t={t} glyph="★" label={scope === "favs" ? "favs ✓" : "favs"} />
        <NotifBtn t={t} glyph="⇄" label="shuffle" primary />
        <NotifBtn t={t} glyph={isPlaying ? "⏸" : "▶"} label={isPlaying ? "stop" : "play"} />
        <NotifBtn t={t} glyph="▮" label="vol" />
        <NotifBtn t={t} glyph="▌" label="open" />
      </div>
    </div>
  );
}

/* home-screen widget — compact, same controls */
function AndroidWidget({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  const isPlaying = state === "playing";
  return (
    <div style={{ width: 200, boxSizing: "border-box", borderRadius: 22, position: "relative", overflow: "hidden", background: t.bg, padding: 13, boxShadow: `inset 0 0 0 1px ${t.rule}, 0 10px 26px -10px rgba(0,0,0,0.5)`, fontFamily: window.WR_SANS }}>
      <div style={{ position: "absolute", inset: 0, backgroundImage: window.crtOverlay(t), mixBlendMode: "overlay", pointerEvents: "none" }} />
      <div style={{ position: "relative" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <window.WRMark t={t} size={10} />
          <window.StateDot t={t} state={state} />
        </div>
        <div style={{ fontWeight: 600, fontSize: 12.5, color: t.bright, marginTop: 7, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>Smooth Jazz Café</div>
        <div style={{ fontFamily: window.WR_MONO, fontSize: 9.5, color: t.accent, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", marginTop: 1 }}>♪ Chuck Wayne — What A…</div>
        <div style={{ margin: "9px 0 10px" }}><window.Spectrum t={t} active={isPlaying} bars={22} height={14} /></div>
        <div style={{ display: "flex", gap: 7 }}>
          <div style={{ flex: 1, height: 34, borderRadius: 8, background: t.hi, color: t.bg, display: "flex", alignItems: "center", justifyContent: "center", gap: 6, fontFamily: window.WR_MONO, fontWeight: 700, fontSize: 11 }}>⇄ SHUFFLE</div>
          <div style={{ width: 34, height: 34, borderRadius: 8, boxShadow: `inset 0 0 0 1px ${t.rule}`, display: "flex", alignItems: "center", justifyContent: "center", color: t.fg, fontSize: 13 }}>{isPlaying ? "⏸" : "▶"}</div>
        </div>
      </div>
    </div>
  );
}

/* compact app screen the notification/widget opens into */
function AndroidCompact({ themeKey = "amber", state = "playing", scope = "all" }) {
  const t = window.WR_THEMES[themeKey];
  const isPlaying = state === "playing";
  return (
    <div style={{ width: 232, height: 420, borderRadius: 30, position: "relative", overflow: "hidden", background: t.bg, boxShadow: `inset 0 0 0 1px ${t.rule}`, fontFamily: window.WR_SANS }}>
      <div style={{ position: "absolute", inset: 0, backgroundImage: window.crtOverlay(t), mixBlendMode: "overlay", pointerEvents: "none", zIndex: 5 }} />
      {/* status bar */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "9px 18px 0", fontFamily: window.WR_MONO, fontSize: 10, color: t.fg }}>
        <span>{window.nowClock()}</span><span>▮▮▮ ▾ 􀙇</span>
      </div>
      <div style={{ position: "relative", zIndex: 2, padding: "16px 18px", display: "flex", flexDirection: "column", height: "100%" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <window.WRMark t={t} size={13} />
          <span style={{ fontFamily: window.WR_MONO, fontSize: 10, color: t.dim }}>MINI</span>
          <span style={{ marginLeft: "auto" }}><window.StateDot t={t} state={state} /></span>
        </div>
        <div style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center", gap: 14 }}>
          <AndroidArt t={t} size={104} />
          <div style={{ textAlign: "center" }}>
            <div style={{ fontWeight: 600, fontSize: 16, color: t.bright }}>Smooth Jazz Café</div>
            <div style={{ fontFamily: window.WR_MONO, fontSize: 11, color: t.accent, marginTop: 3 }}>♪ Chuck Wayne — What A…</div>
            <div style={{ fontFamily: window.WR_MONO, fontSize: 9.5, color: t.dim, marginTop: 4 }}>🇲🇽 MX · AAC 48k</div>
          </div>
          <window.Spectrum t={t} active={isPlaying} bars={26} height={20} />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
          <div style={{ display: "flex", alignSelf: "center", gap: 2, padding: 3, borderRadius: 7, boxShadow: `inset 0 0 0 1px ${t.rule}` }}>
            <span style={{ padding: "4px 12px", borderRadius: 4, fontFamily: window.WR_MONO, fontSize: 10, fontWeight: scope === "all" ? 700 : 400, color: scope === "all" ? t.bg : t.dim, background: scope === "all" ? t.hi : "transparent" }}>ALL</span>
            <span style={{ padding: "4px 12px", borderRadius: 4, fontFamily: window.WR_MONO, fontSize: 10, fontWeight: scope === "favs" ? 700 : 400, color: scope === "favs" ? t.bg : t.dim, background: scope === "favs" ? t.hi : "transparent" }}>★ FAVS</span>
          </div>
          <div style={{ display: "flex", gap: 9, alignItems: "center" }}>
            <div style={{ flex: 1, height: 46, borderRadius: 11, background: t.hi, color: t.bg, display: "flex", alignItems: "center", justifyContent: "center", gap: 7, fontFamily: window.WR_MONO, fontWeight: 700, fontSize: 14, boxShadow: t.light ? "none" : `0 0 16px -3px ${t.glow}` }}>⇄ SHUFFLE</div>
            <div style={{ width: 46, height: 46, borderRadius: 11, boxShadow: `inset 0 0 0 1px ${t.rule}`, display: "flex", alignItems: "center", justifyContent: "center", color: t.fg, fontSize: 17 }}>{isPlaying ? "⏸" : "▶"}</div>
          </div>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { AndroidNotif, AndroidWidget, AndroidCompact, AndroidArt });
