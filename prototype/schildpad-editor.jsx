/* schildpad-editor.jsx — the code pane editor.
 * A real <textarea> (transparent ink, visible caret) layered over a syntax-highlight
 * <pre>, a line-number gutter, and a full-width current-line band for joint attention.
 * Gentle Tier-1 feedback = a soft dotted underline on suspect tokens. No red walls.
 */

const EDITOR_LINE_H = 30;
const EDITOR_FONT_PX = 19;
const EDITOR_PAD_Y = 14;
const EDITOR_PAD_X = 16;

const TOK_STYLE = {
  maak:   { color: "#5cd6f0", fontWeight: 700 },
  verb:   { color: "#8ec9ff" },
  print:  { color: "#ffb86b" },
  play:   { color: "#ffb86b" },
  loop:   { color: "#c9a6ff", fontWeight: 600 },
  type:   { color: "#b79bff" },
  dir:    { color: "#9ad6a8" },
  random: { color: "#ff9e64", fontStyle: "italic" },
  number: { color: "#ffd23a" },
  string: { color: "#7ee787" },
  name:   { color: "#dbe2e8" },
  op:     { color: "#76828d" },
  paren:  { color: "#76828d" },
  unknown:{ color: "#e0746c" },
  maakkw: { color: "#5cd6f0", fontWeight: 700 },
};

function escapeHtml(s) {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" }[c]));
}

function HighlightedLine({ raw, toks }) {
  const NAMED_INK = window.SchildpadFB.NAMED;
  // rebuild the line preserving exact columns (whitespace between tokens)
  const parts = [];
  let prev = 0;
  toks.forEach((tk, i) => {
    if (tk.col > prev) parts.push(<span key={"g" + i}>{raw.slice(prev, tk.col)}</span>);
    const base = tk.kind === "colour" ? { color: NAMED_INK[tk.value.toLowerCase()] || "#fff" } : (TOK_STYLE[tk.kind] || TOK_STYLE.name);
    const style = { ...base };
    if (tk.suspect) {
      style.textDecoration = "underline dotted";
      style.textDecorationColor = "rgba(224,116,108,.55)";
      style.textUnderlineOffset = "4px";
      style.opacity = 0.72;
    }
    parts.push(<span key={"t" + i} style={style}>{tk.text}</span>);
    prev = tk.col + tk.len;
  });
  if (prev < raw.length) parts.push(<span key="tail">{raw.slice(prev)}</span>);
  if (parts.length === 0) parts.push(<span key="empty">{"\u200b"}</span>);
  return <div style={{ height: EDITOR_LINE_H, lineHeight: EDITOR_LINE_H + "px" }}>{parts}</div>;
}

