//! The core → host command/event stream (ARCHITECTURE.md §3). The core owns no device; it
//! emits these and the host blits / synthesises. Colours are carried by NAME (the host maps
//! name → hex via palette.json); positions are already wrapped to logical pixels by the core.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::SchildpadError;

/// Returned by `Engine::step()`. The host processes the list in order.
#[derive(Debug, Clone)]
pub enum Event {
    /// "the machine is now reading this line" — drives the editor highlight (joint attention).
    Line(u32),
    Draw(DrawOp),
    Audio(AudioCmd),
    Error(SchildpadError),
    Done,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WrapMode {
    Wrap,
    Clamp,
}

/// A persistent mutation of a framebuffer. The turtle *sprite* is NOT here — it is read-back
/// state (see `Engine::sprites`); only ink (pen trails, glyphs) is a draw op.
#[derive(Debug, Clone, PartialEq)]
pub enum DrawOp {
    Clear { fb: u8 },
    /// A single pixel, already wrapped into the buffer by the core. `colour` is a vocab name.
    Plot { fb: u8, x: u16, y: u16, colour: &'static str },
    /// Text at a glyph cell. (Phase 1 carries the string; Phase 2 / issue #21 replaces this
    /// with core-rasterised pixel runs so embedded panels render identically.)
    Text { fb: u8, col: u16, row: u16, text: String, colour: &'static str },
    SetWrap { fb: u8, mode: WrapMode },
}

/// A declarative sound description. Synthesis is host-side; pitch is pre-resolved to Hz so
/// the host never parses note names and determinism is preserved (LANGUAGE.md §13).
#[derive(Debug, Clone, PartialEq)]
pub enum AudioCmd {
    Sequence { tempo_bpm: u16, voices: Vec<Voice> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub pitch_hz: Option<f32>, // None = a rest (stilte)
    pub beats: f32,            // duration in beats; `do2` = 2.0, `do/4` = 0.25 (§13)
    pub osc: &'static str,
    pub env: &'static str,
}

/// A read-back snapshot of a live turtle, for the host's sprite layer. `fb` makes a second
/// buffer work later without a rewrite (ARCHITECTURE.md §3, §5).
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub id: usize,
    pub fb: u8,
    pub x: u16,
    pub y: u16,
    pub heading_deg: u16,
    pub tint: u8,
    pub pen_down: bool,
}
