//! C ABI over `schildpad-core`. An opaque `Engine` handle; every call that produces effects
//! returns a JSON array of events as a C string the caller must free with
//! `schildpad_string_free`. Flat, Swift-friendly event schema (tagged by `t`):
//!
//!   {"t":"line","line":4}
//!   {"t":"clear","fb":0}
//!   {"t":"plot","fb":0,"x":10,"y":20,"colour":"rood"}
//!   {"t":"text","fb":0,"col":0,"row":0,"text":"hoi","colour":"wit"}
//!   {"t":"wrap","fb":0,"mode":"wrap"}
//!   {"t":"audio","tempo":120,"voices":[{"hz":261.63,"beats":1,"osc":"sinus","env":"kort"}]}
//!   {"t":"error","line":4,"msg":"…"}
//!   {"t":"done"}
//!
//! Syntax-highlight spans are a separate, stateless call (no engine needed):
//!   schildpad_highlight("maak pietje schildpad")
//!     → [{"line":1,"col":0,"len":4,"kind":"keyword","ok":true}, …]
//!
//! JSON is built by hand here so `core` stays dependency-free (no serde) and the schema is one
//! we control for the Swift side.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};

use schildpad_core::command::{AudioCmd, DrawOp, Sprite, WrapMode};
use schildpad_core::highlight::{self, Span};
use schildpad_core::{Engine, Event};

// ---- JSON helpers ----------------------------------------------------------------

fn esc(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn num(x: f32) -> String {
    // compact: integers without a trailing .0
    if x == (x as i64) as f32 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

pub fn events_to_json(events: &[Event]) -> String {
    let mut s = String::from("[");
    for (i, e) in events.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        event_json(e, &mut s);
    }
    s.push(']');
    s
}

fn event_json(e: &Event, s: &mut String) {
    match e {
        Event::Line(n) => {
            s.push_str(&format!("{{\"t\":\"line\",\"line\":{n}}}"));
        }
        Event::Done => s.push_str("{\"t\":\"done\"}"),
        Event::Error(err) => {
            s.push_str(&format!("{{\"t\":\"error\",\"line\":{},\"msg\":", err.line));
            esc(&err.render_nl(), s);
            s.push('}');
        }
        Event::Draw(d) => draw_json(d, s),
        Event::Audio(a) => audio_json(a, s),
    }
}

fn draw_json(d: &DrawOp, s: &mut String) {
    match d {
        DrawOp::Clear { fb } => s.push_str(&format!("{{\"t\":\"clear\",\"fb\":{fb}}}")),
        DrawOp::Plot { fb, x, y, colour } => {
            s.push_str(&format!("{{\"t\":\"plot\",\"fb\":{fb},\"x\":{x},\"y\":{y},\"colour\":"));
            esc(colour, s);
            s.push('}');
        }
        DrawOp::Text { fb, col, row, text, colour } => {
            s.push_str(&format!("{{\"t\":\"text\",\"fb\":{fb},\"col\":{col},\"row\":{row},\"text\":"));
            esc(text, s);
            s.push_str(",\"colour\":");
            esc(colour, s);
            s.push('}');
        }
        DrawOp::SetWrap { fb, mode } => {
            let m = match mode {
                WrapMode::Wrap => "wrap",
                WrapMode::Clamp => "klem",
            };
            s.push_str(&format!("{{\"t\":\"wrap\",\"fb\":{fb},\"mode\":\"{m}\"}}"));
        }
    }
}

fn audio_json(a: &AudioCmd, s: &mut String) {
    let AudioCmd::Sequence { tempo_bpm, voices } = a;
    s.push_str(&format!("{{\"t\":\"audio\",\"tempo\":{tempo_bpm},\"voices\":["));
    for (i, v) in voices.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let hz = match v.pitch_hz {
            Some(h) => num(h),
            None => "null".to_string(),
        };
        s.push_str(&format!("{{\"hz\":{hz},\"beats\":{},\"osc\":\"{}\",\"env\":\"{}\"}}", v.beats, v.osc, v.env));
    }
    s.push_str("]}");
}

pub fn spans_to_json(spans: &[Span]) -> String {
    let mut s = String::from("[");
    for (i, sp) in spans.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"line\":{},\"col\":{},\"len\":{},\"kind\":\"{}\",\"ok\":{}}}",
            sp.line, sp.col, sp.len, sp.kind.tag(), sp.ok
        ));
    }
    s.push(']');
    s
}