function Editor({ code, analysis, lines, currentLine, errorLine, onChange, onEnterLine, ghost, taRef: extRef }) {
  const innerRef = React.useRef(null);
  const taRef = extRef || innerRef;
  const hlRef = React.useRef(null);
  const gutRef = React.useRef(null);

  const syncScroll = () => {
    const ta = taRef.current;
    if (hlRef.current) { hlRef.current.scrollTop = ta.scrollTop; hlRef.current.scrollLeft = ta.scrollLeft; }
    if (gutRef.current) gutRef.current.scrollTop = ta.scrollTop;
  };

  const handleKeyDown = (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      const ta = taRef.current;
      const val = ta.value;
      const pos = ta.selectionStart;
      const lineStart = val.lastIndexOf("\n", pos - 1) + 1;
      let lineEnd = val.indexOf("\n", pos);
      if (lineEnd === -1) lineEnd = val.length;
      const lineText = val.slice(lineStart, lineEnd);
      const lineNo = val.slice(0, lineStart).split("\n").length;
      if (lineText.trim()) onEnterLine && onEnterLine(lineText, lineNo);
    }
  };

  const lineCount = Math.max(lines.length, 1);
  const showGhost = ghost && code.length === 0;

  return (
    <div style={{ position: "relative", flex: 1, display: "flex", overflow: "hidden", background: "#0c0c0d" }}>
      {/* gutter */}
      <div ref={gutRef} style={{
        width: 46, flexShrink: 0, overflow: "hidden", textAlign: "right",
        padding: `${EDITOR_PAD_Y}px 8px ${EDITOR_PAD_Y}px 0`,
        fontFamily: "'IBM Plex Mono', monospace", fontSize: EDITOR_FONT_PX, lineHeight: EDITOR_LINE_H + "px",
        color: "#3c4147", userSelect: "none", borderRight: "1px solid #1b1c1e", background: "#0a0a0b",
      }}>
        {Array.from({ length: lineCount }, (_, i) => (
          <div key={i} style={{
            height: EDITOR_LINE_H,
            color: i + 1 === currentLine ? "#5cd6f0" : (i + 1 === errorLine ? "#e0746c" : "#3c4147"),
            fontWeight: i + 1 === currentLine ? 700 : 400,
          }}>{i + 1}</div>
        ))}
      </div>

      {/* code area */}
      <div style={{ position: "relative", flex: 1, overflow: "hidden" }}>
        {/* current-line band */}
        {currentLine != null && (
          <div style={{
            position: "absolute", left: 0, right: 0,
            top: EDITOR_PAD_Y + (currentLine - 1) * EDITOR_LINE_H,
            height: EDITOR_LINE_H,
            background: errorLine === currentLine ? "rgba(224,116,108,.10)" : "rgba(92,214,240,.11)",
            borderLeft: `3px solid ${errorLine === currentLine ? "#e0746c" : "#5cd6f0"}`,
            transform: `translateY(${-taRefScroll(taRef)}px)`,
            pointerEvents: "none", transition: "top .08s linear",
          }} />
        )}

        {/* highlight layer */}
        <pre ref={hlRef} aria-hidden="true" style={{
          position: "absolute", inset: 0, margin: 0, overflow: "hidden",
          padding: `${EDITOR_PAD_Y}px ${EDITOR_PAD_X}px`,
          fontFamily: "'IBM Plex Mono', monospace", fontSize: EDITOR_FONT_PX, lineHeight: EDITOR_LINE_H + "px",
          whiteSpace: "pre", color: "#dbe2e8", pointerEvents: "none", tabSize: 2,
        }}>
          {analysis.map((toks, i) => <HighlightedLine key={i} raw={lines[i] || ""} toks={toks} />)}
        </pre>

        {/* ghost invitation */}
        {showGhost && (
          <div style={{
            position: "absolute", top: EDITOR_PAD_Y, left: EDITOR_PAD_X,
            fontFamily: "'IBM Plex Mono', monospace", fontSize: EDITOR_FONT_PX, lineHeight: EDITOR_LINE_H + "px",
            color: "#2f3338", pointerEvents: "none",
          }}>maak pietje schildpad</div>
        )}

        {/* the real textarea */}
        <textarea
          ref={taRef}
          value={code}
          spellCheck={false}
          autoCapitalize="off" autoCorrect="off"
          onChange={(e) => { onChange(e.target.value); }}
          onScroll={syncScroll}
          onKeyDown={handleKeyDown}
          style={{
            position: "absolute", inset: 0, width: "100%", height: "100%",
            padding: `${EDITOR_PAD_Y}px ${EDITOR_PAD_X}px`, margin: 0, border: "none", outline: "none",
            background: "transparent", resize: "none",
            fontFamily: "'IBM Plex Mono', monospace", fontSize: EDITOR_FONT_PX, lineHeight: EDITOR_LINE_H + "px",
            color: "transparent", caretColor: "#5cd6f0", whiteSpace: "pre", overflow: "auto", tabSize: 2,
          }}
        />
      </div>
    </div>
  );
}

// keep the current-line band aligned with textarea scroll (read live each render)
function taRefScroll(ref) { return ref.current ? ref.current.scrollTop : 0; }

window.SchildpadEditor = { Editor, EDITOR_LINE_H };
