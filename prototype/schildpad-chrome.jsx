/* schildpad-chrome.jsx — transport cluster, message/help surface, status bar, on-screen keyboard. */

// ---------- transport ----------------------------------------------------
function ChunkySvg({ children, size = 30 }) {
  return <svg width={size} height={size} viewBox="0 0 24 24" style={{ display: "block" }}>{children}</svg>;
}
const Icons = {
  step: <ChunkySvg><rect x="4" y="4" width="3" height="16" fill="currentColor" /><polygon points="9,4 20,12 9,20" fill="currentColor" /></ChunkySvg>,
  play: <ChunkySvg><polygon points="6,4 20,12 6,20" fill="currentColor" /></ChunkySvg>,
  pause: <ChunkySvg><rect x="5" y="4" width="5" height="16" fill="currentColor" /><rect x="14" y="4" width="5" height="16" fill="currentColor" /></ChunkySvg>,
  loop: <ChunkySvg><path d="M5 8 h9 v-3 l5 5 -5 5 v-3 H7 v4 H4 V8 z" fill="currentColor" /><rect x="4" y="14" width="3" height="6" fill="currentColor" /><path d="M19 16 h-9 v3 l-5 -5 0 0" fill="none" /><polygon points="19,16 10,16 10,13 5,18 10,23 10,20 19,20" fill="currentColor" /></ChunkySvg>,
};

function TransportButton({ icon, label, sub, active, accent, onClick }) {
  const [hover, setHover] = React.useState(false);
  return (
    <button onClick={onClick} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}
      style={{
        flex: 1, height: 64, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center",
        gap: 2, border: "1px solid " + (active ? accent : "#26282b"), borderRadius: 4,
        background: active ? accent + "22" : (hover ? "#171a1c" : "#101113"),
        color: active ? accent : "#aeb6bd", cursor: "pointer", transition: "all .08s",
        boxShadow: active ? `inset 0 0 0 1px ${accent}` : "none", fontFamily: "'IBM Plex Mono', monospace",
      }}>
      <div style={{ color: active ? accent : "#cfd6dc" }}>{icon}</div>
      <div style={{ fontSize: 11, letterSpacing: ".06em", textTransform: "uppercase", fontWeight: 600 }}>{label}</div>
    </button>
  );
}

function Transport({ state, speed, onSpeed, onStep, onPlay, onPause, onLoop }) {
  const playing = state === "playing";
  const looping = state === "looping";
  const paused = state === "paused" || state === "error";
  const stepping = state === "stepping";
  const labels = { idle: "klaar", playing: "speelt af", paused: "gepauzeerd", looping: "lus draait", stepping: "stap", error: "gestopt", done: "klaar" };
  const accent = state === "error" ? "#e0746c" : "#5cd6f0";
  return (
    <div style={{ padding: "10px 12px 12px", borderTop: "1px solid #1b1c1e", background: "#0a0a0b" }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8, padding: "0 2px" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 11 }}>
          <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, letterSpacing: ".18em", textTransform: "uppercase", color: "#52595f" }}>transport</span>
          <div style={{ display: "flex", border: "1px solid #26282b", borderRadius: 4, overflow: "hidden" }}>
            {["traag", "snel"].map((o) => (
              <button key={o} onClick={() => onSpeed(o)} style={{
                padding: "3px 11px", fontSize: 11, fontWeight: 600, letterSpacing: ".04em",
                fontFamily: "'IBM Plex Mono', monospace", border: "none", cursor: "pointer",
                background: speed === o ? "#172024" : "transparent", color: speed === o ? "#5cd6f0" : "#5b646b",
              }}>{o}</button>
            ))}
          </div>
        </div>
        <span style={{ display: "flex", alignItems: "center", gap: 7, fontFamily: "'IBM Plex Mono', monospace", fontSize: 12, color: accent }}>
          <span style={{ width: 8, height: 8, borderRadius: 8, background: accent, boxShadow: `0 0 8px ${accent}`,
            animation: (playing || looping) ? "sp-blink 1s steps(2) infinite" : "none" }} />
          {labels[state] || state}
        </span>
      </div>
      <div style={{ display: "flex", gap: 8 }}>
        <TransportButton icon={Icons.step} label="stap" active={stepping} accent="#5cd6f0" onClick={onStep} />
        <TransportButton icon={Icons.play} label="speel" active={playing} accent="#34e24b" onClick={onPlay} />
        <TransportButton icon={Icons.pause} label="pauze" active={paused} accent="#ffd23a" onClick={onPause} />
        <TransportButton icon={Icons.loop} label="lus" active={looping} accent="#b15cff" onClick={onLoop} />
      </div>
    </div>
  );
}

