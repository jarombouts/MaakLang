# Prototype (reference only)

This is the original interactive JavaScript prototype of Schildpad — a single-file web
playground that proved the *feel* works end to end: typed-slot free-order resolution, the
`§2.1` introspection-driven help bar, dynamic name resolution, verb-bound `random`, the
patient Dutch errors, the chunky integer-scaled framebuffer, the transport, and the custom
on-screen keyboard.

**It is a golden-behaviour reference, not the architecture being shipped.** The real core is
a Rust crate (see `../ARCHITECTURE.md`). Specifically, port the *behaviour* — the error
sentences, the vocabulary, `fmtNum`, the arithmetic grammar, the turtle/font/wrap model — but
**rebuild** these three mechanisms, which the prototype fakes or gets wrong on exactly the
axes the spec says are load-bearing:

1. **Typed-slot resolution** is a kind-dispatch that is correct only by coincidence (no verb
   yet has two same-typed slots); the `LANGUAGE.md §2` pairwise-distinct invariant is
   unenforced. Build a real `PartialFrame`.
2. **Dynamic-name error reconstruction** is string-surgered at two call sites rather than
   being the byproduct of resolution `§7.1` promises.
3. **The run loop** has a synchronous 50,000-step cap that *silently* truncates — a compass
   violation. Execution must be `step()`-only, host-clocked, never silently capped.

Plus: several silent token-drops (`schuin = 45 draairichting` drops the cast) must become hard
errors, and the vocabulary — forked across `schildpad-lang.js`, `schildpad-keyboard.jsx`, and
`Schildpad.html` and already drifting — is unified into the single `../vocab.ron`.

## Files

- `schildpad-lang.js` — the interpreter (tokenize / analyze / compile / Engine).
- `schildpad-turtle.js`, `schildpad-font.js` — framebuffer + 8×8 font. Cleanly portable.
- `Schildpad.html` — the full wired demo.
- `schildpad-*.jsx`, `tweaks-panel.jsx` — the UI.
- `screenshots/` — the prototype running (design + behaviour reference).
