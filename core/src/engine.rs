//! The execution engine: compile source into a statement tree, then drive it under the
//! step/play/pause/loop transport (LANGUAGE.md §9, ARCHITECTURE.md §4).
//!
//! `step()` executes exactly one statement and returns the events it produced. The host owns
//! the clock; there is NO synchronous run-to-completion path here, so a `herhaal 99999` stays
//! interruptible. Turtles move via fixed-point integer math; the engine emits already-wrapped
//! `Plot` ops (the host is a dumb pixel sink).

use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::command::{AudioCmd, DrawOp, Event, Sprite, Voice, WrapMode};
use crate::env::{Binding, Env};
use crate::error::{err, ErrorKind, SchildpadError};
use crate::expr;
use crate::fixed::{self, FixedPos};
use crate::frame;
use crate::lexer::{tokenize, Cmp, Tok, Token};
use crate::resolve::resolve_name;
use crate::rng::{Rng, DEFAULT_SEED};
use crate::value::{str_of, Deuntje, Toon, Value};
use crate::vocab::{self, ConstVal, Type};

const DEFAULT_PENS: &[&str] = &["groen", "blauw", "geel", "rood", "oranje", "paars", "cyaan", "roze"];

#[derive(Debug, Clone)]
enum Stmt {
    Simple { line: u32, toks: Rc<Vec<Token>>, src: String },
    Loop { line: u32, unbounded: bool, count: Option<Rc<Vec<Token>>>, body: Rc<Vec<Stmt>> },
    If { line: u32, lhs: Rc<Vec<Token>>, cmp: Cmp, rhs: Rc<Vec<Token>>, then_b: Rc<Vec<Stmt>>, else_b: Option<Rc<Vec<Stmt>>> },
}

impl Stmt {
    fn line(&self) -> u32 {
        match self {
            Stmt::Simple { line, .. } | Stmt::Loop { line, .. } | Stmt::If { line, .. } => *line,
        }
    }
}

#[derive(Clone)]
struct ExecFrame {
    list: Rc<Vec<Stmt>>,
    idx: usize,
    kind: FK,
}

#[derive(Clone)]
enum FK {
    Once,
    Count { reps: usize },
    Unbounded,
}

#[derive(Debug, Clone)]
struct Framebuffer {
    cols: u16,
    rows: u16,
    wrap: WrapMode,
    cur_col: u16,
    cur_row: u16,
}

impl Framebuffer {
    fn w(&self) -> i64 {
        self.cols as i64 * 8
    }
    fn h(&self) -> i64 {
        self.rows as i64 * 8
    }
}

#[derive(Debug, Clone)]
struct Turtle {
    name: String,
    pos: FixedPos, // unwrapped logical px (Q8); wrapped only on plot/sprite
    heading: i64,
    pen: &'static str,
    pen_down: bool,
    tint: u8,
    buffer: u8,
}