// ---------- message / help surface ---------------------------------------
function MessageBar({ mode, errorText, errorLine, suggestion, onInsert }) {
  if (mode === "error") {
    return (
      <div style={{ borderTop: "1px solid #1b1c1e", background: "#0e0c0c", padding: "12px 14px", minHeight: 96, display: "flex", gap: 12 }}>
        <div style={{ width: 3, alignSelf: "stretch", background: "#e0746c", borderRadius: 2, flexShrink: 0 }} />
        <div style={{ flex: 1 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
            <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 10.5, letterSpacing: ".16em", textTransform: "uppercase", color: "#a85852", whiteSpace: "nowrap" }}>de machine licht toe</span>
            {errorLine != null && <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 11, color: "#e0746c", border: "1px solid #5a2f2c", borderRadius: 3, padding: "1px 6px", whiteSpace: "nowrap" }}>regel {errorLine}</span>}
          </div>
          <div style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 14.5, lineHeight: "21px", color: "#e7d3d0", textWrap: "pretty" }}>{errorText}</div>
        </div>
      </div>
    );
  }
  // help / ambient mode
  return (
    <div style={{ borderTop: "1px solid #1b1c1e", background: "#0b0b0c", padding: "12px 14px", minHeight: 96 }}>
      <div style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 10.5, letterSpacing: ".16em", textTransform: "uppercase", color: "#4b5258", marginBottom: 8 }}>
        {suggestion ? suggestion.title : "maak iets!"}
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", gap: 7 }}>
        {(suggestion ? suggestion.chips : [
          { label: "schildpad", insert: "schildpad" }, { label: "getal", insert: "getal" }, { label: "draairichting", insert: "draairichting" },
        ]).map((c, i) => (
          <button key={i} onClick={() => onInsert(c.insert)} style={{
            fontFamily: "'IBM Plex Mono', monospace", fontSize: 13, color: c.color || "#bcc4cb",
            background: "#141618", border: "1px solid #25282b", borderRadius: 4, padding: "5px 10px", cursor: "pointer",
          }}>{c.label}</button>
        ))}
      </div>
    </div>
  );
}

// ---------- oscilloscope -------------------------------------------------
function Oscilloscope({ label, analyser, active, accent }) {
  const ref = React.useRef(null);
  React.useEffect(() => {
    let raf; const c = ref.current; if (!c) return;
    const ctx = c.getContext("2d");
    const W = c.width, H = c.height;
    const buf = analyser ? new Uint8Array(analyser.frequencyBinCount) : null;
    let ph = 0;
    function frame() {
      ctx.clearRect(0, 0, W, H);
      // dim grid dots
      ctx.fillStyle = "#1a2024";
      for (let x = 2; x < W; x += 6) for (let y = 2; y < H; y += 6) ctx.fillRect(x, y, 1, 1);
      ctx.beginPath();
      const col = active ? accent : "#2a3338";
      ctx.strokeStyle = col; ctx.lineWidth = 1.5;
      if (active && analyser && buf) {
        analyser.getByteTimeDomainData(buf);
        for (let i = 0; i < W; i++) {
          const v = buf[Math.floor((i / W) * buf.length)] / 128 - 1;
          const y = H / 2 + v * (H / 2 - 3);
          i === 0 ? ctx.moveTo(i, y) : ctx.lineTo(i, y);
        }
      } else if (active) {
        ph += 0.25;
        for (let i = 0; i < W; i++) { const y = H / 2 + Math.sin(i * 0.4 + ph) * (H / 4); i === 0 ? ctx.moveTo(i, y) : ctx.lineTo(i, y); }
      } else {
        // dormant: a barely-alive flat line with the tiniest jitter
        ph += 0.02;
        for (let i = 0; i < W; i++) { const y = H / 2 + Math.sin(i * 0.6 + ph) * 0.6; i === 0 ? ctx.moveTo(i, y) : ctx.lineTo(i, y); }
      }
      ctx.stroke();
      raf = requestAnimationFrame(frame);
    }
    frame();
    return () => cancelAnimationFrame(raf);
  }, [analyser, active, accent]);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
      <canvas ref={ref} width={104} height={42} style={{ display: "block", borderRadius: 2, background: "#0a0e10", border: "1px solid #16191b" }} />
      <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 9, letterSpacing: ".14em", color: active ? accent : "#383f44", textAlign: "center" }}>{label}</span>
    </div>
  );
}

