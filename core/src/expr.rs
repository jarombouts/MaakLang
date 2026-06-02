//! Expression evaluation: ordinary infix arithmetic (`+ - * /`, parens, unary minus, normal
//! precedence) inside a slot or after `=` (LANGUAGE.md §2). String `+` concatenates. This is
//! the *expression* layer; free-order typed-slot resolution is the *statement* layer (frame.rs).
//!
//! Strict, never fuzzy: leftover tokens are a hard error, never silently dropped.

use alloc::string::ToString;

use crate::env::Env;
use crate::error::{err, ErrorKind, SchildpadError};
use crate::lexer::{Tok, Token};
use crate::resolve::resolve_name;
use crate::value::{fmt_num, Toon, Value};
use crate::vocab::{self, ConstVal};

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    env: &'a Env,
    line: u32,
}

/// Evaluate a complete expression and require that it consumes every token.
pub fn eval(toks: &[Token], env: &Env, line: u32) -> Result<Value, SchildpadError> {
    let mut p = Parser { toks, pos: 0, env, line };
    let v = p.parse_expr()?;
    if p.pos != p.toks.len() {
        let leftover = p.remaining_text();
        return Err(err(line, ErrorKind::UnconsumedTokens { leftover }));
    }
    Ok(v)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }
    fn remaining_text(&self) -> alloc::string::String {
        use alloc::string::String;
        let mut s = String::new();
        for t in &self.toks[self.pos..] {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&tok_text(&t.kind));
        }
        s
    }

    fn parse_expr(&mut self) -> Result<Value, SchildpadError> {
        self.parse_add()
    }

    fn parse_add(&mut self) -> Result<Value, SchildpadError> {
        let mut left = self.parse_mul()?;
        while let Some(Token { kind: Tok::Op(op @ (b'+' | b'-')), .. }) = self.peek() {
            let op = *op;
            self.pos += 1;
            let right = self.parse_mul()?;
            left = self.apply_op(op, left, right)?;
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Value, SchildpadError> {
        let mut left = self.parse_primary()?;
        while let Some(Token { kind: Tok::Op(op @ (b'*' | b'/')), .. }) = self.peek() {
            let op = *op;
            self.pos += 1;
            let right = self.parse_primary()?;
            left = self.apply_op(op, left, right)?;
        }
        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Value, SchildpadError> {
        let line = self.line;
        let tok = match self.peek() {
            Some(t) => t.kind.clone(),
            None => return Err(err(line, ErrorKind::ExpectedMoreInExpr)),
        };
        match tok {
            Tok::LParen => {
                self.pos += 1;
                let v = self.parse_expr()?;
                if let Some(Token { kind: Tok::RParen, .. }) = self.peek() {
                    self.pos += 1;
                }
                Ok(v)
            }
            Tok::Op(b'-') => {
                self.pos += 1;
                let v = self.parse_primary()?;
                Ok(Value::Getal(-self.to_num(&v)?))
            }
            Tok::Number(n) => {
                self.pos += 1;
                Ok(Value::Getal(n))
            }
            Tok::Str(s) => {
                self.pos += 1;
                Ok(Value::Tekst(s))
            }
            Tok::Const(c) => {
                self.pos += 1;
                Ok(match vocab::constant(c) {
                    Some(ConstVal::Draai(d)) => Value::Draairichting(d),
                    Some(ConstVal::Getal(n)) => Value::Getal(n as f64),
                    Some(ConstVal::Rest) => Value::Toon(Toon::rest(1.0)),
                    None => Value::Nil,
                })
            }
            Tok::Note(name, beats) => {
                self.pos += 1;
                let hz = vocab::note_freq(name).unwrap_or(440.0);
                Ok(Value::Toon(Toon::pitched(hz, beats)))
            }
            Tok::Colour(c) => {
                self.pos += 1;
                Ok(Value::Kleur(c))
            }
            Tok::Osc(o) => {
                self.pos += 1;
                Ok(Value::Oscillator(o))
            }
            Tok::Env(e) => {
                self.pos += 1;
                Ok(Value::Omhullende(e))
            }
            Tok::Random => Err(err(line, ErrorKind::BareRandom)),
            Tok::Name(raw) => {
                self.pos += 1;
                let r = resolve_name(&raw, self.env, line)?;
                match self.env.get(&r.name) {
                    None => {
                        if r.built {
                            Err(err(line, ErrorKind::DynamicNameNotFound { full: r.name, prefix: r.prefix, var: r.var }))
                        } else {
                            Err(err(line, ErrorKind::UnknownName { name: r.name }))
                        }
                    }
                    Some(b) => {
                        if matches!(b.value, Value::Nil) {
                            Err(err(line, ErrorKind::NilInExpr { name: r.name }))
                        } else {
                            Ok(b.value.clone())
                        }
                    }
                }
            }
            other => {
                self.pos += 1;
                Err(err(line, ErrorKind::NotUnderstood { text: tok_text(&other) }))
            }
        }
    }

    fn to_num(&self, v: &Value) -> Result<f64, SchildpadError> {
        match v {
            Value::Getal(n) => Ok(*n),
            Value::Draairichting(d) => Ok(*d as f64),
            other => Err(err(
                self.line,
                ErrorKind::TypeMismatch { wanted: "getal".to_string(), got: other.type_of().nl().to_string() },
            )),
        }
    }

    fn apply_op(&self, op: u8, a: Value, b: Value) -> Result<Value, SchildpadError> {
        if op == b'+' && (matches!(a, Value::Tekst(_)) || matches!(b, Value::Tekst(_))) {
            let mut s = str_simple(&a);
            s.push_str(&str_simple(&b));
            return Ok(Value::Tekst(s));
        }
        let x = self.to_num(&a)?;
        let y = self.to_num(&b)?;
        Ok(match op {
            b'+' => Value::Getal(x + y),
            b'-' => Value::Getal(x - y),
            b'*' => Value::Getal(x * y),
            b'/' => {
                if y == 0.0 {
                    return Err(err(self.line, ErrorKind::DivideByZero));
                }
                Value::Getal(x / y)
            }
            _ => Value::Getal(x),
        })
    }
}

/// A turtle-free string rendering for expression-level concatenation.
fn str_simple(v: &Value) -> alloc::string::String {
    match v {
        Value::Tekst(s) => s.clone(),
        Value::Getal(n) => fmt_num(*n),
        Value::Draairichting(d) => fmt_num(*d as f64),
        Value::Kleur(c) => c.to_string(),
        _ => "nil".to_string(),
    }
}

/// Best-effort source text of a token, for error messages.
pub fn tok_text(t: &Tok) -> alloc::string::String {
    use alloc::string::String;
    match t {
        Tok::Number(n) => fmt_num(*n),
        Tok::Str(s) => {
            let mut q = String::from("\"");
            q.push_str(s);
            q.push('"');
            q
        }
        Tok::Op(c) => String::from(*c as char),
        Tok::LParen => "(".to_string(),
        Tok::RParen => ")".to_string(),
        Tok::LBracket => "[".to_string(),
        Tok::RBracket => "]".to_string(),
        Tok::Name(s) => s.clone(),
        Tok::Verb(s) | Tok::Colour(s) | Tok::Osc(s) | Tok::Env(s) | Tok::Const(s) => s.to_string(),
        Tok::Note(s, _) => s.to_string(),
        Tok::Type(t) => t.nl().to_string(),
        Tok::Maak => "maak".to_string(),
        Tok::Print => "print".to_string(),
        Tok::Random => "random".to_string(),
        Tok::WrapMode => "wrapmode".to_string(),
        Tok::Loop(w) => w.to_string(),
        Tok::LoopKeer => "keer".to_string(),
        Tok::Stop => "stop".to_string(),
        Tok::If => "als".to_string(),
        Tok::Else => "anders".to_string(),
        Tok::Compare(_) => "vergelijking".to_string(),
        Tok::Unknown(c) => String::from(*c),
    }
}