pub struct Engine {
    program: Rc<Vec<Stmt>>,
    env: Env,
    turtles: Vec<Turtle>,
    rng: Rng,
    fbs: Vec<Framebuffer>,
    active_fb: usize,
    stack: Vec<ExecFrame>,
    done: bool,
    seed: u64,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            program: Rc::new(Vec::new()),
            env: Env::new(),
            turtles: Vec::new(),
            rng: Rng::new(DEFAULT_SEED),
            fbs: alloc::vec![Framebuffer { cols: 40, rows: 30, wrap: WrapMode::Wrap, cur_col: 0, cur_row: 0 }],
            active_fb: 0,
            stack: Vec::new(),
            done: true,
            seed: DEFAULT_SEED,
        }
    }

    /// Set the render target for buffer 0 (host-supplied; LANGUAGE never sets this).
    pub fn set_render_target(&mut self, cols: u16, rows: u16) {
        self.fbs[0].cols = cols.max(1);
        self.fbs[0].rows = rows.max(1);
    }

    pub fn reset_seed(&mut self, seed: u64) {
        self.seed = if seed == 0 { DEFAULT_SEED } else { seed };
    }

    pub fn done(&self) -> bool {
        self.done
    }

    /// Compile `src` and reset to the defined initial state. Returns the clearing events.
    pub fn load(&mut self, src: &str) -> Vec<Event> {
        let program = compile(src);
        self.program = Rc::new(program);
        self.reset()
    }

    /// Reset to the defined initial state (LANGUAGE.md §9): cleared buffers, empty env, cursor
    /// at (0,0), PRNG re-seeded. Replay reproduces exactly.
    pub fn reset(&mut self) -> Vec<Event> {
        self.env.clear();
        self.turtles.clear();
        self.rng = Rng::new(self.seed);
        for fb in &mut self.fbs {
            fb.cur_col = 0;
            fb.cur_row = 0;
        }
        self.active_fb = 0;
        self.done = self.program.is_empty();
        self.stack = if self.program.is_empty() {
            Vec::new()
        } else {
            alloc::vec![ExecFrame { list: self.program.clone(), idx: 0, kind: FK::Once }]
        };
        alloc::vec![Event::Draw(DrawOp::Clear { fb: 0 })]
    }

    /// The line that will execute next (for the editor highlight), or None.
    pub fn current_line(&self) -> Option<u32> {
        for fr in self.stack.iter().rev() {
            if fr.idx < fr.list.len() {
                return Some(fr.list[fr.idx].line());
            }
        }
        None
    }

    /// A read-back snapshot of every live turtle, for the host's sprite layer.
    pub fn sprites(&self) -> Vec<Sprite> {
        self.turtles
            .iter()
            .enumerate()
            .map(|(id, t)| {
                let fb = &self.fbs[t.buffer as usize];
                Sprite {
                    id,
                    fb: t.buffer,
                    x: fixed::wrap(t.pos.px_x(), fb.w()) as u16,
                    y: fixed::wrap(t.pos.px_y(), fb.h()) as u16,
                    heading_deg: norm_deg(t.heading) as u16,
                    tint: t.tint,
                    pen_down: t.pen_down,
                }
            })
            .collect()
    }

    /// Execute exactly one statement and return its events. The unit of Step (§9).
    pub fn step(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        if self.done {
            return out;
        }

        // unwind finished frames (loop bookkeeping)
        loop {
            let top = match self.stack.last_mut() {
                Some(f) => f,
                None => break,
            };
            if top.idx < top.list.len() {
                break;
            }
            match &mut top.kind {
                FK::Count { reps } if *reps > 0 => {
                    *reps -= 1;
                    top.idx = 0;
                }
                FK::Unbounded => {
                    if top.list.is_empty() {
                        self.stack.pop();
                    } else {
                        top.idx = 0;
                    }
                }
                _ => {
                    self.stack.pop();
                }
            }
        }

        if self.stack.is_empty() {
            self.done = true;
            out.push(Event::Done);
            return out;
        }

        // take the current node (clone the Rc list so we don't borrow self.stack while mutating)
        let (list, idx) = {
            let top = self.stack.last().unwrap();
            (top.list.clone(), top.idx)
        };
        self.stack.last_mut().unwrap().idx += 1;
        let node = list[idx].clone();
        out.push(Event::Line(node.line()));

        let res = self.exec_node(&node, &mut out);
        if let Err(e) = res {
            out.push(Event::Error(e));
            self.done = true; // halt; the host parks the transport on this line (§9)
        }
        out
    }

    /// Live per-line execution (the Enter gesture, §9). Equivalent to Step against live state.
    pub fn run_line(&mut self, src: &str, line: u32) -> Vec<Event> {
        let mut out = Vec::new();
        let toks: Vec<Token> = tokenize(src).into_iter().filter(|t| !is_bracket(&t.kind)).collect();
        if toks.is_empty() {
            return out;
        }
        if matches!(toks[0].kind, Tok::Loop(_) | Tok::If) {
            out.push(Event::Error(err(line, ErrorKind::LoopViaEnter)));
            return out;
        }
        let node = Stmt::Simple { line, toks: Rc::new(toks), src: src.trim().to_string() };
        if let Err(e) = self.exec_node(&node, &mut out) {
            out.push(Event::Error(e));
        }
        out
    }

    fn exec_node(&mut self, node: &Stmt, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        match node {
            Stmt::Simple { line, toks, src } => self.exec_simple(toks, *line, src, out),
            Stmt::Loop { line, unbounded, count, body } => {
                if *unbounded {
                    self.stack.push(ExecFrame { list: body.clone(), idx: 0, kind: FK::Unbounded });
                } else {
                    let n = match count {
                        Some(c) => {
                            let v = expr::eval(c, &self.env, *line)?;
                            // floor for a non-negative count == truncation toward zero
                            num_of(&v, *line)?.max(0.0) as usize
                        }
                        None => 0,
                    };
                    if n > 0 {
                        self.stack.push(ExecFrame { list: body.clone(), idx: 0, kind: FK::Count { reps: n - 1 } });
                    }
                }
                Ok(())
            }
            Stmt::If { line, lhs, cmp, rhs, then_b, else_b } => {
                let l = num_of(&expr::eval(lhs, &self.env, *line)?, *line)?;
                let r = num_of(&expr::eval(rhs, &self.env, *line)?, *line)?;
                let truth = match cmp {
                    Cmp::Groter => l > r,
                    Cmp::Kleiner => l < r,
                    Cmp::Gelijk => l == r,
                };
                if truth {
                    self.stack.push(ExecFrame { list: then_b.clone(), idx: 0, kind: FK::Once });
                } else if let Some(e) = else_b {
                    self.stack.push(ExecFrame { list: e.clone(), idx: 0, kind: FK::Once });
                }
                Ok(())
            }
        }
    }

    /// `stop` (§6.1): unwind the execution-frame stack up to and including the nearest enclosing
    /// loop frame, so execution resumes after that loop. Outside any loop it is a no-op.
    fn do_stop(&mut self) {
        let in_loop = self.stack.iter().any(|f| matches!(f.kind, FK::Count { .. } | FK::Unbounded));
        if !in_loop {
            return; // no enclosing herhaal/doe → no-op
        }
        while let Some(fr) = self.stack.last() {
            let is_loop = matches!(fr.kind, FK::Count { .. } | FK::Unbounded);
            self.stack.pop();
            if is_loop {
                break;
            }
        }
    }

    fn exec_simple(&mut self, toks: &[Token], line: u32, src: &str, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        if toks.is_empty() {
            return Ok(());
        }
        match &toks[0].kind {
            Tok::Maak => self.do_maak(toks, line),
            Tok::Print => self.do_print(toks, line, out),
            Tok::WrapMode => self.do_wrapmode(toks, line, out),
            Tok::Stop => {
                if toks.len() > 1 {
                    return Err(err(line, ErrorKind::UnconsumedTokens { leftover: expr::tok_text(&toks[1].kind) }));
                }
                self.do_stop();
                Ok(())
            }
            Tok::Random => Err(err(line, ErrorKind::BareRandom)),
            Tok::Name(_) if matches!(toks.get(1).map(|t| &t.kind), Some(Tok::Op(b'='))) => {
                self.do_reassign(toks, line, src)
            }
            _ => {
                // a verb statement (free order). find the single verb token.
                if let Some(vtok) = toks.iter().find(|t| matches!(t.kind, Tok::Verb(_))) {
                    if let Tok::Verb(name) = vtok.kind {
                        if name == "play" {
                            return self.do_play(toks, line, out);
                        }
                        return self.do_verb(name, toks, line, out);
                    }
                }
                Err(err(line, ErrorKind::NotAStatement { src: src.to_string() }))
            }
        }
    }

    // ---- maak ----------------------------------------------------------------
    // `maak` is positional only around `=` (LANGUAGE.md §4): everything LEFT of `=` (or the
    // whole tail when there is no `=`) is the binding target — exactly one name plus an optional
    // type, in EITHER order — and the value, if any, is whatever follows `=`. Because type words
    // are reserved (§3.1), the type token can only be the type and the other token can only be
    // the name, so `maak pietje schildpad` and `maak schildpad pietje` are identical.
    fn do_maak(&mut self, toks: &[Token], line: u32) -> Result<(), SchildpadError> {
        let tail = &toks[1..];
        if tail.is_empty() {
            return Err(err(line, ErrorKind::MaakNeedsName));
        }
        // split target (left of `=`) from value (right of `=`)
        let eq = tail.iter().position(|t| t.kind == Tok::Op(b'='));
        let (target, value_toks) = match eq {
            Some(i) => (&tail[..i], Some(&tail[i + 1..])),
            None => (tail, None),
        };

        // resolve the name/type pair from the target, in any order.
        let mut name_raw: Option<String> = None;
        let mut ty: Option<Type> = None;
        for t in target {
            match &t.kind {
                Tok::Type(tt) => {
                    if ty.is_some() {
                        // a second type word where the name belongs → reserved word used as a name
                        return Err(err(line, ErrorKind::ReservedAsName { word: expr::tok_text(&t.kind) }));
                    }
                    ty = Some(*tt);
                }
                Tok::Name(raw) => {
                    if name_raw.is_some() {
                        return Err(err(line, ErrorKind::MaakNotUnderstood {
                            src: joined_src(toks),
                            name: name_raw.unwrap(),
                        }));
                    }
                    name_raw = Some(raw.clone());
                }
                // reserved non-type words can never be a name
                Tok::Verb(_) | Tok::Colour(_) | Tok::Osc(_) | Tok::Env(_) | Tok::Note(_)
                | Tok::Const(_) | Tok::Maak | Tok::Print | Tok::Random | Tok::Loop(_)
                | Tok::LoopKeer | Tok::If | Tok::Else | Tok::Stop | Tok::WrapMode | Tok::Compare(_) => {
                    return Err(err(line, ErrorKind::ReservedAsName { word: expr::tok_text(&t.kind) }));
                }
                // numbers, strings, operators, parens — not a nameable word (`maak 0 = …`)
                _ => return Err(err(line, ErrorKind::MaakNameNotAWord { word: expr::tok_text(&t.kind) })),
            }
        }

        let raw = match name_raw {
            Some(r) => r,
            None => {
                // a lone type word (`maak schildpad`) reads as a reserved word used as a name;
                // nothing at all is just a missing name.
                return Err(match ty {
                    Some(t) => err(line, ErrorKind::ReservedAsName { word: t.nl().to_string() }),
                    None => err(line, ErrorKind::MaakNeedsName),
                });
            }
        };
        let r = resolve_name(&raw, &self.env, line)?;
        let name = r.name;

        // ---- bind ----
        match ty {
            None => match value_toks {
                None => self.env.set(Binding { ty: Type::Nil, value: Value::Nil, name }),
                Some(vals) => {
                    let v = expr::eval(vals, &self.env, line)?;
                    let ty = v.type_of();
                    self.env.set(Binding { ty, value: v, name });
                }
            },
            Some(Type::Schildpad) => {
                if value_toks.is_some() {
                    return Err(err(line, ErrorKind::MaakNotUnderstood { src: joined_src(toks), name }));
                }
                let id = self.summon(&name);
                self.env.set(Binding { ty: Type::Schildpad, value: Value::Schildpad(id), name });
            }
            Some(Type::Deuntje) => {
                let value = match value_toks {
                    Some(vals) => Value::Deuntje(self.gather_deuntje(vals, line)?),
                    None => Value::Nil,
                };
                self.env.set(Binding { ty: Type::Deuntje, value, name });
            }
            Some(t) => {
                let value = match value_toks {
                    Some(vals) => coerce_to(t, expr::eval(vals, &self.env, line)?, line)?,
                    None => Value::Nil,
                };
                self.env.set(Binding { ty: t, value, name });
            }
        }
        Ok(())
    }

    fn summon(&mut self, name: &str) -> usize {
        let idx = self.turtles.len();
        let t = Turtle {
            name: name.to_string(),
            pos: FixedPos::from_px(0, 0),
            heading: 0,
            pen: DEFAULT_PENS[idx % DEFAULT_PENS.len()],
            pen_down: true,
            tint: idx as u8,
            buffer: self.active_fb as u8,
        };
        self.turtles.push(t);
        idx
    }

    // ---- reassign (incl. §3.1 cast-on-right) ---------------------------------
    fn do_reassign(&mut self, toks: &[Token], line: u32, src: &str) -> Result<(), SchildpadError> {
        let raw = if let Tok::Name(raw) = &toks[0].kind { raw.clone() } else { unreachable!() };
        let r = resolve_name(&raw, &self.env, line)?;
        if !self.env.has(&r.name) {
            let rhs = src.split_once('=').map(|(_, b)| b.trim().to_string()).unwrap_or_default();
            return Err(err(line, ErrorKind::AssignToUndeclared { rhs, name: r.name }));
        }
        // body after '='
        let body = &toks[2..];
        // cast-on-right: a trailing lone Type token sets the type, then we assign (LANGUAGE.md §3.1)
        let (expr_toks, cast) = match body.last().map(|t| &t.kind) {
            Some(Tok::Type(t)) => (&body[..body.len() - 1], Some(*t)),
            _ => (body, None),
        };
        let v = expr::eval(expr_toks, &self.env, line)?;
        let v = match cast {
            Some(t) => coerce_to(t, v, line)?,
            None => v,
        };
        let ty = v.type_of();
        self.env.set(Binding { ty, value: v, name: r.name });
        Ok(())
    }

    // ---- print ----------------------------------------------------------------
    fn do_print(&mut self, toks: &[Token], line: u32, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        let expr_toks = &toks[1..];
        // precise nil message for a bare name
        if expr_toks.len() == 1 {
            if let Tok::Name(raw) = &expr_toks[0].kind {
                let r = resolve_name(raw, &self.env, line)?;
                if let Some(b) = self.env.get(&r.name) {
                    if matches!(b.value, Value::Nil) {
                        return Err(err(line, ErrorKind::PrintNil { name: r.name }));
                    }
                }
            }
        }
        let v = expr::eval(expr_toks, &self.env, line)?;
        if matches!(v, Value::Nil) {
            return Err(err(line, ErrorKind::PrintNil { name: String::new() }));
        }
        let turtles = &self.turtles;
        let text = str_of(&v, |id| turtles.get(id).map(|t| t.name.clone()).unwrap_or_default());
        let fb_i = self.active_fb;
        let (col, row, cols, rows) = {
            let fb = &self.fbs[fb_i];
            (fb.cur_col, fb.cur_row, fb.cols, fb.rows)
        };
        out.push(Event::Draw(DrawOp::Text { fb: fb_i as u8, col, row, text: text.clone(), colour: "wit" }));
        // advance the glyph cursor, wrapping the column grid
        let mut c = col as u32 + text.chars().count() as u32;
        let mut row = row as u32;
        while c >= cols as u32 {
            c -= cols as u32;
            row += 1;
        }
        if rows > 0 {
            row %= rows as u32;
        }
        let fb = &mut self.fbs[fb_i];
        fb.cur_col = c as u16;
        fb.cur_row = row as u16;
        Ok(())
    }

    // ---- wrapmode --------------------------------------------------------------
    fn do_wrapmode(&mut self, toks: &[Token], line: u32, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        let mode = match toks.get(1).map(|t| &t.kind) {
            Some(Tok::Name(n)) if n.eq_ignore_ascii_case("wrap") => WrapMode::Wrap,
            Some(Tok::Name(n)) if n.eq_ignore_ascii_case("klem") => WrapMode::Clamp,
            _ => return Err(err(line, ErrorKind::NotUnderstood { text: "wrapmode".to_string() })),
        };
        let fb_i = self.active_fb;
        self.fbs[fb_i].wrap = mode.clone();
        out.push(Event::Draw(DrawOp::SetWrap { fb: fb_i as u8, mode }));
        Ok(())
    }

    // ---- turtle verbs ----------------------------------------------------------
    fn do_verb(&mut self, name: &'static str, toks: &[Token], line: u32, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        let sig = vocab::verb(name).expect("verb in table");
        let sv = frame::resolve_turtle_verb(sig, toks, &self.env, &mut self.rng, line)?;
        let id = sv.schildpad.expect("schildpad slot verified");
        match name {
            "vooruit" | "achteruit" => {
                let mut d = sv.getal.unwrap();
                if name == "achteruit" {
                    d = -d;
                }
                self.move_turtle(id, d, out);
            }
            "draai" => {
                let d = sv.draai.unwrap();
                self.turtles[id].heading = norm_deg(self.turtles[id].heading + d);
            }
            "pen" => {
                let t = &mut self.turtles[id];
                t.pen = sv.kleur.unwrap();
                t.pen_down = true;
            }
            "penomhoog" => self.turtles[id].pen_down = false,
            "penomlaag" => self.turtles[id].pen_down = true,
            _ => {}
        }
        Ok(())
    }

    fn move_turtle(&mut self, id: usize, dist: f64, out: &mut Vec<Event>) {
        let (heading, start, pen, pen_down, buffer) = {
            let t = &self.turtles[id];
            (t.heading, t.pos, t.pen, t.pen_down, t.buffer)
        };
        let end = fixed::advance(start, heading, dist);
        let (w, h, wrap) = {
            let fb = &self.fbs[buffer as usize];
            (fb.w(), fb.h(), fb.wrap.clone())
        };
        if pen_down {
            let (x0, y0) = (start.px_x(), start.px_y());
            let (x1, y1) = (end.px_x(), end.px_y());
            let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
            for s in 0..=steps {
                let px = x0 + (x1 - x0) * s / steps;
                let py = y0 + (y1 - y0) * s / steps;
                let (wx, wy) = match wrap {
                    WrapMode::Wrap => (fixed::wrap(px, w), fixed::wrap(py, h)),
                    WrapMode::Clamp => (fixed::clamp(px, w), fixed::clamp(py, h)),
                };
                out.push(Event::Draw(DrawOp::Plot { fb: buffer, x: wx as u16, y: wy as u16, colour: pen }));
            }
        }
        self.turtles[id].pos = end;
    }

    // ---- play (audio) ----------------------------------------------------------
    fn do_play(&mut self, toks: &[Token], line: u32, out: &mut Vec<Event>) -> Result<(), SchildpadError> {
        let sig = vocab::verb("play").expect("play in table");
        let mut voices: Vec<Voice> = Vec::new();
        for t in toks {
            match &t.kind {
                Tok::Verb(_) => continue,
                Tok::Number(n) => voices.push(Voice { pitch_hz: Some(*n as f32), beats: 1, osc: "sinus", env: "kort" }),
                Tok::Note(name) => {
                    let hz = vocab::note_freq(name).unwrap_or(440.0);
                    voices.push(Voice { pitch_hz: Some(hz), beats: 1, osc: "sinus", env: "kort" });
                }
                Tok::Const(c) if matches!(vocab::constant(c), Some(ConstVal::Rest)) => {
                    voices.push(Voice { pitch_hz: None, beats: 1, osc: "sinus", env: "kort" });
                }
                Tok::Random => {
                    if let Some(crate::vocab::Sampler::ChoiceNote(opts)) = sig.sampler {
                        let nm = *self.rng.choice(opts);
                        let hz = vocab::note_freq(nm).unwrap_or(440.0);
                        voices.push(Voice { pitch_hz: Some(hz), beats: 1, osc: "sinus", env: "kort" });
                    }
                }
                Tok::Name(raw) => {
                    let r = resolve_name(raw, &self.env, line)?;
                    match self.env.get(&r.name) {
                        Some(b) => match &b.value {
                            Value::Deuntje(d) => {
                                for v in &d.voices {
                                    voices.push(Voice { pitch_hz: v.pitch_hz, beats: v.beats, osc: v.osc, env: v.env });
                                }
                            }
                            Value::Toon(t) => voices.push(Voice { pitch_hz: t.pitch_hz, beats: t.beats, osc: t.osc, env: t.env }),
                            _ => return Err(err(line, ErrorKind::TypeMismatch { wanted: "deuntje".to_string(), got: b.ty.nl().to_string() })),
                        },
                        None => return Err(err(line, ErrorKind::UnknownName { name: r.name })),
                    }
                }
                _ => {}
            }
        }
        if voices.is_empty() {
            return Err(err(line, ErrorKind::NotUnderstood { text: "play".to_string() }));
        }
        out.push(Event::Audio(AudioCmd::Sequence { tempo_bpm: 120, voices }));
        Ok(())
    }

    /// Gather a run of note/stilte/number tokens into a deuntje (LANGUAGE.md §13).
    fn gather_deuntje(&self, toks: &[Token], line: u32) -> Result<Deuntje, SchildpadError> {
        let mut voices = Vec::new();
        for t in toks {
            match &t.kind {
                Tok::Note(name) => voices.push(Toon::pitched(vocab::note_freq(name).unwrap_or(440.0), 1)),
                Tok::Number(n) => voices.push(Toon::pitched(*n as f32, 1)),
                Tok::Const(c) if matches!(vocab::constant(c), Some(ConstVal::Rest)) => voices.push(Toon::rest(1)),
                _ => return Err(err(line, ErrorKind::UnconsumedTokens { leftover: expr::tok_text(&t.kind) })),
            }
        }
        Ok(Deuntje { voices, tempo_bpm: 120 })
    }
}

