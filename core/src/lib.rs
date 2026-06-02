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

    fn texts(events: &[Event]) -> Vec<String> {
        events
            .iter()
            .filter_map(|e| match e {
                Event::Draw(DrawOp::Text { text, .. }) => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    fn has_plot_colour(events: &[Event], colour: &str) -> bool {
        events.iter().any(|e| matches!(e, Event::Draw(DrawOp::Plot { colour: c, .. }) if *c == colour))
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

    // ---- #47: maak name/type order-free (LANGUAGE.md §4) ----------------------

    #[test]
    fn maak_name_type_order_is_free() {
        // `maak schildpad pietje` must be identical to `maak pietje schildpad`.
        let a = run("maak pietje schildpad\nvooruit 100 pietje");
        let b = run("maak schildpad pietje\nvooruit 100 pietje");
        assert!(a.1.is_none() && b.1.is_none(), "a={:?} b={:?}", a.1, b.1);
        assert_eq!(plots(&a.0), plots(&b.0));
    }

    #[test]
    fn maak_typed_assign_both_orders() {
        let a = run("maak getal Score = 7\nprint Score");
        let b = run("maak Score getal = 7\nprint Score");
        assert!(a.1.is_none() && b.1.is_none(), "a={:?} b={:?}", a.1, b.1);
        assert_eq!(texts(&a.0), alloc::vec!["7"]);
        assert_eq!(texts(&b.0), alloc::vec!["7"]);
    }

    #[test]
    fn maak_value_left_of_eq_is_illegal() {
        // `maak 0 = score` — the left of `=` must be a nameable word, not a value.
        let (_, err) = run("maak 0 = score");
        let e = err.unwrap();
        assert!(e.contains("links van '='"), "{e}");
    }

    // ---- #48: `stop` breaks the innermost loop (LANGUAGE.md §6.1) --------------

    #[test]
    fn stop_breaks_herhaal() {
        let stopped = run("maak p schildpad\nherhaal 10\n  vooruit 10 p\n  stop");
        let once = run("maak p schildpad\nvooruit 10 p");
        assert!(stopped.1.is_none(), "{:?}", stopped.1);
        assert_eq!(plots(&stopped.0), plots(&once.0), "herhaal+stop should run the body once");
    }

    #[test]
    fn stop_breaks_unbounded_doe() {
        // without `stop` this is an infinite loop; `stop` is the only in-language way out.
        let stopped = run("maak p schildpad\ndoe\n  vooruit 10 p\n  stop");
        let once = run("maak p schildpad\nvooruit 10 p");
        assert!(stopped.1.is_none(), "{:?}", stopped.1);
        assert_eq!(plots(&stopped.0), plots(&once.0));
    }

    #[test]
    fn stop_outside_loop_is_noop() {
        let r = run("maak p schildpad\nstop\nvooruit 10 p");
        assert!(r.1.is_none(), "{:?}", r.1);
        assert!(plots(&r.0) > 0, "the line after a top-level stop must still run");
    }

    #[test]
    fn stop_in_nested_if_breaks_the_loop() {
        // stop inside an `als` inside a `herhaal` must unwind through the if to the loop.
        let r = run("maak p schildpad\nmaak i = 0\nherhaal 5\n  vooruit 10 p\n  als i gelijk 0\n    stop");
        let once = run("maak p schildpad\nvooruit 10 p");
        assert!(r.1.is_none(), "{:?}", r.1);
        assert_eq!(plots(&r.0), plots(&once.0));
    }

    // ---- #24: curry-named functions (LANGUAGE.md §14) -------------------------

    #[test]
    fn curry_pen_fills_remaining_turtle_slot() {
        let (events, err) = run("maak roodpen = pen rood\nmaak p schildpad\nroodpen p\nvooruit 10 p");
        assert!(err.is_none(), "{err:?}");
        assert!(has_plot_colour(&events, "rood"), "the curry should have set the pen red");
    }

    #[test]
    fn curry_distance_matches_direct_verb() {
        let curried = run("maak stap = vooruit 50\nmaak p schildpad\nstap p");
        let direct = run("maak p schildpad\nvooruit 50 p");
        assert!(curried.1.is_none(), "{:?}", curried.1);
        assert_eq!(plots(&curried.0), plots(&direct.0));
    }

    #[test]
    fn curry_invocation_is_order_free() {
        let a = run("maak rp = pen rood\nmaak p schildpad\nrp p\nvooruit 5 p");
        let b = run("maak rp = pen rood\nmaak p schildpad\np rp\nvooruit 5 p");
        assert!(a.1.is_none() && b.1.is_none(), "a={:?} b={:?}", a.1, b.1);
        assert!(has_plot_colour(&a.0, "rood") && has_plot_colour(&b.0, "rood"));
    }

    #[test]
    fn curry_snapshots_at_maak_time() {
        // `maak hoek = draai schuin` freezes schuin's value; later changing schuin must NOT
        // change the action (no late binding — the cardinal sin of §1).
        let mut e = Engine::new();
        e.load("maak schuin draairichting = 45\nmaak hoek = draai schuin\nschuin = 90\nmaak p schildpad\nhoek p");
        while !e.done() {
            e.step();
        }
        let sp = e.sprites();
        assert_eq!(sp.len(), 1);
        assert_eq!(sp[0].heading_deg, 45, "curry must freeze schuin at maak-time, not re-read 90");
    }

    #[test]
    fn curry_random_is_a_hard_error() {
        let (_, err) = run("maak hoe_ver = vooruit random");
        let e = err.unwrap();
        assert!(e.contains("los in je lus"), "{e}");
    }

    #[test]
    fn curry_play_is_rejected() {
        let (_, err) = run("maak liedje = play do");
        assert!(err.unwrap().contains("play"));
    }

    #[test]
    fn curry_missing_turtle_errors() {
        let (_, err) = run("maak roodpen = pen rood\nroodpen");
        assert!(err.unwrap().contains("schildpad"));
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
