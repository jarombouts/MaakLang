/* schildpad-turtle.js — the payoff surface.
 * A logical pixel framebuffer (320 x H), chunky integer-scaled, no smoothing.
 * Persistent draw layer (pen lines + printed glyphs) + a sprite layer (turtles).
 * Pen lines wrap on a torus by default. Glyphs are the authored 8x8 bitmap face.
 *
 * window.SchildpadFB.create({drawCanvas, spriteCanvas, cols, rows}) -> framebuffer object
 * window.SchildpadFB.NAMED  -> the named in-canvas colour palette (Dutch)
 * window.SchildpadFB.TINTS  -> per-turtle tints (creation order)
 */
(function () {
  "use strict";

  // Punchy named colours that read as single chunky pixels on black.
  const NAMED = {
    rood:   "#ff3b30",
    groen:  "#34e24b",
    blauw:  "#3a86ff",
    geel:   "#ffd23a",
    wit:    "#ffffff",
    zwart:  "#000000",
    oranje: "#ff8a2a",
    paars:  "#b15cff",
    cyaan:  "#2ee6e6",
    roze:   "#ff6ec7",
  };
  // Turtle body tints by creation order — distinct at a glance.
  const TINTS = ["#34e24b", "#ff8a2a", "#3a86ff", "#ff6ec7", "#ffd23a", "#2ee6e6", "#b15cff", "#ff3b30"];

  function create(opts) {
    const { drawCanvas, spriteCanvas, cols, rows } = opts;
    const W = cols * 8;          // logical width (320 for 40 cols)
    const H = rows * 8;          // logical height
    drawCanvas.width = W; drawCanvas.height = H;
    spriteCanvas.width = W; spriteCanvas.height = H;
    const dctx = drawCanvas.getContext("2d");
    const sctx = spriteCanvas.getContext("2d");
    dctx.imageSmoothingEnabled = false; sctx.imageSmoothingEnabled = false;

    const fb = {
      W, H, cols, rows,
      cursorCol: 0, cursorRow: 0,
      fontVariant: "regular",
      wrapMode: "wrap",
      _font: window.SchildpadFont.get("regular"),
    };

    function wrapX(x) { return ((x % W) + W) % W; }
    function wrapY(y) { return ((y % H) + H) % H; }

    fb.setFont = function (variant) { fb.fontVariant = variant; fb._font = window.SchildpadFont.get(variant); };

    fb.clear = function () {
      dctx.fillStyle = "#000000"; dctx.fillRect(0, 0, W, H);
      sctx.clearRect(0, 0, W, H);
      fb.cursorCol = 0; fb.cursorRow = 0;
    };

    fb.plot = function (x, y, color) {
      const px = Math.round(x), py = Math.round(y);
      const wx = fb.wrapMode === "wrap" ? wrapX(px) : px;
      const wy = fb.wrapMode === "wrap" ? wrapY(py) : py;
      if (fb.wrapMode !== "wrap" && (wx < 0 || wx >= W || wy < 0 || wy >= H)) return;
      dctx.fillStyle = color;
      dctx.fillRect(wx, wy, 1, 1);
    };

    // move a turtle forward `dist` logical px along its heading, drawing if pen down
    fb.move = function (turtle, dist) {
      const rad = (turtle.heading * Math.PI) / 180;
      const dx = Math.cos(rad) * dist, dy = Math.sin(rad) * dist;
      const x0 = turtle.x, y0 = turtle.y;
      const steps = Math.max(1, Math.ceil(Math.abs(dist)));
      for (let i = 1; i <= steps; i++) {
        const t = i / steps;
        const px = x0 + dx * t, py = y0 + dy * t;
        if (turtle.penDown) fb.plot(px, py, turtle.pen || "#ffffff");
      }
      turtle.x = fb.wrapMode === "wrap" ? wrapX(x0 + dx) : x0 + dx;
      turtle.y = fb.wrapMode === "wrap" ? wrapY(y0 + dy) : y0 + dy;
    };

    // print text at the glyph cursor, advancing & wrapping the 40-col grid
    fb.print = function (str) {
      const font = fb._font;
      for (const ch of String(str)) {
        if (ch === "\n") { fb.cursorCol = 0; fb.cursorRow++; continue; }
        const gx = fb.cursorCol * 8, gy = fb.cursorRow * 8;
        window.SchildpadFont.drawChar(font, ch, gx, gy, (x, y) => {
          const wx = fb.wrapMode === "wrap" ? wrapX(x) : x;
          const wy = fb.wrapMode === "wrap" ? wrapY(y) : y;
          dctx.fillStyle = "#ffffff";
          dctx.fillRect(wx, wy, 1, 1);
        });
        fb.cursorCol++;
        if (fb.cursorCol >= fb.cols) { fb.cursorCol = 0; fb.cursorRow++; }
        if (fb.cursorRow >= fb.rows) fb.cursorRow = 0;
      }
      fb.cursorCol = 0; fb.cursorRow++;       // print is line-terminated
      if (fb.cursorRow >= fb.rows) fb.cursorRow = 0;
    };

    fb.colorOf = function (name) { return NAMED[name] || "#ffffff"; };
    fb.defaultPen = function (idx) { return TINTS[idx % TINTS.length]; };
    fb.tintOf = function (idx) { return TINTS[idx % TINTS.length]; };

    // ---- sprite rendering ------------------------------------------------
    fb.renderSprites = function (turtles, spriteStyle, t) {
      sctx.clearRect(0, 0, W, H);
      for (const turtle of turtles) drawTurtle(sctx, turtle, fb.tintOf(turtle.tint), spriteStyle, t);
    };

    return fb;
  }

  function px(ctx, x, y, c) { ctx.fillStyle = c; ctx.fillRect(Math.round(x), Math.round(y), 1, 1); }
  function dirUnit(deg) { const r = (deg * Math.PI) / 180; return [Math.cos(r), Math.sin(r)]; }

  // three explorable sprite styles; heading shown by a nose/notch/arrow
  function drawTurtle(ctx, turtle, tint, style, t) {
    const cx = Math.round(turtle.x), cy = Math.round(turtle.y);
    const [ux, uy] = dirUnit(turtle.heading);
    const shade = darken(tint, 0.45);
    const dark = darken(tint, 0.65);

    // arrival: a single expanding chunky ring the moment a turtle is summoned
    if (turtle._bornAt && t != null) {
      const age = t - turtle._bornAt;
      if (age >= 0 && age < 420) {
        const r = 3 + (age / 420) * 8;
        ctx.fillStyle = tint;
        for (let a = 0; a < 360; a += 30) {
          const rr = (a * Math.PI) / 180;
          px(ctx, cx + Math.cos(rr) * r, cy + Math.sin(rr) * r, tint);
        }
      }
    }

    if (style === "pijl") {
      // bold chevron arrowhead pointing along heading
      const [nx, ny] = [Math.round(ux), Math.round(uy)]; // perpendicular by swap
      const [pxx, pyy] = [-Math.round(uy), Math.round(ux)];
      for (let i = -2; i <= 2; i++) {
        px(ctx, cx + ux * 3 + pxx * i, cy + uy * 3 + pyy * i, tint);
      }
      for (let i = -1; i <= 1; i++) px(ctx, cx + ux * 2 + pxx * i, cy + uy * 2 + pyy * i, tint);
      px(ctx, cx + ux * 4, cy + uy * 4, "#ffffff");
      px(ctx, cx - ux, cy - uy, shade);
      px(ctx, cx - ux * 2, cy - uy * 2, shade);
      return;
    }

    // shared rounded body (used by 'schildpad' and 'rond')
    const body = [
      [-1, -2], [0, -2], [1, -2],
      [-2, -1], [-1, -1], [0, -1], [1, -1], [2, -1],
      [-2, 0], [-1, 0], [0, 0], [1, 0], [2, 0],
      [-2, 1], [-1, 1], [0, 1], [1, 1], [2, 1],
      [-1, 2], [0, 2], [1, 2],
    ];
    for (const [dx, dy] of body) px(ctx, cx + dx, cy + dy, tint);

    if (style === "rond") {
      // smooth dome + bright directional notch
      px(ctx, cx, cy, darken(tint, 0.3));
      const nxp = cx + Math.round(ux * 2), nyp = cy + Math.round(uy * 2);
      px(ctx, nxp, nyp, "#ffffff");
      px(ctx, cx + Math.round(ux * 3), cy + Math.round(uy * 3), tint);
      return;
    }

    // default: 'schildpad' — shell pattern + four feet + a head/nose poking forward
    px(ctx, cx, cy, shade);
    px(ctx, cx - 1, cy - 1, shade); px(ctx, cx + 1, cy + 1, shade);
    px(ctx, cx + 1, cy - 1, dark); px(ctx, cx - 1, cy + 1, dark);
    // feet at the four diagonals
    const feet = [[-2, -2], [2, -2], [-2, 2], [2, 2]];
    for (const [dx, dy] of feet) px(ctx, cx + dx, cy + dy, tint);
    // head/nose: two pixels forward + a white tip = the heading indicator
    const hx = cx + Math.round(ux * 3), hy = cy + Math.round(uy * 3);
    const hx2 = cx + Math.round(ux * 2), hy2 = cy + Math.round(uy * 2);
    px(ctx, hx2, hy2, tint);
    px(ctx, hx, hy, "#ffffff");
    // little tail opposite heading
    px(ctx, cx - Math.round(ux * 3), cy - Math.round(uy * 3), shade);
  }

  function darken(hex, f) {
    const h = hex.replace("#", "");
    const r = parseInt(h.slice(0, 2), 16), g = parseInt(h.slice(2, 4), 16), b = parseInt(h.slice(4, 6), 16);
    const d = (v) => Math.max(0, Math.round(v * (1 - f)));
    return `rgb(${d(r)},${d(g)},${d(b)})`;
  }

  window.SchildpadFB = { create, NAMED, TINTS };
})();
