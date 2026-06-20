/* r4dio.net — logo exploration. Built on the World Radio brand system:
   IBM Plex Mono/Sans, amber CRT palette, the ▌ block-cursor glyph and the
   text-spectrum motif. The "4" replaces the "a" of radio (the domain hook),
   carried in amber-hi as the single spark across every lockup. */

const R4 = {
  bg:'#14110d', bgSoft:'#191510', panel:'#1b1610', panelHi:'#211b13',
  fg:'#cfc4ad', fgBright:'#ece2cb', mute:'#8a7f64', dim:'#6f6650',
  rule:'#332e25', ruleSoft:'#262119',
  amber:'#d49a3a', amberHi:'#ffc457', amberDeep:'#a8762a', olive:'#9ec074',
  paper:'#efe6cc', paperInk:'#241c10', paperDim:'#8a7a5a', paperAmber:'#b87422',
  mono:'"IBM Plex Mono", ui-monospace, "SF Mono", Menlo, monospace',
  sans:'"IBM Plex Sans", system-ui, sans-serif',
};

const glow = 'radial-gradient(120% 90% at 50% 8%, rgba(212,154,58,.10), transparent 62%)';

// Centered dark stage used by most artboards.
function Stage({ children, bg = R4.bg, glowOn = true, style = {} }) {
  return (
    <div style={{ width:'100%', height:'100%', background:bg, backgroundImage: glowOn ? glow : 'none',
      display:'flex', alignItems:'center', justifyContent:'center', position:'relative', ...style }}>
      {children}
    </div>
  );
}

// The wordmark itself — r4dio with the amber "4". Optional ▌ cursor + .net tld.
function Mark({ size = 64, cursor = true, tld = false, ink = R4.fgBright, accent = R4.amberHi,
                cursorColor = R4.amber, weight = 600, tldColor = R4.dim }) {
  return (
    <span style={{ fontFamily:R4.mono, fontWeight:weight, fontSize:size, letterSpacing:'-.015em',
      color:ink, whiteSpace:'nowrap', lineHeight:1, display:'inline-flex', alignItems:'baseline' }}>
      {cursor && <span style={{ color:cursorColor, marginRight:'.1em', fontWeight:700 }}>▌</span>}
      <span>r<span style={{ color:accent }}>4</span>dio</span>
      {tld && <span style={{ color:tldColor, fontWeight:500, fontSize:'.92em' }}>.net</span>}
    </span>
  );
}

// Flex-div spectrum bars (crisper than glyphs at icon scale).
function Bars({ heights = [.4,.7,.95,.55,.8,.45,.65], bw = 5, gap = 4, max = 46, color = R4.amber, animate = false }) {
  return (
    <div style={{ display:'flex', alignItems:'flex-end', gap }}>
      {heights.map((v,i)=>(
        <div key={i} className={animate ? 'r4-eq' : ''} style={{ width:bw, height:Math.round(max*v),
          background:color, borderRadius:1, animationDelay: animate ? `${i*0.08}s` : undefined }} />
      ))}
    </div>
  );
}

const cap = (t) => (
  <div style={{ fontFamily:R4.mono, fontSize:11, letterSpacing:'.12em', textTransform:'uppercase',
    color:R4.dim }}>{t}</div>
);

/* ───────────────────────── App-icon tiles ───────────────────────── */

function IconCursor({ s = 1 }) {
  return (
    <div style={{ width:160*s, height:160*s, borderRadius:34*s, background:R4.bg, position:'relative',
      boxShadow:`inset 0 0 0 ${1.5*s}px ${R4.rule}`, display:'flex', alignItems:'center', justifyContent:'center',
      backgroundImage:'radial-gradient(110% 80% at 50% 0%, rgba(212,154,58,.16), transparent 60%)' }}>
      <span style={{ fontFamily:R4.mono, fontWeight:700, fontSize:72*s, lineHeight:1, letterSpacing:'-.02em',
        color:R4.fgBright, display:'inline-flex', alignItems:'center' }}>
        <span style={{ color:R4.amber }}>▌</span>r<span style={{ color:R4.amberHi }}>4</span>
      </span>
    </div>
  );
}

