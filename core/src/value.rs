//! Runtime values and number formatting.

use alloc::string::String;
use alloc::vec::Vec;

use crate::vocab::Type;

/// A handle into the engine's turtle store.
pub type TurtleId = usize;

/// A runtime value. Every binding holds one. The `Type` of a binding is tracked separately
/// (see `env::Binding`) because a `nil`-typed binding has no value yet.
#[derive(Debug, Clone)]
pub enum Value {
    Getal(f64),
    Draairichting(i64), // signed degrees
    Schildpad(TurtleId),
    Kleur(&'static str), // a colour name from the vocab (hex lives host-side)
    Tekst(String),
    Toon(Toon),
    Deuntje(Deuntje),
    Oscillator(&'static str),
    Omhullende(&'static str),
    /// A partially-applied verb — a curry-named function value (LANGUAGE.md §14, Phase 3).
    Frame(crate::frame::PartialFrame),
    Nil,
}

impl Value {
    pub fn type_of(&self) -> Type {
        match self {
            Value::Getal(_) => Type::Getal,
            Value::Draairichting(_) => Type::Draairichting,
            Value::Schildpad(_) => Type::Schildpad,
            Value::Kleur(_) => Type::Kleur,
            Value::Tekst(_) => Type::Tekst,
            Value::Toon(_) => Type::Toon,
            Value::Deuntje(_) => Type::Deuntje,
            Value::Oscillator(_) => Type::Oscillator,
            Value::Omhullende(_) => Type::Omhullende,
            Value::Frame(_) => Type::Nil, // a frame is its own thing; not surfaced as a type yet
            Value::Nil => Type::Nil,
        }
    }
}

/// One tone: a pitch (None = a rest / `stilte`) plus a duration in beats, and the
/// oscillator/envelope preset names that shape it. Depth lives here; the surface only sets
/// pitch + duration (LANGUAGE.md §13).
#[derive(Debug, Clone, PartialEq)]
pub struct Toon {
    pub pitch_hz: Option<f32>,
    pub beats: f32, // duration in beats; `do2` = 2.0, `do/4` = 0.25 (§13)
    pub osc: &'static str,
    pub env: &'static str,
}

impl Toon {
    pub fn rest(beats: f32) -> Self {
        Toon { pitch_hz: None, beats, osc: "sinus", env: "kort" }
    }
    pub fn pitched(hz: f32, beats: f32) -> Self {
        Toon { pitch_hz: Some(hz), beats, osc: "sinus", env: "kort" }
    }
}

/// A tune: a sequence of tones and rests (LANGUAGE.md §13).
#[derive(Debug, Clone, PartialEq)]
pub struct Deuntje {
    pub voices: Vec<Toon>,
    pub tempo_bpm: u16,
}

/// Render a number the way the spec wants: `12`, not `12.0`; decimals to 2 places (§3).
pub fn fmt_num(x: f64) -> String {
    use alloc::format;
    if x == (x as i64) as f64 {
        format!("{}", x as i64)
    } else {
        // round to 2 dp (no_std: integer-cast rounding, no f64::round), then trim trailing zeros
        let scaled = x * 100.0;
        let rounded = if scaled >= 0.0 { (scaled + 0.5) as i64 } else { (scaled - 0.5) as i64 };
        let r = rounded as f64 / 100.0;
        let mut s = format!("{r:.2}");
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        s
    }
}

/// Render any value to a string for `print` / string concatenation.
pub fn str_of(v: &Value, turtle_name: impl Fn(TurtleId) -> String) -> String {
    use alloc::format;
    use alloc::string::ToString;
    match v {
        Value::Tekst(s) => s.clone(),
        Value::Getal(n) => fmt_num(*n),
        Value::Draairichting(d) => fmt_num(*d as f64),
        Value::Kleur(c) => c.to_string(),
        Value::Schildpad(id) => turtle_name(*id),
        Value::Oscillator(o) => o.to_string(),
        Value::Omhullende(e) => e.to_string(),
        Value::Toon(t) => match t.pitch_hz {
            Some(hz) => format!("toon {}", fmt_num(hz as f64)),
            None => "stilte".to_string(),
        },
        Value::Deuntje(d) => format!("deuntje ({} tonen)", d.voices.len()),
        Value::Frame(_) => "actie".to_string(),
        Value::Nil => "nil".to_string(),
    }
}