fn norm_deg(d: i64) -> i64 {
    ((d % 360) + 360) % 360
}

/// Best-effort source reconstruction from tokens, for error messages.
fn joined_src(toks: &[Token]) -> String {
    let mut s = String::new();
    for t in toks {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&expr::tok_text(&t.kind));
    }
    s
}

fn num_of(v: &Value, line: u32) -> Result<f64, SchildpadError> {
    match v {
        Value::Getal(n) => Ok(*n),
        Value::Draairichting(d) => Ok(*d as f64),
        other => Err(err(line, ErrorKind::TypeMismatch { wanted: "getal".to_string(), got: other.type_of().nl().to_string() })),
    }
}

/// Cast a value to a target type (the §3.1 postfix cast). Errors rather than guessing.
fn coerce_to(t: Type, v: Value, line: u32) -> Result<Value, SchildpadError> {
    let got = v.type_of().nl().to_string();
    match (t, v) {
        (Type::Getal, Value::Getal(n)) => Ok(Value::Getal(n)),
        (Type::Getal, Value::Draairichting(d)) => Ok(Value::Getal(d as f64)),
        (Type::Draairichting, Value::Getal(n)) => Ok(Value::Draairichting(n as i64)),
        (Type::Draairichting, Value::Draairichting(d)) => Ok(Value::Draairichting(d)),
        (Type::Toon, Value::Toon(t)) => Ok(Value::Toon(t)),
        (Type::Oscillator, Value::Oscillator(o)) => Ok(Value::Oscillator(o)),
        (Type::Omhullende, Value::Omhullende(e)) => Ok(Value::Omhullende(e)),
        (Type::Kleur, Value::Kleur(c)) => Ok(Value::Kleur(c)),
        (Type::Tekst, Value::Tekst(s)) => Ok(Value::Tekst(s)),
        (want, _) => Err(err(line, ErrorKind::TypeMismatch { wanted: want.nl().to_string(), got })),
    }
}

