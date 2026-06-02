//! Syntax-highlighting spans for the editor (DESIGN_BRIEF §3, issue #49).
//!
//! This is a pure projection of the lexer's classification into a small set of colour KINDS —
//! no parsing, no evaluation. The SAME kinds drive editor highlighting, the egui editor, and
//! the on-screen keyboard palette, so there is one colour-by-kind scheme everywhere (issue #50).
//! The machine never rewrites the child's text (no auto-casing); colour is the only signal that
//! distinguishes a keyword from a name (DESIGN_BRIEF §3).

use alloc::vec::Vec;

use crate::lexer::{tokenize, Tok};

/// The colour kind of a token. Each kind gets exactly one colour, shared editor ↔ palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The machine's control words: maak, print, herhaal, doe, keer, stop, als, anders,
    /// wrapmode, and the als-header compare words (groter/kleiner/gelijk).
    Keyword,
    /// Actions: vooruit, achteruit, draai, pen, penomhoog, penomlaag, play.
    Verb,
    /// Reserved type words: schildpad, getal, draairichting, toon, deuntje.
    Type,
    /// Colour names (rood, blauw, …). The host may tint these as the colour itself.
    Colour,
    /// Note literals (do re mi fa sol la si).
    Note,
    /// Other builtin value-words: links, rechts, stilte, and the oscillator/envelope presets.
    Value,
    /// `random` — the one source of unpredictability (§5); deliberately its own colour.
    Random,
    /// A numeric literal.
    Number,
    /// A string literal.
    Text,
    /// Operators and brackets: + - * / = ( ) [ ].
    Op,
    /// A user identifier — the child's own words.
    Name,
    /// A stray character (lexically suspect; pair with `ok=false`).
    Unknown,
}

impl Kind {
    /// A stable lowercase tag for the host (JSON value / palette key matching).
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Keyword => "keyword",
            Kind::Verb => "verb",
            Kind::Type => "type",
            Kind::Colour => "colour",
            Kind::Note => "note",
            Kind::Value => "value",
            Kind::Random => "random",
            Kind::Number => "number",
            Kind::Text => "text",
            Kind::Op => "op",
            Kind::Name => "name",
            Kind::Unknown => "unknown",
        }
    }
}

/// One highlighted span: a token's position (1-based line, 0-based column, length — both in
/// characters) and its colour kind. `ok=false` marks a lexically suspect token (unclosed
/// string, stray character) for a soft, non-alarming underline (issue #49).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub len: u32,
    pub kind: Kind,
    pub ok: bool,
}

fn kind_of(t: &Tok) -> Kind {
    match t {
        Tok::Maak | Tok::Print | Tok::WrapMode | Tok::Loop(_) | Tok::LoopKeer | Tok::Stop
        | Tok::If | Tok::Else | Tok::Compare(_) => Kind::Keyword,
        Tok::Random => Kind::Random,
        Tok::Verb(_) => Kind::Verb,
        Tok::Type(_) => Kind::Type,
        Tok::Colour(_) => Kind::Colour,
        Tok::Note(_) => Kind::Note,
        Tok::Const(_) | Tok::Osc(_) | Tok::Env(_) => Kind::Value,
        Tok::Number(_) => Kind::Number,
        Tok::Str(_) => Kind::Text,
        Tok::Op(_) | Tok::LParen | Tok::RParen | Tok::LBracket | Tok::RBracket => Kind::Op,
        Tok::Name(_) => Kind::Name,
        Tok::Unknown(_) => Kind::Unknown,
    }
}

/// Tokenize each line of `src` and return a flat list of colour spans in document order.
pub fn highlight(src: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    for (i, line) in src.split('\n').enumerate() {
        for tok in tokenize(line) {
            spans.push(Span {
                line: i as u32 + 1,
                col: tok.col as u32,
                len: tok.len as u32,
                kind: kind_of(&tok.kind),
                ok: tok.ok,
            });
        }
    }
    spans
}
