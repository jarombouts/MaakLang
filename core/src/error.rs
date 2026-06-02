//! Tier-2 runtime errors: structured `ErrorKind` + a Dutch renderer.
//!
//! The kind carries the *reconstructed intent* (the built dynamic name and its parts, the
//! wanted/given types, …) so `render_nl` is a faithful, patient account — never a guess, never
//! cute (LANGUAGE.md §8). The structured form keeps the door open for host-side formatting /
//! localisation (ARCHITECTURE.md §7); the default Dutch rendering lives here so the core is
//! usable and testable on its own.

use alloc::format;
use alloc::string::{String, ToString};

#[derive(Debug, Clone)]
pub struct SchildpadError {
    pub line: u32,
    pub kind: ErrorKind,
}

impl SchildpadError {
    pub fn render_nl(&self) -> String {
        self.kind.render_nl(self.line)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    AssignToUndeclared { rhs: String, name: String },
    PrintNil { name: String },
    NilInExpr { name: String },
    BareRandom,
    /// `full` is the computed name (e.g. `a-7`), `prefix` the literal part (`a-`), `var` the
    /// source text of the interpolated expression (`i`). Reconstructed from real data, not by
    /// re-splitting source — so §7.1's "error for free" actually holds.
    DynamicNameNotFound { full: String, prefix: String, var: String },
    UnknownName { name: String },
    UnknownNameTurtleHint { name: String },
    ReservedAsName { word: String },
    VerbWantsTurtle { verb: String, example: String },
    VerbWantsNumber { verb: String },
    VerbWantsDirection { verb: String },
    VerbWantsColour { verb: String },
    WrongTypeForVerb { name: String, ty: String, verb: String },
    TypeMismatch { wanted: String, got: String },
    DivideByZero,
    UnconsumedTokens { leftover: String },
    ExpectedMoreInExpr,
    NotUnderstood { text: String },
    NotAStatement { src: String },
    RandomVerbCannotSample { verb: String },
    LoopViaEnter,
    MaakNeedsName,
    MaakNotUnderstood { src: String, name: String },
    /// Left of `=` must be a nameable word, not a value (`maak 0 = score`). (LANGUAGE.md §4)
    MaakNameNotAWord { word: String },
    /// `maak hoe_ver = vooruit random` — currying `random` would freeze one draw. (§14)
    CurriedRandom { verb: String },
    /// `maak x = play …` — `play` can't be stored half-finished; make a deuntje instead. (§14)
    CurryPlay,
}

impl ErrorKind {
    pub fn render_nl(&self, line: u32) -> String {
        use ErrorKind::*;
        match self {
            AssignToUndeclared { rhs, name } => format!(
                "je probeerde '{rhs}' toe te kennen aan '{name}' op regel {line}, maar '{name}' \
                 bestaat op dat moment nog niet. misschien wil je '{name}' eerst maken met 'maak {name}'?"
            ),
            PrintNil { name } => format!(
                "je probeerde '{name}' te printen, maar die had op dat moment de waarde nil. \
                 ik weet niet hoe ik nil moet printen."
            ),
            NilInExpr { name } => format!(
                "'{name}' bestaat wel, maar had op dat moment de waarde nil. ik weet niet hoe ik \
                 daar mee moet rekenen."
            ),
            BareRandom => "random waarvan? ik kan alleen willekeurig kiezen als er een werkwoord \
                voor staat dat weet waaruit het mag kiezen, zoals 'vooruit random' of 'draai random'."
                .to_string(),
            DynamicNameNotFound { full, prefix, var } => format!(
                "ik probeerde '{full}' te vinden, opgebouwd uit '{prefix}' en de waarde van {var}, \
                 maar die bestaat niet."
            ),
            UnknownName { name } => format!(
                "ik ken geen '{name}'. misschien moet je die eerst maken met 'maak {name}'?"
            ),
            UnknownNameTurtleHint { name } => format!(
                "ik ken geen '{name}'. heb je die al gemaakt met 'maak {name} schildpad'?"
            ),
            ReservedAsName { word } => {
                format!("'{word}' is een gereserveerd woord; je kunt het niet als naam gebruiken.")
            }
            VerbWantsTurtle { verb, example } => format!(
                "'{verb}' wil weten welke schildpad het moet besturen, maar ik zie er geen op deze \
                 regel. bijvoorbeeld: '{example}'."
            ),
            VerbWantsNumber { verb } => format!(
                "'{verb}' wil een getal (een afstand), maar dat zie ik niet op deze regel. \
                 bijvoorbeeld '{verb} 50 ...'."
            ),
            VerbWantsDirection { verb } => {
                format!("'{verb}' wil een richting, zoals 'links' of 'rechts'.")
            }
            VerbWantsColour { verb } => {
                format!("'{verb}' wil een kleur, zoals 'rood' of 'blauw'.")
            }
            WrongTypeForVerb { name, ty, verb } => {
                format!("'{name}' is een {ty}; daar weet '{verb}' geen raad mee.")
            }
            TypeMismatch { wanted, got } => {
                format!("ik wilde hier een {wanted}, maar kreeg een {got}.")
            }
            DivideByZero => "ik kan niet door nul delen.".to_string(),
            UnconsumedTokens { leftover } => format!(
                "ik snapte het begin van regel {line}, maar wist niet wat ik met '{leftover}' moest. \
                 ik laat liever niets stiekem vallen."
            ),
            ExpectedMoreInExpr => {
                "ik verwachtte hier nog een getal of een naam, maar de regel hield op.".to_string()
            }
            NotUnderstood { text } => format!("ik begreep '{text}' hier niet."),
            NotAStatement { src } => format!(
                "ik weet niet wat ik met '{src}' moet doen. een regel begint meestal met 'maak', \
                 'print', of een werkwoord zoals 'vooruit'."
            ),
            RandomVerbCannotSample { verb } => format!(
                "'{verb}' kan niets willekeurig kiezen. random werkt bij 'vooruit' (een afstand) en \
                 'draai' (links of rechts)."
            ),
            LoopViaEnter => "een herhaal- of doe-lus moet je met de afspeelknop of stap-knop draaien, \
                niet los met enter."
                .to_string(),
            MaakNeedsName => {
                "na 'maak' verwacht ik een naam, bijvoorbeeld 'maak pietje schildpad'.".to_string()
            }
            MaakNotUnderstood { src, name } => format!(
                "ik begreep '{src}' niet. probeer 'maak {name} schildpad' of 'maak {name} = 0'."
            ),
            MaakNameNotAWord { word } => format!(
                "ik kan hier geen naam van maken: links van '=' hoort een naam te staan, niet '{word}'. \
                 bijvoorbeeld 'maak score = 0'."
            ),
            CurriedRandom { verb } => format!(
                "een actie met 'random' erin kan ik niet opslaan: dan zou ik '{verb} random' één keer \
                 gooien en dat ene getal voor altijd onthouden. schrijf '{verb} random pietje' los in je \
                 lus, dan gooit hij elke keer opnieuw."
            ),
            CurryPlay => "je kunt 'play' niet half af opslaan als actie. maak liever een deuntje: \
                'maak liedje deuntje = do re mi'."
                .to_string(),
        }
    }
}

/// Convenience constructor.
pub fn err(line: u32, kind: ErrorKind) -> SchildpadError {
    SchildpadError { line, kind }
}
