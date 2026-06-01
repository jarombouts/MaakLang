/* schildpad-keyboard.jsx — custom input accessory + on-screen keyboard.
 * A scaffold, not a replacement for typing: big tappable verbs/keywords + the
 * fine-motor-nightmare symbols, over a child-legible letter grid. Three explorable
 * layouts via the `layout` prop: "woorden" (verbs-first), "gegroepeerd" (labelled
 * sections), "compact" (one verb row + keys).
 */

const KB_VERBS = [
  { t: "maak", c: "#5cd6f0" }, { t: "vooruit", c: "#8ec9ff" }, { t: "draai", c: "#8ec9ff" },
  { t: "links", c: "#9ad6a8" }, { t: "rechts", c: "#9ad6a8" }, { t: "pen", c: "#8ec9ff" },
  { t: "print", c: "#ffb86b" }, { t: "herhaal", c: "#c9a6ff" }, { t: "random", c: "#ff9e64" },
];
const KB_TYPES = [{ t: "schildpad", c: "#b79bff" }, { t: "getal", c: "#b79bff" }, { t: "draairichting", c: "#b79bff" }];
const KB_COLORS = ["rood", "groen", "blauw", "geel", "wit", "oranje", "paars", "cyaan", "roze"];
const KB_SYMBOLS = ["=", '"', "[", "]", "(", ")", "+", "-", "*", "/"];
const KB_DIGITS = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
const KB_QWERTY = [["q", "w", "e", "r", "t", "y", "u", "i", "o", "p"], ["a", "s", "d", "f", "g", "h", "j", "k", "l"], ["z", "x", "c", "v", "b", "n", "m"]];

function Key({ label, color, w = 1, h = 1, big, onPress, mono = true }) {
  const [down, setDown] = React.useState(false);
  return (
    <button
      onMouseDown={(e) => { e.preventDefault(); setDown(true); }}
      onMouseUp={() => setDown(false)} onMouseLeave={() => setDown(false)}
      onClick={onPress}
      style={{
        flex: w, height: 44 * h, minWidth: 0,
        fontFamily: "'IBM Plex Mono', monospace", fontSize: big ? 16 : 14, fontWeight: big ? 600 : 500,
        color: color || "#d2d9df",
        background: down ? "#23272b" : "#15171a",
        border: "1px solid " + (down ? "#3a4046" : "#26292d"),
        borderBottom: down ? "1px solid #3a4046" : "2px solid #0c0d0f",
        borderRadius: 5, cursor: "pointer", transform: down ? "translateY(1px)" : "none",
        transition: "transform .04s, background .04s", padding: "0 4px", whiteSpace: "nowrap", overflow: "hidden",
      }}>{label}</button>
  );
}

function Row({ children, gap = 6 }) {
  return <div style={{ display: "flex", gap, width: "100%" }}>{children}</div>;
}

function SectionLabel({ children }) {
  return <div style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 9, letterSpacing: ".18em", textTransform: "uppercase", color: "#4b5258", margin: "2px 2px 0" }}>{children}</div>;
}

function OnScreenKeyboard({ layout, onKey, onBack, onEnter, onSpace }) {
  const NAMED_KB = window.SchildpadFB.NAMED;
  const ins = (t) => onKey(t);
  const word = (t) => onKey(t + " ");

  const VerbRow = ({ big }) => (
    <Row>{KB_VERBS.map((v) => <Key key={v.t} label={v.t} color={v.c} big={big} onPress={() => word(v.t)} />)}</Row>
  );
  const TypeColorRow = () => (
    <Row>
      {KB_TYPES.map((v) => <Key key={v.t} label={v.t} color={v.c} onPress={() => word(v.t)} />)}
      {KB_COLORS.map((c) => <Key key={c} label={c} color={NAMED_KB[c]} onPress={() => word(c)} />)}
    </Row>
  );
  const SymbolRow = () => (
    <Row>
      {KB_DIGITS.map((d) => <Key key={d} label={d} color="#ffd23a" onPress={() => ins(d)} />)}
      {KB_SYMBOLS.map((s) => <Key key={s} label={s} color="#86919a" onPress={() => ins(s)} />)}
    </Row>
  );
  const Letters = () => (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      {KB_QWERTY.map((r, i) => (
        <Row key={i}>
          {i === 2 && <Key label="⌫" w={1.4} color="#86919a" onPress={onBack} />}
          {r.map((k) => <Key key={k} label={k} onPress={() => ins(k)} />)}
          {i === 2 && <Key label="enter" w={1.8} color="#5cd6f0" onPress={onEnter} />}
        </Row>
      ))}
      <Row>
        <Key label="'" w={1} color="#86919a" onPress={() => ins("'")} />
        <Key label="spatie" w={6} color="#86919a" onPress={onSpace} />
        <Key label="." w={1} color="#86919a" onPress={() => ins(".")} />
      </Row>
    </div>
  );

  let body;
  if (layout === "gegroepeerd") {
    body = (
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <SectionLabel>werkwoorden</SectionLabel>
        <VerbRow big />
        <SectionLabel>soorten &amp; kleuren</SectionLabel>
        <TypeColorRow />
        <SectionLabel>tekens &amp; cijfers</SectionLabel>
        <SymbolRow />
        <SectionLabel>letters</SectionLabel>
        <Letters />
      </div>
    );
  } else if (layout === "compact") {
    body = (
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <Row><div style={{ display: "flex", gap: 6, overflowX: "auto", width: "100%" }}>
          {[...KB_VERBS, ...KB_TYPES].map((v) => <Key key={v.t} label={v.t} color={v.c} onPress={() => word(v.t)} />)}
        </div></Row>
        <SymbolRow />
        <Letters />
      </div>
    );
  } else {
    // woorden (verbs-first, the default)
    body = (
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <VerbRow big />
        <TypeColorRow />
        <SymbolRow />
        <Letters />
      </div>
    );
  }

  return (
    <div style={{
      background: "#0b0c0d", borderTop: "2px solid #1f2226", padding: "10px 12px 12px",
      boxShadow: "0 -14px 30px rgba(0,0,0,.5)",
    }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8, padding: "0 2px" }}>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 10, letterSpacing: ".18em", textTransform: "uppercase", color: "#4b5258" }}>schermtoetsenbord</span>
        <span style={{ fontFamily: "'IBM Plex Mono', monospace", fontSize: 10, color: "#3a4146" }}>{layout}</span>
      </div>
      {body}
    </div>
  );
}

window.SchildpadKeyboard = { OnScreenKeyboard };