pub fn sprites_to_json(sprites: &[Sprite]) -> String {
    let mut s = String::from("[");
    for (i, sp) in sprites.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"id\":{},\"fb\":{},\"x\":{},\"y\":{},\"heading\":{},\"tint\":{},\"penDown\":{}}}",
            sp.id, sp.fb, sp.x, sp.y, sp.heading_deg, sp.tint, sp.pen_down
        ));
    }
    s.push(']');
    s
}

// ---- C ABI -----------------------------------------------------------------------

fn to_c(s: String) -> *mut c_char {
    CString::new(s).unwrap_or_default().into_raw()
}

unsafe fn cstr<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        ""
    } else {
        CStr::from_ptr(p).to_str().unwrap_or("")
    }
}

#[no_mangle]
pub extern "C" fn schildpad_new() -> *mut Engine {
    Box::into_raw(Box::new(Engine::new()))
}

/// # Safety: `p` must be a pointer from `schildpad_new`, used at most once.
#[no_mangle]
pub unsafe extern "C" fn schildpad_free(p: *mut Engine) {
    if !p.is_null() {
        drop(Box::from_raw(p));
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_set_render_target(p: *mut Engine, cols: u16, rows: u16) {
    if let Some(e) = p.as_mut() {
        e.set_render_target(cols, rows);
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_reset_seed(p: *mut Engine, seed: u64) {
    if let Some(e) = p.as_mut() {
        e.reset_seed(seed);
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_load(p: *mut Engine, src: *const c_char) -> *mut c_char {
    match p.as_mut() {
        Some(e) => to_c(events_to_json(&e.load(cstr(src)))),
        None => to_c("[]".to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_reset(p: *mut Engine) -> *mut c_char {
    match p.as_mut() {
        Some(e) => to_c(events_to_json(&e.reset())),
        None => to_c("[]".to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_step(p: *mut Engine) -> *mut c_char {
    match p.as_mut() {
        Some(e) => to_c(events_to_json(&e.step())),
        None => to_c("[]".to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_run_line(p: *mut Engine, src: *const c_char, line: u32) -> *mut c_char {
    match p.as_mut() {
        Some(e) => to_c(events_to_json(&e.run_line(cstr(src), line))),
        None => to_c("[]".to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_sprites(p: *mut Engine) -> *mut c_char {
    match p.as_mut() {
        Some(e) => to_c(sprites_to_json(&e.sprites())),
        None => to_c("[]".to_string()),
    }
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_done(p: *mut Engine) -> bool {
    p.as_ref().map(|e| e.done()).unwrap_or(true)
}

#[no_mangle]
pub unsafe extern "C" fn schildpad_current_line(p: *mut Engine) -> c_int {
    p.as_ref().and_then(|e| e.current_line()).map(|l| l as c_int).unwrap_or(-1)
}

/// Syntax-highlight `src` into colour spans (stateless; no engine handle needed). Returns a
/// JSON array the caller must free with `schildpad_string_free`.
///
/// # Safety: `src` must be a valid C string or null.
#[no_mangle]
pub unsafe extern "C" fn schildpad_highlight(src: *const c_char) -> *mut c_char {
    to_c(spans_to_json(&highlight::highlight(cstr(src))))
}

/// # Safety: `s` must be a pointer returned by one of the JSON-returning functions.
#[no_mangle]
pub unsafe extern "C" fn schildpad_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_has_clean_schema() {
        let mut e = Engine::new();
        let mut events = e.load("maak pietje schildpad\nvooruit 10 pietje");
        while !e.done() {
            events.extend(e.step());
        }
        let json = events_to_json(&events);
        assert!(json.starts_with('['));
        assert!(json.contains("\"t\":\"clear\""));
        assert!(json.contains("\"t\":\"plot\""));
        assert!(json.contains("\"t\":\"done\""));
        // sprite snapshot
        let sj = sprites_to_json(&e.sprites());
        assert!(sj.contains("\"penDown\":true"));
    }

    #[test]
    fn highlight_json_schema() {
        let json = spans_to_json(&highlight::highlight("maak pietje schildpad"));
        assert!(json.starts_with('['));
        assert!(json.contains("\"kind\":\"keyword\""));
        assert!(json.contains("\"kind\":\"name\""));
        assert!(json.contains("\"kind\":\"type\""));
        assert!(json.contains("\"line\":1"));
        assert!(json.contains("\"ok\":true"));
    }

    #[test]
    fn error_json_escapes() {
        let mut e = Engine::new();
        let mut events = e.load("maak x = random");
        while !e.done() {
            events.extend(e.step());
        }
        let json = events_to_json(&events);
        assert!(json.contains("\"t\":\"error\""));
        assert!(json.contains("random waarvan"));
    }
}
