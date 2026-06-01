//! schildpad-core — the Schildpad language core.
//!
//! A strict, deterministic, left-to-right pipeline interpreter with typed-slot free-order
//! resolution, a tiny reserved type system, verb-bound `random`, dynamic name resolution, and
//! verbose patient Dutch errors. `no_std` + `alloc`: owns no device, emits a command/event
//! stream the host renders. The single source of truth for the language (see ARCHITECTURE.md).

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod command;
pub mod engine;
pub mod env;
pub mod error;
pub mod expr;
pub mod fixed;
pub mod frame;
pub mod lexer;
pub mod resolve;
pub mod rng;
pub mod value;
pub mod vocab;

pub use command::{AudioCmd, DrawOp, Event, Sprite, Voice, WrapMode};
pub use engine::Engine;
pub use error::{ErrorKind, SchildpadError};
pub use value::Value;
pub use vocab::Type;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec::Vec;

    /// Run a program to completion (with a generous host-side step guard) and return its
    /// events plus the first rendered Dutch error, if any.
    fn run(src: &str) -> (Vec<Event>, Option<String>) {
        let mut e = Engine::new();
        let mut events = e.load(src);
        let mut guard = 0;
        while !e.done() && guard < 100_000 {
            events.extend(e.step());
            guard += 1;
        }
        let err = events.iter().find_map(|ev| match ev {
            Event::Error(se) => Some(se.render_nl()),
            _ => None,
        });
        (events, err)
    }

    fn plots(events: &[Event]) -> usize {
        events.iter().filter(|e| matches!(e, Event::Draw(DrawOp::Plot { .. }))).count()
    }

    #[test]
    fn canonical_first_program_runs_clean() {
        let (events, err) = run("maak pietje schildpad\nvooruit 100 pietje\ndraai links pietje\nvooruit 100 pietje");
        assert!(err.is_none(), "unexpected error: {err:?}");
        assert!(plots(&events) > 100, "expected a drawn trail, got {} plots", plots(&events));
    }

    #[test]
    fn free_word_order_is_identical() {
        let a = run("maak pietje schildpad\nvooruit 100 pietje");
        let b = run("maak pietje schildpad\npietje vooruit 100");
        let c = run("maak pietje schildpad\nvooruit pietje 100");
        assert!(a.1.is_none() && b.1.is_none() && c.1.is_none());
        assert_eq!(plots(&a.0), plots(&b.0));
        assert_eq!(plots(&b.0), plots(&c.0));
    }

    #[test]
    fn bare_random_errors() {
        let (_, err) = run("maak x = random");
        assert!(err.unwrap().contains("random waarvan?"));
    }

    #[test]
    fn assign_to_undeclared_errors() {
        let (_, err) = run("maak a = 1\nmaak b = 2\nd = b - a");
        let e = err.unwrap();
        assert!(e.contains("'d'") && e.contains("bestaat op dat moment nog niet"), "{e}");
    }

    #[test]
    fn print_nil_errors() {
        let (_, err) = run("maak leeg\nprint leeg");
        assert!(err.unwrap().contains("de waarde nil"));
    }

    #[test]
    fn reserved_word_as_name_errors() {
        let (_, err) = run("maak schildpad schildpad");
        assert!(err.unwrap().contains("gereserveerd woord"));
    }

    #[test]
    fn user_draairichting_and_star() {
        // maak punt draairichting = 144 ; draai punt — the star demo
        let (_, err) = run("maak pietje schildpad\nmaak punt draairichting = 144\nvooruit 50 pietje\ndraai punt pietje");
        assert!(err.is_none(), "{err:?}");
    }

    #[test]
    fn dynamic_name_not_found_reconstructs_intent() {
        let src = "maak i = 7\ndraai random a-'i pietje";
        // pietje exists but a-7 does not
        let full = "maak pietje schildpad\n".to_string() + src;
        let (_, err) = run(&full);
        let e = err.unwrap();
        assert!(e.contains("'a-7'") && e.contains("'a-'") && e.contains("de waarde van i"), "{e}");
    }

    #[test]
    fn dynamic_name_makes_and_uses_turtles() {
        let src = "maak i = 0\ndoe 3 keer\n  maak a-'i schildpad\n  i = i + 1\nvooruit 10 a-'0";
        let (_, err) = run(src);
        assert!(err.is_none(), "{err:?}");
    }

    #[test]
    fn herhaal_repeats_body() {
        let one = run("maak p schildpad\nvooruit 40 p");
        let four = run("maak p schildpad\nherhaal 4\n  vooruit 40 p\n  draai rechts p");
        assert!(one.1.is_none() && four.1.is_none());
        assert!(plots(&four.0) > plots(&one.0));
    }

    #[test]
    fn conditional_als_anders() {
        let then = run("maak p schildpad\nmaak s = 20\nals s groter 10\n  vooruit 50 p\nanders\n  vooruit 5 p");
        let els = run("maak p schildpad\nmaak s = 3\nals s groter 10\n  vooruit 50 p\nanders\n  vooruit 5 p");
        assert!(then.1.is_none() && els.1.is_none());
        assert!(plots(&then.0) > plots(&els.0), "the true branch should draw more");
    }

    #[test]
    fn cast_on_right_is_not_dropped() {
        // §3.1: `schuin = 45 draairichting` must NOT silently drop the cast.
        let (_, err) = run("maak schuin\nschuin = 45 draairichting\nmaak p schildpad\ndraai schuin p");
        assert!(err.is_none(), "{err:?}");
    }

    #[test]
    fn play_emits_audio() {
        let (events, err) = run("play do re mi");
        assert!(err.is_none());
        let voices = events.iter().find_map(|e| match e {
            Event::Audio(AudioCmd::Sequence { voices, .. }) => Some(voices.len()),
            _ => None,
        });
        assert_eq!(voices, Some(3));
    }

    #[test]
    fn deterministic_replay_with_seed() {
        let mut e = Engine::new();
        e.reset_seed(42);
        e.load("maak p schildpad\nherhaal 20\n  vooruit random p\n  draai random p");
        let mut first = alloc::vec::Vec::new();
        while !e.done() { first.extend(e.step()); }
        // replay with the same seed reproduces exactly
        e.reset_seed(42);
        e.load("maak p schildpad\nherhaal 20\n  vooruit random p\n  draai random p");
        let mut second = alloc::vec::Vec::new();
        while !e.done() { second.extend(e.step()); }
        assert_eq!(plots(&first), plots(&second), "seeded replay must reproduce exactly");
    }
}
