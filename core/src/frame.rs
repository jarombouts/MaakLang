//! Typed-slot resolution — the real thing (LANGUAGE.md §2), not the prototype's kind-dispatch.
//!
//! A verb declares pairwise-distinct typed slots (enforced at build time). Tokens fill slots
//! BY TYPE in any order; the statement fires when every slot is full. A token that fits no
//! remaining slot is a hard error — nothing is silently dropped. The `getal` slot is filled by
//! an arithmetic *expression* (a run of number/op/paren/getal-name tokens), evaluated once.
//!
//! `PartialFrame` is also the value behind a curry-named function (LANGUAGE.md §14): a verb
//! with some slots pre-filled, waiting to be invoked with the rest. (Construction is Phase 3.)

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::env::Env;
use crate::error::{err, ErrorKind, SchildpadError};
use crate::expr;
use crate::lexer::{Tok, Token};
use crate::resolve::resolve_name;
use crate::rng::Rng;
use crate::value::{TurtleId, Value};
use crate::vocab::{self, ConstVal, Sampler, Type, VerbSig};

/// A partially-applied verb — a curry-named function value (Phase 3).
#[derive(Debug, Clone, PartialEq)]
pub struct PartialFrame {
    pub verb: &'static str,
    // Phase 3: snapshot of slots filled at `maak`-time.
}

/// The slot values needed to fire a turtle verb.
#[derive(Debug, Default, Clone)]
pub struct SlotValues {
    pub getal: Option<f64>,
    pub draai: Option<i64>,
    pub schildpad: Option<TurtleId>,
    pub kleur: Option<&'static str>,
}

fn as_num(v: &Value) -> Option<f64> {
    match v {
        Value::Getal(n) => Some(*n),
        Value::Draairichting(d) => Some(*d as f64),
        _ => None,
    }
}

/// Resolve a turtle-verb statement (`vooruit`/`achteruit`/`draai`/`pen`/`penomhoog`/`penomlaag`)
/// from its tokens, in any order. `toks` includes the verb token (skipped here).
pub fn resolve_turtle_verb(
    sig: &VerbSig,
    toks: &[Token],
    env: &Env,
    rng: &mut Rng,
    line: u32,
) -> Result<SlotValues, SchildpadError> {
    let mut sv = SlotValues::default();
    let mut getal_toks: Vec<Token> = Vec::new();
    let mut has_random = false;
    let wants = |t: Type| sig.slots.contains(&t);

    let leftover_err = |t: &Token| err(line, ErrorKind::UnconsumedTokens { leftover: expr::tok_text(&t.kind) });

    for t in toks {
        match &t.kind {
            Tok::Verb(_) => continue,
            Tok::Number(_) | Tok::Op(_) | Tok::LParen | Tok::RParen => getal_toks.push(t.clone()),
            Tok::Random => has_random = true,
            Tok::Const(c) => match vocab::constant(c) {
                Some(ConstVal::Draai(d)) if wants(Type::Draairichting) => sv.draai = Some(d),
                _ => return Err(leftover_err(t)),
            },
            Tok::Colour(col) if wants(Type::Kleur) => sv.kleur = Some(col),
            Tok::Name(raw) => {
                let r = resolve_name(raw, env, line)?;
                match env.get(&r.name) {
                    None => {
                        return Err(if r.built {
                            err(line, ErrorKind::DynamicNameNotFound { full: r.name, prefix: r.prefix, var: r.var })
                        } else {
                            err(line, ErrorKind::UnknownNameTurtleHint { name: r.name })
                        })
                    }
                    Some(b) => match b.ty {
                        Type::Schildpad => {
                            if let Value::Schildpad(id) = b.value {
                                sv.schildpad = Some(id);
                            }
                        }
                        Type::Draairichting if wants(Type::Draairichting) => {
                            if let Value::Draairichting(d) = b.value {
                                sv.draai = Some(d);
                            }
                        }
                        Type::Getal if wants(Type::Getal) => getal_toks.push(t.clone()),
                        other => {
                            return Err(err(
                                line,
                                ErrorKind::WrongTypeForVerb { name: r.name, ty: other.nl().to_string(), verb: sig.name.to_string() },
                            ))
                        }
                    },
                }
            }
            _ => return Err(leftover_err(t)),
        }
    }

    if has_random {
        match sig.sampler {
            Some(Sampler::UniformGetal { lo, hi }) => sv.getal = Some(rng.range_inclusive(lo, hi) as f64),
            Some(Sampler::ChoiceDraai(opts)) => sv.draai = Some(*rng.choice(opts) as i64),
            _ => return Err(err(line, ErrorKind::RandomVerbCannotSample { verb: sig.name.to_string() })),
        }
    } else if !getal_toks.is_empty() {
        if !wants(Type::Getal) {
            return Err(err(line, ErrorKind::UnconsumedTokens { leftover: expr::tok_text(&getal_toks[0].kind) }));
        }
        let v = expr::eval(&getal_toks, env, line)?;
        sv.getal = Some(as_num(&v).ok_or_else(|| {
            err(line, ErrorKind::TypeMismatch { wanted: "getal".to_string(), got: v.type_of().nl().to_string() })
        })?);
    }

    // every declared slot must be filled (precise "what's missing" errors)
    for slot in sig.slots {
        let present = match slot {
            Type::Getal => sv.getal.is_some(),
            Type::Draairichting => sv.draai.is_some(),
            Type::Schildpad => sv.schildpad.is_some(),
            Type::Kleur => sv.kleur.is_some(),
            _ => true,
        };
        if !present {
            return Err(missing_slot_error(sig, *slot, line));
        }
    }
    Ok(sv)
}

fn missing_slot_error(sig: &VerbSig, slot: Type, line: u32) -> SchildpadError {
    let verb = sig.name.to_string();
    match slot {
        Type::Schildpad => {
            let example = if sig.name == "draai" {
                "draai links pietje".to_string()
            } else {
                let mut s = sig.name.to_string();
                s.push_str(" 50 pietje");
                s
            };
            err(line, ErrorKind::VerbWantsTurtle { verb, example })
        }
        Type::Getal => err(line, ErrorKind::VerbWantsNumber { verb }),
        Type::Draairichting => err(line, ErrorKind::VerbWantsDirection { verb }),
        Type::Kleur => err(line, ErrorKind::VerbWantsColour { verb }),
        _ => err(line, ErrorKind::VerbWantsNumber { verb }),
    }
}