fn is_bracket(t: &Tok) -> bool {
    matches!(t, Tok::LBracket | Tok::RBracket)
}

// ---- compile: source text → statement tree (indentation-delimited blocks) ------------------

struct Line {
    indent: usize,
    line_no: u32,
    toks: Vec<Token>,
    trimmed: String,
}

fn compile(src: &str) -> Vec<Stmt> {
    let lines: Vec<Line> = src
        .split('\n')
        .enumerate()
        .map(|(i, s)| {
            let indent = s.chars().take_while(|c| *c == ' ' || *c == '\t').count();
            let toks: Vec<Token> = tokenize(s).into_iter().filter(|t| !is_bracket(&t.kind)).collect();
            Line { indent, line_no: i as u32 + 1, toks, trimmed: s.trim().to_string() }
        })
        .collect();

    let mut p = 0usize;
    parse_block(&lines, &mut p, 0)
}

fn is_blank(l: &Line) -> bool {
    l.toks.is_empty()
}

fn parse_block(lines: &[Line], p: &mut usize, min_indent: usize) -> Vec<Stmt> {
    let mut out = Vec::new();
    while *p < lines.len() {
        if is_blank(&lines[*p]) {
            *p += 1;
            continue;
        }
        if lines[*p].indent < min_indent {
            break;
        }
        let ln_indent = lines[*p].indent;
        let line_no = lines[*p].line_no;
        let first = lines[*p].toks[0].kind.clone();

        match first {
            Tok::Loop(_) => {
                let header = lines[*p].toks.clone();
                *p += 1;
                let child_indent = next_indent(lines, *p, ln_indent + 1);
                let body = parse_block(lines, p, child_indent);
                out.push(make_loop(header, line_no, body));
            }
            Tok::If => {
                let header = lines[*p].toks.clone();
                *p += 1;
                let child_indent = next_indent(lines, *p, ln_indent + 1);
                let then_b = parse_block(lines, p, child_indent);
                // optional `anders` at the same indent as the `als`
                let else_b = parse_else(lines, p, ln_indent);
                out.push(make_if(header, line_no, then_b, else_b));
            }
            _ => {
                let toks = lines[*p].toks.clone();
                let src = lines[*p].trimmed.clone();
                *p += 1;
                out.push(Stmt::Simple { line: line_no, toks: Rc::new(toks), src });
            }
        }
    }
    out
}

