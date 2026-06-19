/* MiniWindow — the core ~260px CRT panel reused by every platform.
   Props: themeKey, state ('playing'|'stopped'|'buffering'|'error'),
          scope ('all'|'favs'), w (width, default 260), compact (bool).
   Exports to window: MiniWindow, WRMark, Spectrum, fontStack, crtOverlay. */

const WR_MONO = '"IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace';
const WR_SANS = '"IBM Plex Sans", system-ui, sans-serif';

/* scanline + vignette overlay tuned per theme */
function crtOverlay(t) {
  if (t.scan <= 0.001) return "none";
  return `repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0, rgba(0,0,0,0) 2px, rgba(0,0,0,${t.scan}) 2px, rgba(0,0,0,${t.scan}) 3px)`;
}

/* ── mini FFT spectrum, block glyphs, animated only when playing ── */
function Spectrum({ t, bars = 14, active, height = 16, dim = false }) {
  // deterministic-ish heights so SSR/screenshot is stable, animated via CSS
  const seed = [5, 7, 4, 8, 6, 3, 7, 5, 8, 4, 6, 7, 3, 5];
  return (
    <div style={{ display: "flex", alignItems: "flex-end", gap: 1, height }}>
      {Array.from({ length: bars }).map((_, i) => {
        const base = seed[i % seed.length];
        const h = active ? (base / 8) * height : Math.max(2, (base / 8) * height * 0.12);
        return (
          <span
            key={i}
            className={active ? "wr-spec-bar" : ""}
            style={{
              width: 3,
              height: h,
              background: dim ? t.dim : t.hi,
              opacity: active ? 1 : 0.4,
              borderRadius: 0.5,
              animationDelay: `${(i % 7) * 90}ms`,
              "--wr-spec-max": `${(base / 8) * height}px`,
              "--wr-spec-min": `${Math.max(2, (base / 8) * height * 0.28)}px`,
            }}
          />
        );
      })}
    </div>
  );
}

/* ── ▌WR block marker ── */
function WRMark({ t, size = 12 }) {
  return (
    <span style={{ fontFamily: WR_MONO, fontWeight: 700, fontSize: size, letterSpacing: "0.04em", color: t.hi, lineHeight: 1, whiteSpace: "nowrap" }}>
      <span style={{ marginRight: 1 }}>▌</span>WR
    </span>
  );
}

function VolBar({ t, level = 0.6, muted = false }) {
  const seg = 6;
  const on = Math.round(level * seg);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
      <span style={{ fontSize: 9, color: t.dim, fontFamily: WR_MONO }}>{muted ? "🔇" : "VOL"}</span>
      <div style={{ display: "flex", gap: 1.5 }}>
        {Array.from({ length: seg }).map((_, i) => (
          <span key={i} style={{ width: 3.5, height: 9, borderRadius: 0.5, background: i < on && !muted ? t.hi : "transparent", boxShadow: `inset 0 0 0 1px ${i < on && !muted ? t.hi : t.dim}` }} />
        ))}
      </div>
    </div>
  );
}

function StateDot({ t, state }) {
  const map = {
    playing: { c: t.ok, label: "LIVE", pulse: true },
    stopped: { c: t.dim, label: "IDLE", pulse: false },
    buffering: { c: t.warn, label: "···", pulse: true },
    error: { c: t.err, label: "OFFLINE", pulse: false },
  };
  const s = map[state];
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 5, fontFamily: WR_MONO, fontSize: 9.5, letterSpacing: "0.1em", color: s.c }}>
      <span className={s.pulse ? "wr-pulse" : ""} style={{ width: 6, height: 6, borderRadius: "50%", background: s.c, boxShadow: state === "playing" ? `0 0 6px ${s.c}` : "none" }} />
      {s.label}
    </span>
  );
}

/* primary shuffle + scope segmented toggle */
function ShuffleScope({ t, scope }) {
  const seg = (key, label) => {
    const on = scope === key;
    return (
      <span style={{ padding: "2px 6px", fontFamily: WR_MONO, fontSize: 9, letterSpacing: "0.04em", borderRadius: 3, color: on ? t.bg : t.dim, background: on ? t.hi : "transparent", fontWeight: on ? 700 : 400 }}>{label}</span>
    );
  };
  return (
    <div style={{ display: "inline-flex", alignItems: "center", gap: 2, padding: 2, borderRadius: 5, boxShadow: `inset 0 0 0 1px ${t.rule}` }}>
      {seg("all", "ALL")}
      {seg("favs", "★ FAVS")}
    </div>
  );
}

function CtrlBtn({ t, children, primary, wide, title }) {
  return (
    <button
      title={title}
      style={{
        appearance: "none", cursor: "pointer", fontFamily: WR_MONO, fontWeight: primary ? 700 : 500,
        fontSize: primary ? 12 : 13, letterSpacing: primary ? "0.06em" : "0", lineHeight: 1,
        padding: primary ? "8px 12px" : "7px 9px", borderRadius: 5,
        flex: wide ? 1 : "none", display: "inline-flex", alignItems: "center", justifyContent: "center", gap: 6,
        color: primary ? t.bg : t.fg, background: primary ? t.hi : "transparent",
        border: "none", boxShadow: primary ? `0 0 10px -2px ${t.glow === "transparent" ? "rgba(0,0,0,0)" : t.glow}` : `inset 0 0 0 1px ${t.rule}`,
        whiteSpace: "nowrap",
      }}
    >{children}</button>
  );
}