function IconSpectrum({ s = 1 }) {
  return (
    <div style={{ width:160*s, height:160*s, borderRadius:34*s, background:R4.bg, position:'relative', overflow:'hidden',
      boxShadow:`inset 0 0 0 ${1.5*s}px ${R4.rule}`, display:'flex', flexDirection:'column',
      alignItems:'center', justifyContent:'center', gap:14*s }}>
      <span style={{ fontFamily:R4.mono, fontWeight:700, fontSize:84*s, lineHeight:.9, color:R4.amberHi }}>4</span>
      <Bars heights={[.45,.8,1,.6,.9,.5]} bw={7*s} gap={5*s} max={26*s} color={R4.amber} />
    </div>
  );
}

function IconMono({ s = 1 }) {
  return (
    <div style={{ width:160*s, height:160*s, borderRadius:34*s, background:R4.amber, position:'relative',
      display:'flex', alignItems:'center', justifyContent:'center',
      boxShadow:`inset 0 ${2*s}px 0 rgba(255,255,255,.18), inset 0 ${-3*s}px ${8*s}px rgba(0,0,0,.22)` }}>
      <span style={{ fontFamily:R4.mono, fontWeight:700, fontSize:74*s, lineHeight:1, letterSpacing:'-.02em',
        color:R4.bg }}>r<span style={{ color:R4.panel }}>4</span></span>
    </div>
  );
}

function IconPaper({ s = 1 }) {
  return (
    <div style={{ width:160*s, height:160*s, borderRadius:34*s, background:R4.paper, position:'relative',
      display:'flex', alignItems:'center', justifyContent:'center',
      boxShadow:`inset 0 0 0 ${1.5*s}px rgba(0,0,0,.10)` }}>
      <span style={{ fontFamily:R4.mono, fontWeight:700, fontSize:72*s, lineHeight:1, letterSpacing:'-.02em',
        color:R4.paperInk, display:'inline-flex', alignItems:'center' }}>
        <span style={{ color:R4.paperAmber }}>▌</span>r<span style={{ color:R4.paperAmber }}>4</span>
      </span>
    </div>
  );
}

/* ───────────────────────── In-context ───────────────────────── */

function NavContext() {
  return (
    <div style={{ width:'100%', height:'100%', background:R4.bg, display:'flex', flexDirection:'column' }}>
      <div style={{ height:60, borderBottom:`1px solid ${R4.rule}`, display:'flex', alignItems:'center',
        gap:26, padding:'0 26px', background:'rgba(20,17,13,.85)' }}>
        <Mark size={20} tld={true} />
        <div style={{ marginLeft:'auto', display:'flex', gap:24, fontFamily:R4.mono, fontSize:12.5, color:R4.mute }}>
          <span>features</span><span>design</span><span style={{ color:R4.fgBright }}>download</span>
          <span style={{ border:`1px solid ${R4.rule}`, padding:'6px 12px', borderRadius:4, color:R4.fgBright }}>github ↗</span>
        </div>
      </div>
      <div style={{ flex:1, display:'flex', alignItems:'center', padding:'0 26px' }}>
        {cap('top navigation lockup · ▌r4dio.net')}
      </div>
    </div>
  );
}

