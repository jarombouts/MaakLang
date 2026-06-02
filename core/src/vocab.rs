//! The vocabulary: the `Type` lattice and the generated lookup tables.
//!
//! The data (`VERBS`, `CONSTANTS`, `NOTES`, …) is generated at build time from
//! `../vocab.ron` by `build.rs` and `include!`d below — so this file defines the *shapes*
//! and the spec defines the *contents*. There is exactly one place to author vocabulary.

use alloc::string::String;

/// The built-in type lattice (LANGUAGE.md §3, §13). `reserved` types can never be variable
/// names and act as postfix casts; `Kleur`/`Oscillator`/`Omhullende`/`Tekst` are runtime
/// value-types that are not surface keywords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Schildpad,
    Getal,
    Draairichting,
    Toon,
    Deuntje,
    Kleur,
    Oscillator,
    Omhullende,
    Tekst,
    Nil,
}

impl Type {
    /// The Dutch name used in error sentences ("ik wilde een getal, maar kreeg een …").
    pub fn nl(self) -> &'static str {
        match self {
            Type::Schildpad => "schildpad",
            Type::Getal => "getal",
            Type::Draairichting => "draairichting",
            Type::Toon => "toon",
            Type::Deuntje => "deuntje",
            Type::Kleur => "kleur",
            Type::Oscillator => "oscillator",
            Type::Omhullende => "omhullende",
            Type::Tekst => "tekst",
            Type::Nil => "nil",
        }
    }
}

/// A verb's signature: its ordered, pairwise-distinct typed slots and (optionally) the
/// domain `random` samples from when this verb holds it (LANGUAGE.md §5).
#[derive(Debug, Clone, Copy)]
pub struct VerbSig {
    pub name: &'static str,
    pub slots: &'static [Type],
    pub sampler: Option<Sampler>,
}

/// The sampler a verb exposes to `random`. Polymorphic by the holding verb — never global.
#[derive(Debug, Clone, Copy)]
pub enum Sampler {
    /// e.g. `vooruit random` — a distance uniform in `[lo, hi]`.
    UniformGetal { lo: i64, hi: i64 },
    /// e.g. `draai random` — uniform over a set of signed angles (degrees).
    ChoiceDraai(&'static [i32]),
    /// e.g. `play random` — uniform over a set of note ids (Phase 3).
    ChoiceNote(&'static [&'static str]),
}

/// A builtin typed constant value (`links`, `rechts`, `stilte`, …).
#[derive(Debug, Clone, Copy)]
pub enum ConstVal {
    Draai(i64),
    Getal(i64),
    Rest,
}

/// What a reserved non-type keyword does. Drives statement dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KwKind {
    Maak,
    Print,
    Loop,
    LoopAux,
    If,
    Else,
    Compare,
    WrapMode,
    Break,
}

// The generated tables: RESERVED_TYPE_WORDS, VERBS, CONSTANTS, COLOURS, OSCILLATORS,
// ENVELOPES, NOTES, KEYWORDS. Authored in ../vocab.ron.
include!(concat!(env!("OUT_DIR"), "/vocab_gen.rs"));

// ---- lookups (linear scans over tiny tables; the vocab is a few dozen entries) ----------

pub fn verb(name: &str) -> Option<&'static VerbSig> {
    VERBS.iter().find(|v| v.name == name)
}

pub fn is_verb(name: &str) -> bool {
    VERBS.iter().any(|v| v.name == name)
}

pub fn keyword(word: &str) -> Option<KwKind> {
    KEYWORDS.iter().find(|(k, _)| *k == word).map(|(_, v)| *v)
}

pub fn reserved_type(word: &str) -> Option<Type> {
    RESERVED_TYPE_WORDS.iter().find(|(w, _)| *w == word).map(|(_, t)| *t)
}

pub fn constant(word: &str) -> Option<ConstVal> {
    CONSTANTS.iter().find(|(w, _)| *w == word).map(|(_, v)| *v)
}

pub fn is_colour(word: &str) -> bool {
    COLOURS.contains(&word)
}

pub fn is_oscillator(word: &str) -> bool {
    OSCILLATORS.contains(&word)
}

pub fn is_envelope(word: &str) -> bool {
    ENVELOPES.contains(&word)
}

pub fn note_freq(word: &str) -> Option<f32> {
    NOTES.iter().find(|(w, _)| *w == word).map(|(_, f)| *f)
}

/// Is `word` reserved in any way (a keyword, a reserved type, a constant, a colour, an
/// oscillator/envelope preset, a verb, or a note)? Used to forbid it as a variable name and
/// to classify tokens. (LANGUAGE.md §3.1: reservation is what keeps `maak` parseable.)
pub fn is_reserved(word: &str) -> bool {
    keyword(word).is_some()
        || reserved_type(word).is_some()
        || constant(word).is_some()
        || is_colour(word)
        || is_oscillator(word)
        || is_envelope(word)
        || is_verb(word)
        || note_freq(word).is_some()
}

/// Every reserved word, for the Tier-1 "did you mean a keyword?" typo check (LANGUAGE.md §8).
pub fn all_reserved_words() -> alloc::vec::Vec<String> {
    use alloc::string::ToString;
    let mut out = alloc::vec::Vec::new();
    for (w, _) in KEYWORDS { out.push(w.to_string()); }
    for (w, _) in RESERVED_TYPE_WORDS { out.push(w.to_string()); }
    for (w, _) in CONSTANTS { out.push(w.to_string()); }
    for w in COLOURS { out.push(w.to_string()); }
    for w in OSCILLATORS { out.push(w.to_string()); }
    for w in ENVELOPES { out.push(w.to_string()); }
    for v in VERBS { out.push(v.name.to_string()); }
    for (w, _) in NOTES { out.push(w.to_string()); }
    out
}