function MiniWindow({ themeKey = "amber", state = "playing", scope = "all", w = 260, radius = 12 }) {
  const t = window.WR_THEMES[themeKey];
  const isPlaying = state === "playing";
  const isBuffering = state === "buffering";
  const isError = state === "error";
  const isStopped = state === "stopped";

  // content per state
  const station = isStopped ? "Nothing playing" : isBuffering ? "Pure Lounge Radio" : isError ? "Sapporo Jazz" : "Smooth Jazz Café";
  const nowText = isPlaying ? "Chuck Wayne — What A Difference A Day Made" : isBuffering ? "connecting to stream…" : isError ? "stream offline — couldn't connect" : "press Shuffle to start listening";
  const meta = isStopped ? "—" : isError ? "🇯🇵 JP" : isBuffering ? "🇫🇷 FR · MP3 256k" : "🇲🇽 MX · AAC 48k";

  const nowColor = isError ? t.err : isBuffering ? t.warn : isStopped ? t.dim : t.accent;
  const playGlyph = isPlaying ? "⏸" : isBuffering ? "⏸" : "▶";

  return (
    <div
      style={{
        width: w, boxSizing: "border-box", position: "relative", borderRadius: radius, overflow: "hidden",
        background: t.bg, color: t.fg, fontFamily: WR_MONO,
        boxShadow: t.light ? "inset 0 0 0 1px rgba(0,0,0,0.12)" : `inset 0 0 0 1px ${t.rule}, 0 0 0 1px rgba(0,0,0,0.4)`,
        padding: "11px 13px 12px",
      }}
    >
      {/* scanline overlay */}
      <div style={{ position: "absolute", inset: 0, backgroundImage: crtOverlay(t), pointerEvents: "none", mixBlendMode: "overlay", zIndex: 3 }} />
      {!t.light && (
        <div style={{ position: "absolute", inset: 0, background: `radial-gradient(120% 90% at 50% 0%, ${t.glow === "transparent" ? "rgba(0,0,0,0)" : t.glow}22, transparent 70%)`, pointerEvents: "none", zIndex: 1 }} />
      )}

      <div style={{ position: "relative", zIndex: 4, display: "flex", flexDirection: "column", gap: 7 }}>
        {/* header */}
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <WRMark t={t} size={11} />
          <StateDot t={t} state={state} />
          <span style={{ marginLeft: "auto", fontFamily: WR_MONO, fontSize: 9, color: t.dim, whiteSpace: "nowrap" }}>{meta}</span>
        </div>

        {/* station + now playing */}
        <div style={{ minWidth: 0 }}>
          <div style={{ fontFamily: WR_SANS, fontWeight: 600, fontSize: 13.5, color: isStopped ? t.dim : t.bright, lineHeight: 1.15, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", textShadow: !t.light && isPlaying ? `0 0 8px ${t.glow}55` : "none" }}>
            {station}
          </div>
          <div style={{ marginTop: 2, height: 14, overflow: "hidden", position: "relative" }}>
            <div className={isPlaying ? "wr-marquee" : ""} style={{ fontFamily: WR_MONO, fontSize: 10.5, color: nowColor, whiteSpace: "nowrap", lineHeight: "14px" }}>
              {isPlaying && <span style={{ marginRight: 6 }}>♪</span>}
              {isBuffering && <span className="wr-pulse" style={{ marginRight: 6 }}>⏳</span>}
              {isError && <span style={{ marginRight: 6 }}>✗</span>}
              {nowText}
              {isPlaying && <span style={{ paddingLeft: 40 }}>♪ {nowText}</span>}
            </div>
          </div>
        </div>

        {/* spectrum + volume */}
        <div style={{ display: "flex", alignItems: "flex-end", justifyContent: "space-between", gap: 8, opacity: isError ? 0.4 : 1 }}>
          <Spectrum t={t} active={isPlaying} dim={isStopped || isError} bars={16} height={16} />
          <VolBar t={t} level={0.6} muted={isError} />
        </div>

        {/* controls */}
        <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
          <CtrlBtn t={t} primary wide title="Shuffle a station">
            <span style={{ fontSize: 13 }}>⇄</span> {isError ? "RETRY" : "SHUFFLE"}
          </CtrlBtn>
          <CtrlBtn t={t} title={isPlaying ? "Stop" : "Play"}>{playGlyph}</CtrlBtn>
        </div>

        {/* scope */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <ShuffleScope t={t} scope={scope} />
          <span style={{ fontFamily: WR_MONO, fontSize: 9, color: t.dim }}>shuffle scope</span>
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { MiniWindow, WRMark, Spectrum, VolBar, StateDot, ShuffleScope, CtrlBtn, WR_MONO, WR_SANS, crtOverlay });
