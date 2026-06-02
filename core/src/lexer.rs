//! The tokenizer. Classifies each word against the vocabulary so the parser/resolver work on
//! typed tokens. A name may carry dynamic interpolation (`a-'i`, `a-'(i+1)`) which is captured
//! whole here and expanded in `resolve` (LANGUAGE.md §7).

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::vocab::{self, Type};

#[derive(Debug, Clone, PartialEq)]
pub enum Cmp {
    Groter,
    Kleiner,
    Gelijk,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Maak,
    Print,
    Random,
    WrapMode,
    Loop(&'static str), // herhaal / doe — carries which word, so `doe` ≠ `herhaal`
    LoopKeer,           // keer
    Stop,               // break out of the innermost loop (§6.1)
    If,
    Else,
    Compare(Cmp),
    Verb(&'static str),
    Type(Type),
    Colour(&'static str),
    Osc(&'static str),
    Env(&'static str),
    Note(&'static str, f32), // (name, duration in beats); `do`=1.0, `do2`=2.0, `do/4`=0.25 (§13)
    Const(&'static str), // links / rechts / stilte — resolve via vocab::constant
    Number(f64),
    Str(String),
    Op(u8), // b'+', b'-', b'*', b'/', b'='
    LParen,
    RParen,
    LBracket,
    RBracket,
    /// A user identifier. `raw` keeps the exact text (incl. any `'` interpolation).
    Name(String),
    Unknown(char),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Tok,
    pub col: usize,
    pub len: usize,
    /// false for an unclosed string / unknown char — a Tier-1 "suspect" hint (LANGUAGE.md §8).
    pub ok: bool,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic()
}
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Tokenize one line. Bracket tokens are emitted; the parser filters them.
pub fn tokenize(line: &str) -> Vec<Token> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            i += 1;
            continue;
        }
        let start = i;

        // string literal
        if c == '"' {
            i += 1;
            let mut val = String::new();
            let mut closed = false;
            while i < n {
                if chars[i] == '"' {
                    closed = true;
                    i += 1;
                    break;
                }
                val.push(chars[i]);
                i += 1;
            }
            toks.push(Token { kind: Tok::Str(val), col: start, len: i - start, ok: closed });
            continue;
        }

        // number
        if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < n && (chars[j].is_ascii_digit() || chars[j] == '.') {
                j += 1;
            }
            let text: String = chars[i..j].iter().collect();
            let val = text.parse::<f64>().unwrap_or(0.0);
            toks.push(Token { kind: Tok::Number(val), col: start, len: j - i, ok: true });
            i = j;
            continue;
        }

        // operators
        if matches!(c, '+' | '-' | '*' | '/' | '=') {
            toks.push(Token { kind: Tok::Op(c as u8), col: start, len: 1, ok: true });
            i += 1;
            continue;
        }
        if c == '(' {
            toks.push(Token { kind: Tok::LParen, col: start, len: 1, ok: true });
            i += 1;
            continue;
        }
        if c == ')' {
            toks.push(Token { kind: Tok::RParen, col: start, len: 1, ok: true });
            i += 1;
            continue;
        }
        if c == '[' {
            toks.push(Token { kind: Tok::LBracket, col: start, len: 1, ok: true });
            i += 1;
            continue;
        }
        if c == ']' {
            toks.push(Token { kind: Tok::RBracket, col: start, len: 1, ok: true });
            i += 1;
            continue;
        }

        // identifier / name (may include dynamic - ' interpolation)
        if is_name_start(c) {
            let mut j = i + 1;
            while j < n && is_name_char(chars[j]) {
                j += 1;
            }
            // dynamic continuation: ('...) or (-'...)
            loop {
                if j < n && chars[j] == '\'' {
                    j += 1; // consume '
                    if j < n && chars[j] == '(' {
                        let mut depth = 1;
                        j += 1;
                        while j < n && depth > 0 {
                            if chars[j] == '(' {
                                depth += 1;
                            } else if chars[j] == ')' {
                                depth -= 1;
                            }
                            j += 1;
                        }
                    } else {
                        while j < n && is_name_char(chars[j]) {
                            j += 1;
                        }
                    }
                } else if j + 1 < n && chars[j] == '-' && chars[j + 1] == '\'' {
                    j += 1; // include hyphen; loop handles the '
                } else {
                    break;
                }
            }
            let text: String = chars[i..j].iter().collect();
            let lower = text.to_lowercase();

            // note literal with optional duration: `do`, `do2` (2 beats), `do/3` (1/3 beat) — §13
            if !text.contains('\'') {
                if let Some((nm, mut beats)) = split_note_beats(&lower) {
                    // fractional `note/N` only on a BARE note (no integer suffix already present)
                    if beats == 1.0
                        && lower.bytes().all(|b| b.is_ascii_alphabetic())
                        && j + 1 < n
                        && chars[j] == '/'
                        && chars[j + 1].is_ascii_digit()
                    {
                        let mut k = j + 1;
                        while k < n && chars[k].is_ascii_digit() {
                            k += 1;
                        }
                        let den: u32 = chars[j + 1..k].iter().collect::<String>().parse().unwrap_or(1);
                        beats = 1.0 / den.max(1) as f32;
                        j = k;
                    }
                    toks.push(Token { kind: Tok::Note(nm, beats), col: start, len: j - i, ok: true });
                    i = j;
                    continue;
                }
            }

            let kind = classify(&lower, &text);
            toks.push(Token { kind, col: start, len: j - i, ok: true });
            i = j;
            continue;
        }

        // stray character
        toks.push(Token { kind: Tok::Unknown(c), col: start, len: 1, ok: false });
        i += 1;
    }
    toks
}

/// Classify a lowercased word against the vocabulary. `raw` is preserved for `Name` (so the
/// dynamic interpolation text survives).
fn classify(lower: &str, raw: &str) -> Tok {
    // dynamic names are always user names, never reserved words
    let dynamic = raw.contains('\'');
    if !dynamic {
        if lower == "random" {
            return Tok::Random;
        }
        if let Some(kw) = vocab::keyword(lower) {
            use crate::vocab::KwKind::*;
            return match kw {
                Maak => Tok::Maak,
                Print => Tok::Print,
                Loop => Tok::Loop(if lower == "herhaal" { "herhaal" } else { "doe" }),
                LoopAux => Tok::LoopKeer,
                Break => Tok::Stop,
                If => Tok::If,
                Else => Tok::Else,
                WrapMode => Tok::WrapMode,
                Compare => match lower {
                    "groter" => Tok::Compare(Cmp::Groter),
                    "kleiner" => Tok::Compare(Cmp::Kleiner),
                    _ => Tok::Compare(Cmp::Gelijk),
                },
            };
        }
        if let Some(t) = vocab::reserved_type(lower) {
            return Tok::Type(t);
        }
        if vocab::is_verb(lower) {
            // return the 'static name from the vocab table
            let name = vocab::VERBS.iter().find(|v| v.name == lower).unwrap().name;
            return Tok::Verb(name);
        }
        if let Some((c, _)) = vocab::CONSTANTS.iter().find(|(w, _)| *w == lower) {
            return Tok::Const(c);
        }
        if let Some(col) = vocab::COLOURS.iter().find(|w| **w == lower) {
            return Tok::Colour(col);
        }
        if let Some(o) = vocab::OSCILLATORS.iter().find(|w| **w == lower) {
            return Tok::Osc(o);
        }
        if let Some(e) = vocab::ENVELOPES.iter().find(|w| **w == lower) {
            return Tok::Env(e);
        }
        // note literal, possibly with a trailing duration digit handled by the resolver;
        // here a bare note id matches.
        if let Some((nm, _)) = vocab::NOTES.iter().find(|(w, _)| *w == lower) {
            return Tok::Note(nm, 1.0);
        }
    }
    Tok::Name(raw.to_string())
}

/// A note name with an optional trailing integer beat-count: `do` → ("do", 1.0), `do2` →
/// ("do", 2.0). The `note/N` fractional form is handled by the caller (it spans the `/`). Returns
/// None for a plain identifier, so `dog`, `do_x`, `do2x` stay names (§13).
fn split_note_beats(lower: &str) -> Option<(&'static str, f32)> {
    let split = lower.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(lower.len());
    let (base, rest) = lower.split_at(split);
    if !rest.is_empty() && !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None; // mixed suffix → a plain name, not a note+duration
    }
    let (nm, _) = vocab::NOTES.iter().find(|(w, _)| *w == base)?;
    let beats = if rest.is_empty() { 1.0 } else { rest.parse::<u32>().unwrap_or(1).max(1) as f32 };
    Some((nm, beats))
}
