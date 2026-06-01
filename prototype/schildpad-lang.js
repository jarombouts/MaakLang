/* schildpad-lang.js  —  the real little interpreter.
 *
 * A strict, deterministic, left-to-right pipeline language with typed-slot resolution
 * (free word order), a tiny reserved type system, verb-bound `random`, dynamic name
 * resolution (a-'i), and verbose patient Dutch run-time errors. See LANGUAGE.md.
 *
 * Exposes window.Schildpad with: vocab sets, tokenize(line), analyze(text), compile(text),
 * and Engine (the stepping transport). The UI imports these; nothing here touches the DOM
 * except drawing callbacks handed in via ctx.
 */
(function () {
  "use strict";

  // ---- vocabulary (all reserved, all Dutch) -------------------------------
  const TYPES = new Set(["schildpad", "getal", "draairichting"]);
  const DIRECTIONS = { links: -90, rechts: 90 };
  const COLORS = ["rood", "groen", "blauw", "geel", "wit", "zwart", "oranje", "paars", "cyaan", "roze"];
  const COLORSET = new Set(COLORS);
  const VERBS = {
    vooruit:    { slots: ["getal", "schildpad"],        sampler: { type: "getal", kind: "uniform", lo: 0, hi: 100 } },
    achteruit:  { slots: ["getal", "schildpad"],        sampler: { type: "getal", kind: "uniform", lo: 0, hi: 100 } },
    draai:      { slots: ["draairichting", "schildpad"], sampler: { type: "draairichting", kind: "lr" } },
    pen:        { slots: ["kleur", "schildpad"],         sampler: null },
    penomhoog:  { slots: ["schildpad"],                  sampler: null },
    penomlaag:  { slots: ["schildpad"],                  sampler: null },
  };
  const LOOPWORDS = new Set(["herhaal", "doe", "keer"]);
  const KEYWORDS = new Set([
    "maak", "print", "random", "play",
    ...Object.keys(VERBS), ...Object.keys(DIRECTIONS), ...COLORS, ...TYPES, ...LOOPWORDS,
  ]);

  // ---- lexer --------------------------------------------------------------
  // A name may carry dynamic interpolation: a-'i  or  a-'(i+1)
  const NAME_START = /[A-Za-z]/;
  const NAME_CHAR = /[A-Za-z0-9_]/;

  function tokenize(line) {
    const toks = [];
    let i = 0;
    const n = line.length;
    while (i < n) {
      const ch = line[i];
      if (ch === " " || ch === "\t") { i++; continue; }
      const start = i;

      // string literal
      if (ch === '"') {
        i++;
        let val = "";
        let closed = false;
        while (i < n) {
          if (line[i] === '"') { closed = true; i++; break; }
          val += line[i]; i++;
        }
        toks.push({ kind: "string", value: val, text: line.slice(start, i), col: start, len: i - start, closed });
        continue;
      }
      // number
      if (/[0-9]/.test(ch)) {
        let j = i + 1;
        while (j < n && /[0-9.]/.test(line[j])) j++;
        const text = line.slice(i, j);
        toks.push({ kind: "number", value: parseFloat(text), text, col: i, len: j - i });
        i = j; continue;
      }
      // operators & brackets
      if ("+-*/=".includes(ch)) {
        toks.push({ kind: "op", value: ch, text: ch, col: i, len: 1 });
        i++; continue;
      }
      if ("()".includes(ch)) { toks.push({ kind: "paren", value: ch, text: ch, col: i, len: 1 }); i++; continue; }
      if ("[]".includes(ch)) { toks.push({ kind: "bracket", value: ch, text: ch, col: i, len: 1 }); i++; continue; }

      // identifier / name (may include dynamic - ' interpolation)
      if (NAME_START.test(ch)) {
        let j = i + 1;
        while (j < n && NAME_CHAR.test(line[j])) j++;
        // dynamic continuation: ('...) or (-'...)
        while (j < n) {
          if (line[j] === "'") {
            j++; // consume '
            if (line[j] === "(") {
              let depth = 1; j++;
              while (j < n && depth > 0) { if (line[j] === "(") depth++; else if (line[j] === ")") depth--; j++; }
            } else {
              while (j < n && NAME_CHAR.test(line[j])) j++;
            }
          } else if (line[j] === "-" && line[j + 1] === "'") {
            j++; // include the hyphen, loop handles the '
          } else break;
        }
        const text = line.slice(i, j);
        const lower = text.toLowerCase();
        let kind = "name";
        if (lower === "maak") kind = "maak";
        else if (lower === "print") kind = "print";
        else if (lower === "random") kind = "random";
        else if (lower === "play") kind = "play";
        else if (LOOPWORDS.has(lower)) kind = "loop";
        else if (VERBS[lower]) kind = "verb";
        else if (DIRECTIONS.hasOwnProperty(lower)) kind = "dir";
        else if (COLORSET.has(lower)) kind = "colour";
        else if (TYPES.has(lower)) kind = "type";
        toks.push({ kind, value: text, text, col: i, len: j - i, dynamic: text.includes("'") });
        i = j; continue;
      }

      // stray character
      toks.push({ kind: "unknown", value: ch, text: ch, col: i, len: 1 });
      i++;
    }
    return toks;
  }

  // ---- gentle (Tier 1) analysis for the editor ----------------------------
  function editDistance1(a, b) {
    if (a === b) return false;
    const la = a.length, lb = b.length;
    if (Math.abs(la - lb) > 1) return false;
    let i = 0, j = 0, edits = 0;
    while (i < la && j < lb) {
      if (a[i] === b[j]) { i++; j++; continue; }
      edits++; if (edits > 1) return false;
      if (la > lb) i++; else if (lb > la) j++; else { i++; j++; }
    }
    edits += (la - i) + (lb - j);
    return edits === 1;
  }
  const KEYWORD_LIST = [...KEYWORDS];

  // Returns per-line token arrays with a `suspect` flag for gentle feedback.
  function analyze(text) {
    const userNames = new Set();
    const lines = text.split("\n");
    // collect declared names (maak <naam> ...) so they aren't flagged as typos
    for (const ln of lines) {
      const t = tokenize(ln);
      if (t[0] && t[0].kind === "maak" && t[1] && t[1].kind === "name") {
        userNames.add(t[1].value.toLowerCase());
      }
    }
    return lines.map((ln) => {
      const toks = tokenize(ln);
      for (const tk of toks) {
        tk.suspect = false;
        if (tk.kind === "string" && tk.closed === false) tk.suspect = true;
        if (tk.kind === "unknown") tk.suspect = true;
        if (tk.kind === "name" && !tk.dynamic) {
          const lw = tk.value.toLowerCase();
          if (!userNames.has(lw)) {
            for (const kw of KEYWORD_LIST) {
              if (editDistance1(lw, kw)) { tk.suspect = true; break; }
            }
          }
        }
      }
      return toks;
    });
  }

  // ---- values & environment ----------------------------------------------
  function val(type, value) { return { type, value }; }
  function fmtNum(x) {
    if (typeof x !== "number") return String(x);
    if (Number.isInteger(x)) return String(x);
    return String(Math.round(x * 100) / 100);
  }

  // ---- errors -------------------------------------------------------------
  function err(line, message) { const e = new Error(message); e.schildpad = true; e.line = line; return e; }

  // ---- dynamic name resolution -------------------------------------------
  // "a-'i" -> literal "a-" + str(value of i);  "a-'(i+1)" -> "a-" + str(eval(i+1))
  function resolveName(raw, env, line) {
    if (!raw.includes("'")) return { name: raw, built: false };
    const parts = raw.split("'");
    let name = parts[0];
    const pieces = [parts[0]];
    for (let k = 1; k < parts.length; k++) {
      let seg = parts[k], expr, rest = "";
      if (seg.startsWith("(")) {
        const close = seg.indexOf(")");
        expr = seg.slice(1, close);
        rest = seg.slice(close + 1);
      } else {
        const m = seg.match(/^[A-Za-z0-9_]+/);
        expr = m ? m[0] : "";
        rest = seg.slice(expr.length);
      }
      const v = evalExpr(tokenize(expr), env, line);
      name += fmtNum(v.value) + rest;
      pieces.push(expr, rest);
    }
    return { name, built: true, raw };
  }

  // ---- expression evaluation (infix, normal precedence) -------------------
  function evalExpr(toks, env, line) {
    let pos = 0;
    function peek() { return toks[pos]; }
    function next() { return toks[pos++]; }
    function parseExpr() { return parseAdd(); }
    function parseAdd() {
      let left = parseMul();
      while (peek() && peek().kind === "op" && (peek().value === "+" || peek().value === "-")) {
        const op = next().value;
        const right = parseMul();
        left = applyOp(op, left, right);
      }
      return left;
    }
    function parseMul() {
      let left = parsePrimary();
      while (peek() && peek().kind === "op" && (peek().value === "*" || peek().value === "/")) {
        const op = next().value;
        const right = parsePrimary();
        left = applyOp(op, left, right);
      }
      return left;
    }
    function parsePrimary() {
      const tk = peek();
      if (!tk) throw err(line, "ik verwachtte hier nog een getal of een naam, maar de regel hield op.");
      if (tk.kind === "paren" && tk.value === "(") {
        next();
        const v = parseExpr();
        if (peek() && peek().kind === "paren" && peek().value === ")") next();
        return v;
      }
      if (tk.kind === "op" && tk.value === "-") { next(); const v = parsePrimary(); return val("getal", -toNum(v, line)); }
      if (tk.kind === "number") { next(); return val("getal", tk.value); }
      if (tk.kind === "string") { next(); return val("string", tk.value); }
      if (tk.kind === "dir") { next(); return val("draairichting", DIRECTIONS[tk.value.toLowerCase()]); }
      if (tk.kind === "colour") { next(); return val("kleur", tk.value.toLowerCase()); }
      if (tk.kind === "random") { throw err(line, "random waarvan? ik kan alleen willekeurig kiezen als er een werkwoord voor staat dat weet waaruit het mag kiezen, zoals 'vooruit random' of 'draai random'."); }
      if (tk.kind === "name") {
        next();
        const r = resolveName(tk.value, env, line);
        const b = env.get(r.name.toLowerCase());
        if (!b) {
          if (r.built) throw err(line, `ik probeerde '${r.name}' te vinden, opgebouwd uit '${r.raw.split("'")[0]}' en de waarde van ${r.raw.split("'").slice(1).join("'")}, maar die bestaat niet.`);
          throw err(line, `ik ken geen '${r.name}'. misschien moet je die eerst maken met 'maak ${r.name}'?`);
        }
        if (b.type === "nil") throw err(line, `'${r.name}' bestaat wel, maar had op dat moment de waarde nil. ik weet niet hoe ik daar mee moet rekenen.`);
        return b;
      }
      throw err(line, `ik begreep '${tk.text}' hier niet.`);
    }
    function toNum(v, ln) {
      if (v.type === "getal" || v.type === "draairichting") return v.value;
      throw err(ln, `ik wilde hier een getal, maar kreeg een ${typeNL(v.type)}.`);
    }
    function applyOp(op, a, b) {
      if (op === "+" && (a.type === "string" || b.type === "string")) {
        return val("string", strOf(a) + strOf(b));
      }
      const x = toNum(a, line), y = toNum(b, line);
      if (op === "+") return val("getal", x + y);
      if (op === "-") return val("getal", x - y);
      if (op === "*") return val("getal", x * y);
      if (op === "/") { if (y === 0) throw err(line, "ik kan niet door nul delen."); return val("getal", x / y); }
      throw err(line, `onbekende rekenstap '${op}'.`);
    }
    const result = parseExpr();
    return result;
  }
  function strOf(v) {
    if (v.type === "string") return v.value;
    if (v.type === "getal") return fmtNum(v.value);
    if (v.type === "draairichting") return fmtNum(v.value);
    if (v.type === "schildpad") return v.value.name;
    return "nil";
  }
  function typeNL(t) {
    return ({ schildpad: "schildpad", getal: "getal", draairichting: "draairichting", kleur: "kleur", string: "tekst", nil: "nil" })[t] || t;
  }

  // ---- statement compilation ---------------------------------------------
  // Build a tree of statement nodes from source text, using indentation for blocks.
  function compile(text) {
    const rawLines = text.split("\n");
    const lines = rawLines.map((s, idx) => ({ src: s, indent: s.match(/^[ \t]*/)[0].length, lineNo: idx + 1, trimmed: s.trim() }));

    let p = 0;
    function parseBlock(minIndent) {
      const out = [];
      while (p < lines.length) {
        const ln = lines[p];
        if (ln.trimmed === "" || ln.trimmed === "[" || ln.trimmed === "]") { p++; continue; }
        if (ln.indent < minIndent) break;
        p++;
        const toks = tokenize(ln.src).filter((t) => t.kind !== "bracket");
        const first = toks[0];
        if (first && first.kind === "loop") {
          // loop header: herhaal <n> | doe <n> keer | doe
          const node = parseLoopHeader(toks, ln);
          // body = following lines with greater indent (or bracketed block follows)
          const childIndent = (lines[p] ? lines[p].indent : ln.indent + 1);
          node.body = parseBlock(Math.max(childIndent, ln.indent + 1));
          out.push(node);
        } else {
          out.push({ kind: "simple", line: ln.lineNo, toks, src: ln.trimmed });
        }
      }
      return out;
    }
    function parseLoopHeader(toks, ln) {
      // forms: herhaal <n> ; doe <n> keer ; doe
      const w = toks[0].value.toLowerCase();
      let countToks = null, unbounded = false;
      if (w === "herhaal") {
        countToks = toks.slice(1);
      } else if (w === "doe") {
        const keerIdx = toks.findIndex((t) => t.kind === "loop" && t.value.toLowerCase() === "keer");
        if (keerIdx >= 0) countToks = toks.slice(1, keerIdx);
        else if (toks.length === 1) unbounded = true;
        else countToks = toks.slice(1);
      }
      return { kind: "loop", line: ln.lineNo, unbounded, countToks, header: ln.trimmed };
    }
    const program = parseBlock(0);
    return { program };
  }

  // ---- the execution engine (transport) -----------------------------------
  // ctx must provide: turtleFactory(name, env), draw line, plot pixel, printGlyphs, clear,
  //                   audio(spec), markBuilt(name). The UI supplies these.
  class Engine {
    constructor(ctx) {
      this.ctx = ctx;
      this.env = new Map();
      this.turtleOrder = [];
      this.stack = [];
      this.done = true;
      this.error = null;
    }

    reset(program) {
      this.program = program;
      this.env = new Map();
      this.turtleOrder = [];
      this.stack = program ? [{ list: program, idx: 0, kind: "root" }] : [];
      this.done = !program || program.length === 0;
      this.error = null;
      this.ctx.clear();
    }

    // line that will execute next (for the highlight), or null
    currentLine() {
      for (let s = this.stack.length - 1; s >= 0; s--) {
        const fr = this.stack[s];
        if (fr.idx < fr.list.length) return fr.list[fr.idx].line;
      }
      return null;
    }

    // execute exactly one statement; returns { line, done } or throws schildpad error
    step() {
      // unwind finished frames (loop bookkeeping)
      while (this.stack.length) {
        const fr = this.stack[this.stack.length - 1];
        if (fr.idx < fr.list.length) break;
        if (fr.kind === "count" && fr.reps > 0) { fr.reps--; fr.idx = 0; }
        else if (fr.kind === "unbounded") { fr.idx = 0; if (fr.list.length === 0) { this.stack.pop(); } }
        else this.stack.pop();
      }
      if (!this.stack.length) { this.done = true; return { done: true, line: null }; }

      const fr = this.stack[this.stack.length - 1];
      const node = fr.list[fr.idx];
      fr.idx++;

      if (node.kind === "loop") {
        let count = 0;
        if (!node.unbounded) {
          const v = evalExpr(node.countToks, this.env, node.line);
          count = Math.max(0, Math.floor(v.value));
        }
        if (node.unbounded) this.stack.push({ list: node.body, idx: 0, kind: "unbounded" });
        else if (count > 0) this.stack.push({ list: node.body, idx: 0, kind: "count", reps: count - 1 });
        // count 0 -> skip body
        return { done: false, line: node.line };
      }
      // simple statement
      this.execSimple(node);
      return { done: false, line: node.line };
    }

    // run a single source line immediately against live env (the Enter / live mode)
    runLine(src, lineNo) {
      const toks = tokenize(src).filter((t) => t.kind !== "bracket");
      if (toks.length === 0) return;
      const first = toks[0];
      if (first.kind === "loop") {
        throw err(lineNo, "een herhaal- of doe-lus moet je met de afspeelknop of stap-knop draaien, niet los met enter.");
      }
      this.execSimple({ toks, line: lineNo, src });
    }

    execSimple(node) {
      const toks = node.toks;
      const line = node.line;
      const env = this.env;
      const first = toks[0];

      // maak (positional)
      if (first.kind === "maak") return this.doMaak(toks, line);
      // print
      if (first.kind === "print") {
        const pexpr = toks.slice(1);
        if (pexpr.length === 1 && pexpr[0].kind === "name") {
          const pr = resolveName(pexpr[0].value, env, line);
          const pb = env.get(pr.name.toLowerCase());
          if (pb && pb.type === "nil") throw err(line, `je probeerde '${pr.name}' te printen, maar die had op dat moment de waarde nil. ik weet niet hoe ik nil moet printen.`);
        }
        const v = evalExpr(pexpr, env, line);
        if (v.type === "nil") throw err(line, `je probeerde iets te printen, maar dat had op dat moment de waarde nil. ik weet niet hoe ik nil moet printen.`);
        this.ctx.print(strOf(v));
        return;
      }
      // play
      if (first.kind === "play") return this.doPlay(toks, line);

      // reassignment  naam = expr
      if (first.kind === "name" && toks[1] && toks[1].kind === "op" && toks[1].value === "=") {
        const r = resolveName(first.value, env, line);
        const key = r.name.toLowerCase();
        if (!env.has(key)) {
          const rhs = node.src.slice(node.src.indexOf("=") + 1).trim();
          throw err(line, `je probeerde '${rhs}' toe te kennen aan '${r.name}' op regel ${line}, maar '${r.name}' bestaat op dat moment nog niet. misschien wil je '${r.name}' eerst maken met 'maak ${r.name}'?`);
        }
        const v = evalExpr(toks.slice(2), env, line);
        env.set(key, { type: v.type, value: v.value });
        return;
      }

      // verb statement (free order)
      const verbTok = toks.find((t) => t.kind === "verb");
      if (verbTok) return this.doVerb(verbTok, toks, line);

      // a lone name? probably the child summoning capability help; treat as error gently
      throw err(line, `ik weet niet wat ik met '${node.src}' moet doen. een regel begint meestal met 'maak', 'print', of een werkwoord zoals 'vooruit'.`);
    }

    doMaak(toks, line) {
      const env = this.env;
      const nameTok = toks[1];
      if (!nameTok || (nameTok.kind !== "name" && nameTok.kind !== "type" && nameTok.kind !== "verb")) {
        throw err(line, "na 'maak' verwacht ik een naam, bijvoorbeeld 'maak pietje schildpad'.");
      }
      if (TYPES.has(nameTok.value.toLowerCase()) || VERBS[nameTok.value.toLowerCase()] || KEYWORDS.has(nameTok.value.toLowerCase())) {
        throw err(line, `'${nameTok.value}' is een gereserveerd woord; je kunt het niet als naam gebruiken.`);
      }
      const r = resolveName(nameTok.value, env, line);
      const key = r.name.toLowerCase();
      const rest = toks.slice(2);

      // maak <naam>
      if (rest.length === 0) { env.set(key, { type: "nil", value: null, name: r.name }); return; }

      // maak <naam> <type> [= expr]
      if (rest[0].kind === "type") {
        const t = rest[0].value.toLowerCase();
        if (t === "schildpad") {
          const turtle = this.summon(r.name);
          env.set(key, { type: "schildpad", value: turtle, name: r.name });
        } else {
          env.set(key, { type: t, value: null, name: r.name });
        }
        if (rest[1] && rest[1].kind === "op" && rest[1].value === "=") {
          const v = evalExpr(rest.slice(2), env, line);
          const b = env.get(key);
          if (t === "draairichting" || t === "getal") b.value = v.value;
        }
        return;
      }
      // maak <naam> = expr
      if (rest[0].kind === "op" && rest[0].value === "=") {
        const v = evalExpr(rest.slice(1), env, line);
        env.set(key, { type: v.type, value: v.value, name: r.name });
        return;
      }
      throw err(line, `ik begreep '${toks.map((t) => t.text).join(" ")}' niet. probeer 'maak ${r.name} schildpad' of 'maak ${r.name} = 0'.`);
    }

    summon(name) {
      const idx = this.turtleOrder.length;
      const turtle = { name, x: 0, y: 0, heading: 0, pen: this.ctx.defaultPen(idx), penDown: true, tint: idx, _fresh: true };
      this.turtleOrder.push(turtle);
      this.ctx.onSummon(turtle);
      return turtle;
    }

    doVerb(verbTok, toks, line) {
      const env = this.env;
      const name = verbTok.value.toLowerCase();
      const sig = VERBS[name];
      const slots = {};            // type -> value
      let hasRandom = false;
      const exprToks = [];         // leftover tokens for the getal slot expression

      for (const tk of toks) {
        if (tk === verbTok) continue;
        if (tk.kind === "random") { hasRandom = true; continue; }
        if (tk.kind === "dir") { slots.draairichting = DIRECTIONS[tk.value.toLowerCase()]; continue; }
        if (tk.kind === "colour") { slots.kleur = tk.value.toLowerCase(); continue; }
        if (tk.kind === "name") {
          const r = resolveName(tk.value, env, line);
          const b = env.get(r.name.toLowerCase());
          if (!b) {
            if (r.built) throw err(line, `ik probeerde '${r.name}' te vinden, opgebouwd uit '${r.raw.split("'")[0]}' en de waarde van ${r.raw.split("'").slice(1).join("'")}, maar die bestaat niet.`);
            throw err(line, `ik ken geen '${r.name}'. heb je die al gemaakt met 'maak ${r.name} schildpad'?`);
          }
          if (b.type === "schildpad") slots.schildpad = b.value;
          else if (b.type === "draairichting") slots.draairichting = b.value;
          else if (b.type === "getal") exprToks.push({ kind: "number", value: b.value, text: r.name });
          else throw err(line, `'${r.name}' is een ${typeNL(b.type)}; daar weet '${name}' geen raad mee.`);
          continue;
        }
        // numbers / operators / parens go to the arithmetic expression
        if (tk.kind === "number" || tk.kind === "op" || tk.kind === "paren") exprToks.push(tk);
      }

      // random sampler
      if (hasRandom) {
        if (!sig.sampler) throw err(line, `'${name}' kan niets willekeurig kiezen. random werkt bij 'vooruit' (een afstand) en 'draai' (links of rechts).`);
        if (sig.sampler.type === "getal") slots.getal = Math.round(sig.sampler.lo + Math.random() * (sig.sampler.hi - sig.sampler.lo));
        else if (sig.sampler.type === "draairichting") slots.draairichting = Math.random() < 0.5 ? -90 : 90;
      } else if (exprToks.length) {
        slots.getal = evalExpr(exprToks, env, line).value;
      }

      // verify all required slots present
      for (const need of sig.slots) {
        if (slots[need] === undefined) {
          if (need === "schildpad") throw err(line, `'${name}' wil weten welke schildpad het moet besturen, maar ik zie er geen op deze regel. bijvoorbeeld: '${name === "draai" ? "draai links pietje" : name + " 50 pietje"}'.`);
          if (need === "getal") throw err(line, `'${name}' wil een getal (een afstand), maar dat zie ik niet op deze regel. bijvoorbeeld '${name} 50 ...'.`);
          if (need === "draairichting") throw err(line, `'${name}' wil een richting, zoals 'links' of 'rechts'.`);
          if (need === "kleur") throw err(line, `'${name}' wil een kleur, zoals 'rood' of 'blauw'.`);
        }
      }

      const t = slots.schildpad;
      if (name === "vooruit" || name === "achteruit") {
        let d = slots.getal; if (name === "achteruit") d = -d;
        this.ctx.move(t, d);
      } else if (name === "draai") {
        t.heading = (((t.heading + slots.draairichting) % 360) + 360) % 360;
        this.ctx.onTurn(t);
      } else if (name === "pen") {
        t.pen = this.ctx.colorOf(slots.kleur); t.penName = slots.kleur; t.penDown = true;
        this.ctx.onTurn(t);
      } else if (name === "penomhoog") { t.penDown = false; this.ctx.onTurn(t); }
      else if (name === "penomlaag") { t.penDown = true; this.ctx.onTurn(t); }
    }

    doPlay(toks, line) {
      const rest = toks.slice(1);
      if (rest.length === 1 && rest[0].kind === "number") { this.ctx.audio({ kind: "freq", freq: rest[0].value }); return; }
      const notes = [];
      const NOTE = { a: 440, b: 493.88, c: 261.63, d: 293.66, e: 329.63, f: 349.23, g: 392.0 };
      for (const tk of rest) {
        if (tk.kind === "number") notes.push(tk.value);
        else if (tk.kind === "name" && NOTE[tk.value.toLowerCase()[0]]) notes.push(NOTE[tk.value.toLowerCase()[0]]);
      }
      if (!notes.length) throw err(line, "play wil een toonhoogte (zoals 'play 440') of noten (zoals 'play a b c d').");
      this.ctx.audio({ kind: "seq", notes });
    }
  }

  window.Schildpad = {
    TYPES, DIRECTIONS, COLORS, VERBS, KEYWORDS,
    tokenize, analyze, compile, Engine, fmtNum, resolveName,
  };
})();