function BrowserContext() {
  return (
    <Stage glowOn={false} style={{ flexDirection:'column', gap:0, alignItems:'stretch', padding:0 }}>
      <div style={{ flex:1 }} />
      <div style={{ padding:'0 30px' }}>
        {/* tab */}
        <div style={{ display:'flex', alignItems:'flex-end' }}>
          <div style={{ display:'inline-flex', alignItems:'center', gap:9, background:R4.panel,
            border:`1px solid ${R4.rule}`, borderBottom:'none', borderRadius:'9px 9px 0 0', padding:'9px 14px' }}>
            <span style={{ width:16, height:16, borderRadius:4, background:R4.bg, display:'inline-flex',
              alignItems:'center', justifyContent:'center', fontFamily:R4.mono, fontWeight:700, fontSize:10.5,
              color:R4.amberHi, boxShadow:`inset 0 0 0 1px ${R4.rule}` }}>4</span>
            <span style={{ fontFamily:R4.mono, fontSize:12.5, color:R4.fgBright }}>r4dio.net</span>
            <span style={{ color:R4.dim, marginLeft:2 }}>×</span>
          </div>
        </div>
        {/* url bar */}
        <div style={{ display:'flex', alignItems:'center', gap:10, background:R4.bgSoft,
          border:`1px solid ${R4.rule}`, borderRadius:'0 8px 8px 8px', padding:'10px 14px' }}>
          <span style={{ color:R4.olive, fontSize:12 }}>▲</span>
          <span style={{ fontFamily:R4.mono, fontSize:13, color:R4.fgBright }}>
            r<span style={{ color:R4.amberHi }}>4</span>dio.net<span style={{ color:R4.dim }}>/download</span>
          </span>
        </div>
      </div>
      <div style={{ flex:1 }} />
      <div style={{ padding:'0 30px 22px' }}>{cap('favicon + address bar')}</div>
    </Stage>
  );
}

function TerminalContext() {
  return (
    <Stage glowOn={false} style={{ flexDirection:'column', alignItems:'stretch', padding:30, gap:0 }}>
      <div style={{ flex:1 }} />
      <div style={{ border:`1px solid ${R4.rule}`, borderRadius:8, overflow:'hidden' }}>
        <div style={{ display:'flex', alignItems:'center', gap:9, background:R4.bgSoft, padding:'11px 14px',
          borderBottom:`1px solid ${R4.rule}` }}>
          <span style={{ display:'flex', gap:6 }}>
            <i style={{ width:10, height:10, borderRadius:5, background:'#c2553f', display:'block' }} />
            <i style={{ width:10, height:10, borderRadius:5, background:'#d49a3a', display:'block' }} />
            <i style={{ width:10, height:10, borderRadius:5, background:'#7f9a4e', display:'block' }} />
          </span>
          <span style={{ fontFamily:R4.mono, fontSize:11.5, color:R4.dim, marginLeft:6 }}>r4dio — 118×34</span>
        </div>
        <div style={{ background:R4.bg, padding:'16px 16px 18px', fontFamily:R4.mono, fontSize:13 }}>
          <span style={{ color:R4.fgBright }}>
            <span style={{ color:R4.amber, fontWeight:700 }}>▌</span>r<span style={{ color:R4.amberHi }}>4</span>dio
          </span>
          <span style={{ color:R4.dim }}>  ·  </span>
          <span style={{ color:R4.olive }}>●</span> <span style={{ color:R4.amberHi }}>LIVE</span>
          <span style={{ color:R4.dim }}>   Smooth Jazz Café   </span>
          <span style={{ display:'inline-block', verticalAlign:'middle', marginLeft:4 }}>
            <Bars heights={[.3,.6,.9,.5,.8,.4,.7,.5]} bw={3} gap={2} max={14} color={R4.amber} animate />
          </span>
        </div>
      </div>
      <div style={{ flex:1 }} />
      <div style={{ paddingTop:18 }}>{cap('in-app header glyph')}</div>
    </Stage>
  );
}

function FaviconScales() {
  const Tile = ({ px }) => (
    <div style={{ display:'flex', flexDirection:'column', alignItems:'center', gap:10 }}>
      <div style={{ width:px, height:px, borderRadius:Math.max(3, px*0.22), background:R4.bg,
        boxShadow:`inset 0 0 0 1px ${R4.rule}`, display:'flex', alignItems:'center', justifyContent:'center' }}>
        <span style={{ fontFamily:R4.mono, fontWeight:700, fontSize:px*0.62, lineHeight:1, color:R4.amberHi }}>4</span>
      </div>
      <span style={{ fontFamily:R4.mono, fontSize:11, color:R4.dim }}>{px}px</span>
    </div>
  );
  return (
    <Stage>
      <div style={{ display:'flex', alignItems:'flex-end', gap:34 }}>
        <Tile px={64} /><Tile px={32} /><Tile px={16} />
      </div>
    </Stage>
  );
}

