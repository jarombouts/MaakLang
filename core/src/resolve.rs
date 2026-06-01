//! Dynamic name resolution (LANGUAGE.md §7): a name like `a-'i` is the literal `"a-"`
//! concatenated with the runtime value of `i`. We return the *structured* decomposition
//! (the literal prefix and the source text of the interpolated expression) so that a failed
//! lookup reconstructs the child's intent precisely — §7.1's "error for free" is literally a
//! byproduct of how resolution works, computed once here, not string-surgered at call sites.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::env::Env;
use crate::error::SchildpadError;
use crate::expr;
use crate::lexer::tokenize;
use crate::value::{fmt_num, Value};

#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    /// true if the name was built via interpolation (`a-'i`), false for a plain identifier.
    pub built: bool,
    /// the literal part before the first interpolation (`a-`), for the error message.
    pub prefix: String,
    /// the source text of the first interpolated expression (`i`, `(i+1)`), for the message.
    pub var: String,
}

fn val_to_name_part(v: &Value) -> String {
    match v {
        Value::Getal(n) => fmt_num(*n),
        Value::Draairichting(d) => fmt_num(*d as f64),
        Value::Tekst(s) => s.clone(),
        _ => String::new(),
    }
}

pub fn resolve_name(raw: &str, env: &Env, line: u32) -> Result<Resolved, SchildpadError> {
    if !raw.contains('\'') {
        return Ok(Resolved { name: raw.to_string(), built: false, prefix: String::new(), var: String::new() });
    }
    let parts: Vec<&str> = raw.split('\'').collect();
    let prefix = parts[0].to_string();
    let mut name = prefix.clone();
    let mut first_var = String::new();

    for (k, seg) in parts.iter().enumerate().skip(1) {
        let (expr_text, rest): (String, String) = if let Some(stripped) = seg.strip_prefix('(') {
            // (expr)rest — find the matching close paren (flat; resolver exprs are simple)
            if let Some(close) = stripped.find(')') {
                (stripped[..close].to_string(), stripped[close + 1..].to_string())
            } else {
                (stripped.to_string(), String::new())
            }
        } else {
            let split = seg
                .char_indices()
                .find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_'))
                .map(|(i, _)| i)
                .unwrap_or(seg.len());
            (seg[..split].to_string(), seg[split..].to_string())
        };

        let toks = tokenize(&expr_text);
        let v = expr::eval(&toks, env, line)?;
        name.push_str(&val_to_name_part(&v));
        name.push_str(&rest);

        if k == 1 {
            first_var = if seg.starts_with('(') {
                let mut s = String::from("(");
                s.push_str(&expr_text);
                s.push(')');
                s
            } else {
                expr_text.clone()
            };
        }
    }

    Ok(Resolved { name, built: true, prefix, var: first_var })
}
