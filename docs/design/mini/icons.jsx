/* Icon set — tray/menu-bar glyphs (idle/playing/buffering), mac monochrome
   template, colored variants, and launcher icons.
   Exports: WRTrayGlyph, LauncherIcon, IconPlate. */

/* core tray mark: the ▌ block bar + broadcast waves; equalizer when playing.
   Drawn on a 24×24 grid, scaled by `size`. Monochrome (uses currentColor via `color`). */
function WRTrayGlyph({ color = "#000", size = 16, state = "idle", colored = false, themeKey = "amber" }) {
  const t = window.WR_THEMES[themeKey];
  const c = colored ? t.hi : color;
  const wave = colored ? t.hi : color;
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" style={{ display: "block" }}>
      {/* block bar — the ▌ marker */}
      <rect x="3" y="5" width="3.4" height="14" rx="0.6" fill={c} />
      {state === "playing" ? (
        // equalizer bars
        <g fill={c}>
          <rect className="wr-eq" x="9" y="9" width="2.6" height="10" rx="0.6" style={{ transformOrigin: "10.3px 19px", animationDelay: "0ms" }} />
          <rect className="wr-eq" x="13" y="6" width="2.6" height="13" rx="0.6" style={{ transformOrigin: "14.3px 19px", animationDelay: "180ms" }} />
          <rect className="wr-eq" x="17" y="10" width="2.6" height="9" rx="0.6" style={{ transformOrigin: "18.3px 19px", animationDelay: "90ms" }} />
        </g>
      ) : state === "buffering" ? (
        <g fill={c}>
          <circle cx="10.5" cy="12" r="1.4" className="wr-pulse" style={{ animationDelay: "0ms" }} />
          <circle cx="14.5" cy="12" r="1.4" className="wr-pulse" style={{ animationDelay: "200ms" }} />
          <circle cx="18.5" cy="12" r="1.4" className="wr-pulse" style={{ animationDelay: "400ms" }} />
        </g>
      ) : (
        // idle: broadcast arcs
        <g stroke={wave} strokeWidth="1.8" fill="none" strokeLinecap="round" opacity="0.92">
          <path d="M10.5 8.5 A 5.5 5.5 0 0 1 10.5 15.5" />
          <path d="M13.8 6.2 A 9 9 0 0 1 13.8 17.8" opacity="0.62" />
        </g>
      )}
    </svg>
  );
}

/* launcher / app icon — rounded squircle, themed, glowing ▌WR over scanlines */
function LauncherIcon({ themeKey = "amber", size = 88, radius = 20 }) {
  const t = window.WR_THEMES[themeKey];
  return (
    <div style={{ width: size, height: size, borderRadius: radius, position: "relative", overflow: "hidden", background: t.bg, boxShadow: `inset 0 0 0 1px ${t.rule}, 0 8px 22px -8px rgba(0,0,0,0.6)` }}>
      <div style={{ position: "absolute", inset: 0, background: `radial-gradient(110% 90% at 50% 18%, ${t.glow === "transparent" ? t.hi : t.glow}33, transparent 68%)` }} />
      <div style={{ position: "absolute", inset: 0, backgroundImage: window.crtOverlay(t), mixBlendMode: "overlay", opacity: 0.8 }} />
      <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center" }}>
        <span style={{ fontFamily: window.WR_MONO, fontWeight: 700, fontSize: size * 0.32, letterSpacing: "0.02em", color: t.hi, textShadow: t.light ? "none" : `0 0 ${size * 0.12}px ${t.glow}` }}>
          <span style={{ marginRight: size * 0.01 }}>▌</span>WR
        </span>
      </div>
    </div>
  );
}

/* labeled plate for the icon-grid presentation */
function IconPlate({ label, sub, bg = "#1b1510", children, light = false }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 8 }}>
      <div style={{ width: 72, height: 72, borderRadius: 12, background: bg, display: "flex", alignItems: "center", justifyContent: "center", boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.07)" }}>
        {children}
      </div>
      <div style={{ textAlign: "center", lineHeight: 1.3 }}>
        <div style={{ fontFamily: window.WR_MONO, fontSize: 10.5, color: light ? "#2e2517" : "#cfc4ad" }}>{label}</div>
        {sub && <div style={{ fontFamily: window.WR_MONO, fontSize: 9, color: "#8a7f64" }}>{sub}</div>}
      </div>
    </div>
  );
}

Object.assign(window, { WRTrayGlyph, LauncherIcon, IconPlate });