/* ───────────────────────── Canvas ───────────────────────── */

function App() {
  return (
    <DesignCanvas>

      <DCSection id="wordmark" title="Primary wordmark" subtitle="r4dio — the 4 replaces the a; amber-hi is the single accent">
        <DCArtboard id="cursor" label="A · Cursor lockup  ★ recommended" width={560} height={240}>
          <Stage style={{ flexDirection:'column', gap:18 }}>
            <Mark size={76} cursor={true} />
            <div style={{ fontFamily:R4.mono, fontSize:13, letterSpacing:'.34em', textTransform:'uppercase', color:R4.mute }}>
              world&nbsp;&nbsp;radio
            </div>
          </Stage>
        </DCArtboard>

        <DCArtboard id="domain" label="B · Domain lockup" width={560} height={240}>
          <Stage>
            <span style={{ fontFamily:R4.mono, fontWeight:600, fontSize:74, letterSpacing:'-.015em', color:R4.fgBright }}>
              r<span style={{ color:R4.amberHi }}>4</span>dio<span style={{ color:R4.amber, fontWeight:500 }}>.net</span>
            </span>
          </Stage>
        </DCArtboard>

        <DCArtboard id="spectrum" label="C · Spectrum lockup" width={560} height={240}>
          <Stage>
            <div style={{ display:'flex', alignItems:'center', gap:22 }}>
              <Mark size={70} cursor={false} />
              <Bars heights={[.35,.7,1,.55,.85,.45,.75,.6]} bw={6} gap={5} max={56} color={R4.amber} animate />
            </div>
          </Stage>
        </DCArtboard>

        <DCArtboard id="prompt" label="D · Prompt lockup" width={560} height={240}>
          <Stage>
            <span style={{ fontFamily:R4.mono, fontWeight:600, fontSize:60, letterSpacing:'-.01em', color:R4.fgBright,
              display:'inline-flex', alignItems:'center' }}>
              <span style={{ color:R4.amber, marginRight:'.4em', fontWeight:500 }}>~&nbsp;$</span>
              r<span style={{ color:R4.amberHi }}>4</span>dio
              <span className="r4-cursor" style={{ color:R4.amberHi, marginLeft:'.12em' }}>▌</span>
            </span>
          </Stage>
        </DCArtboard>
      </DCSection>

      <DCSection id="icon" title="App icon / mark" subtitle="Square tile for favicon, dock, package badge — works down to 16px">
        <DCArtboard id="i-cursor" label="1 · ▌r4 mark  ★ recommended" width={300} height={300}>
          <Stage><IconCursor /></Stage>
        </DCArtboard>
        <DCArtboard id="i-spectrum" label="2 · 4 + spectrum" width={300} height={300}>
          <Stage><IconSpectrum /></Stage>
        </DCArtboard>
        <DCArtboard id="i-mono" label="3 · Amber tile" width={300} height={300}>
          <Stage glowOn={false}><IconMono /></Stage>
        </DCArtboard>
        <DCArtboard id="i-paper" label="4 · Hi-Fi Paper" width={300} height={300}>
          <Stage glowOn={false} bg={R4.bgSoft}><IconPaper /></Stage>
        </DCArtboard>
      </DCSection>

      <DCSection id="context" title="In context" subtitle="The mark living inside the product and the site">
        <DCArtboard id="nav" label="Site navigation" width={640} height={220}>
          <NavContext />
        </DCArtboard>
        <DCArtboard id="browser" label="Browser tab + URL" width={420} height={260}>
          <BrowserContext />
        </DCArtboard>
        <DCArtboard id="terminal" label="In-app header" width={560} height={260}>
          <TerminalContext />
        </DCArtboard>
        <DCArtboard id="favicon" label="Favicon scales" width={360} height={260}>
          <FaviconScales />
        </DCArtboard>
      </DCSection>

    </DesignCanvas>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