// ---------- IO log (ambient) ---------------------------------------------
function IOLog() {
  const [lines, setLines] = React.useState([]);
  React.useEffect(() => {
    const verbs = ["DISK", "DISK", "NET ", "DISK", "MEM ", "NET "];
    const acts = ["rd blk", "wr blk", "seek", "rx pkt", "tx pkt", "cache", "flush", "sync", "alloc"];
    const id = setInterval(() => {
      const a = "0x" + Math.floor(Math.random() * 65536).toString(16).padStart(4, "0");
      const ln = `${verbs[Math.floor(Math.random() * verbs.length)]} ${acts[Math.floor(Math.random() * acts.length)]} ${a}`;
      setLines((p) => [...p.slice(-3), ln]);
    }, 900);
    return () => clearInterval(id);
  }, []);
  return (
    <div style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 9.5, lineHeight: "12px", color: "#2f363b", width: 116, overflow: "hidden", height: 42 }}>
      {lines.map((l, i) => <div key={i} style={{ opacity: 0.35 + i * 0.18 }}>{l}</div>)}
    </div>
  );
}

// ---------- status bar ---------------------------------------------------
function StatusBar({ pointer, analyser, audioActive, recentKeys }) {
  const cellW = 1;
  return (
    <div style={{
      display: "flex", alignItems: "center", gap: 18, height: 84, padding: "0 16px",
      background: "#08090a", border: "1px solid #16191b", borderRadius: 4,
    }}>
      {/* pointer coords — the one genuinely useful readout */}
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 9, letterSpacing: ".16em", color: "#4b5258" }}>AANWIJZER</span>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 17, color: "#9fb0bb", fontWeight: 600 }}>
          x:{String(pointer.x).padStart(3, "0")} <span style={{ color: "#4b5258" }}>y:</span>{String(pointer.y).padStart(3, "0")}
        </span>
        <span style={{ display: "flex", alignItems: "center", gap: 6, fontFamily: "'IBM Plex Mono', monospace", fontSize: 10, color: pointer.down ? "#34e24b" : "#3a4146" }}>
          <span style={{ width: 7, height: 7, borderRadius: 1, background: pointer.down ? "#34e24b" : "#1f2529" }} />
          {pointer.down ? "raak" : pointer.inside ? "zweef" : "—"}
        </span>
      </div>
      <div style={{ width: 1, alignSelf: "stretch", margin: "16px 0", background: "#16191b" }} />
      <Oscilloscope label="GELUID UIT" analyser={analyser} active={audioActive} accent="#34e24b" />
      <Oscilloscope label="GELUID IN" analyser={null} active={false} accent="#3a86ff" />
      <div style={{ width: 1, alignSelf: "stretch", margin: "16px 0", background: "#16191b" }} />
      <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 9, letterSpacing: ".16em", color: "#4b5258" }}>TOETSEN</span>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 14, color: "#6b757c", whiteSpace: "nowrap", letterSpacing: ".05em" }}>
          {recentKeys || "\u00b7\u00b7\u00b7\u00b7\u00b7\u00b7"}
        </span>
      </div>
      <div style={{ flex: 1 }} />
      <IOLog />
    </div>
  );
}

window.SchildpadChrome = { Transport, MessageBar, StatusBar };