fn parse_else(lines: &[Line], p: &mut usize, if_indent: usize) -> Option<Rc<Vec<Stmt>>> {
    // skip blanks
    let mut q = *p;
    while q < lines.len() && is_blank(&lines[q]) {
        q += 1;
    }
    if q < lines.len() && lines[q].indent == if_indent && matches!(lines[q].toks[0].kind, Tok::Else) {
        let else_indent = lines[q].line_no; // placeholder; recompute child indent below
        let _ = else_indent;
        let anders_indent = lines[q].indent;
        *p = q + 1;
        let child_indent = next_indent(lines, *p, anders_indent + 1);
        let body = parse_block(lines, p, child_indent);
        Some(Rc::new(body))
    } else {
        None
    }
}

fn next_indent(lines: &[Line], p: usize, fallback: usize) -> usize {
    let mut q = p;
    while q < lines.len() && is_blank(&lines[q]) {
        q += 1;
    }
    if q < lines.len() {
        lines[q].indent.max(1).min(fallback.max(lines[q].indent))
    } else {
        fallback
    }
}

fn make_loop(header: Vec<Token>, line: u32, body: Vec<Stmt>) -> Stmt {
    // forms: herhaal <n> ; doe <n> keer ; doe
    let w = if let Tok::Loop(w) = &header[0].kind { *w } else { "herhaal" };
    let body = Rc::new(body);
    if w == "herhaal" {
        let count = header[1..].to_vec();
        Stmt::Loop { line, unbounded: false, count: Some(Rc::new(count)), body }
    } else {
        // doe
        if let Some(keer_idx) = header.iter().position(|t| matches!(t.kind, Tok::LoopKeer)) {
            let count = header[1..keer_idx].to_vec();
            Stmt::Loop { line, unbounded: false, count: Some(Rc::new(count)), body }
        } else if header.len() == 1 {
            Stmt::Loop { line, unbounded: true, count: None, body }
        } else {
            let count = header[1..].to_vec();
            Stmt::Loop { line, unbounded: false, count: Some(Rc::new(count)), body }
        }
    }
}

fn make_if(header: Vec<Token>, line: u32, then_b: Vec<Stmt>, else_b: Option<Rc<Vec<Stmt>>>) -> Stmt {
    // als <expr> <vergelijk> <expr>
    let cmp_idx = header.iter().position(|t| matches!(t.kind, Tok::Compare(_)));
    let (lhs, cmp, rhs) = match cmp_idx {
        Some(i) => {
            let cmp = if let Tok::Compare(c) = &header[i].kind { c.clone() } else { Cmp::Gelijk };
            (header[1..i].to_vec(), cmp, header[i + 1..].to_vec())
        }
        None => (header[1..].to_vec(), Cmp::Gelijk, Vec::new()),
    };
    Stmt::If { line, lhs: Rc::new(lhs), cmp, rhs: Rc::new(rhs), then_b: Rc::new(then_b), else_b }
}
