# Schildpad — Build Roadmap

The order of work, phased so each phase produces something testable and nothing is built
before the thing it depends on. Phases map 1:1 onto GitHub milestones; the checklist items
map onto issues. The compass (`LANGUAGE.md` §1) and the surface/runtime split
(`ARCHITECTURE.md` §0) govern every item.

Legend: **[surface]** changes what the child types · **[runtime]** grows the engine only ·
**[tooling]** dev/docs/CI · **[host]** an app.

---

## Phase 0 — Workspace & ground truth  *(foundation)*

- [ ] Cargo workspace scaffold (`core`, `cli`, empty `ffi-uniffi`/`ffi-c`/`wasm`/`lsp`). **[runtime/tooling]**
- [ ] `core` builds as `#![no_std] + alloc`; CI job builds it for a bare-metal target so an accidental `std` import fails fast. **[tooling]**
- [ ] `vocab.ron` loader (serde) + the schema (`schemars` → JSON Schema for authoring validation). **[tooling]**
- [ ] Loader validates the **pairwise-distinct slot invariant** and refuses to boot otherwise (`LANGUAGE.md` §2). **[runtime]**
- [ ] Golden-test harness in `cli`: run a program, capture the `Event` stream, assert. Seed it with the prototype's Dutch error sentences as fixtures. **[tooling]**

## Phase 1 — The real interpreter core  *(behaviour parity with the prototype, minus its bugs)*

- [ ] Tokenizer (incl. string literals, numbers, operators, dynamic-name `a-'i` / `a-'(expr)`). **[runtime]**
- [ ] **Real typed-slot resolution**: a `PartialFrame { verb, filled, remaining }` that drives fill-by-type, fires on full, and is queryable for the help system. *Not* the prototype's kind-dispatch. **[runtime]**
- [ ] Expression evaluator (infix `+ - * /`, parens, unary minus, string `+`, divide-by-zero error). **[runtime]**
- [ ] `maak` positional forms incl. `<naam>`, `<naam> <type>`, `<naam> = <expr>`, `<naam> <type> = <expr>`, and the §3.1 cast-on-right `schuin = 45 draairichting` (works **or** hard-errors — never silently drops the cast). **[surface/runtime]**
- [ ] Dynamic name resolution returns the structured `(literal, value)` decomposition → §7.1 error is a true byproduct. **[runtime]**
- [ ] Verb effects for v1 vocab (`vooruit`/`achteruit`/`draai`/`pen`/`penomhoog`/`penomlaag`). **[runtime]**
- [ ] **Fixed-point integer turtle math** with a deterministic sin/cos table (no float trig). **[runtime]**
- [ ] **Core-owned seeded PRNG** (`reset(seed)`); verb-bound `random` samplers from `vocab.ron`; bare `random` errors. **[runtime]**
- [ ] `print` + the full Tier-2 error catalogue as structured `ErrorKind + args`. **[runtime]**
- [ ] Loops: `herhaal n`, `doe n keer`, unbounded `doe`; `wrapmode`. **[surface/runtime]**
- [ ] **`step()`-only execution contract**; no synchronous run-to-completion path anywhere. **[runtime]**
- [ ] Convert every prototype silent token-drop into a hard `UnconsumedTokens` error. **[runtime]**

## Phase 2 — Rendering & framebuffers  *(runtime-only; zero surface)*

- [ ] The `Event`/`DrawOp`/`AudioCmd` command stream as the core↔host contract. **[runtime]**
- [ ] `RenderTarget` descriptor (width = cols×8 fixed; rows/wrap configurable; frozen at reset). **[runtime]**
- [ ] Core computes wrap + movement; emits already-wrapped logical-pixel `Plot` ops. **[runtime]**
- [ ] Core owns the 8×8 font; rasterizes `print` to `GlyphPixels`. **[runtime]**
- [ ] Sprite read-back snapshot (turtles carry their `fb` id; ink is persistent, sprites are per-frame). **[runtime]**
- [ ] Multi-framebuffer plumbing: `Vec<RenderTarget>` + implicit active index; turtle captures buffer at `summon()`. **Ship exactly one buffer; no surface to add/switch.** **[runtime]**

## Phase 3 — The new surface features  *(this version's language additions)*

- [ ] **Curry-named functions [surface]**: `PartialFrame` becomes a first-class value `maak` can bind; captured values **snapshotted at maak-time**; dispatcher enters verb-exec from a bound frame. Hard boundary: single verb, no body. Keep it out of the keyboard/help-bar.
- [ ] **Audio types [surface]**: `toon`, `deuntje` reserved types; `stilte` as a `toon` value; note literals (solfège — confirm) with trailing-number = beats; `play` becomes a verb; emit one `AudioCmd::Sequence` per play; delete the old `doPlay` path.
- [ ] **Audio depth [runtime]**: `oscillator`/`omhullende` preset types as whole-tune overrides; no per-note composite syntax on the surface.
- [ ] **Conditionals [surface]**: header-only `als <expr> <vergelijk> <expr> [ … ]` + optional `anders`; comparison words trapped in the header; **no first-class boolean type**.

## Phase 4 — Introspection, docs, LSP  *(tooling)*

- [ ] `core::introspect`: `what_can_maak`, `fits_next`, `verbs_accepting`, `hover`, `diagnostics` (the Tier-1 typo pass). **[runtime]**
- [ ] `xtask gen-docs`: one page per verb/type/keyword from `vocab.ron`; every example doctested through the real core in CI. **[tooling]**
- [ ] `xtask gen-palette`: keyboard manifest (verbs grouped, colour keys joined from `palette.json`). **[tooling]**
- [ ] Host error formatter: `ErrorKind + args` → Dutch sentence from `ErrorTmpl`. **[tooling]**
- [ ] `schildpad-lsp` (tower-lsp) — **deferred**; thin adapter over `introspect`, built only when an external editor needs it. **[tooling]**

## Phase 5 — The hosts  *(the apps — "where it lives")*

- [ ] **egui app [host]**: link `core` directly; framebuffer sink + audio synth (cpal/fundsp); transport, editor with live syntax + `fits_next` autocomplete, current-line highlight, status bar, error surface. Dogfood the introspection API here first.
- [ ] **UniFFI binding [tooling]**: `ffi-uniffi` → `Schildpad.xcframework` → local Swift Package; one-button `make ios`; CI rebuild.
- [ ] **SwiftUI iPad app [host]**: the full `DESIGN_BRIEF.md` — landscape split pane, chunky framebuffer, transport cluster, custom on-screen keyboard palette (generated), status bar with oscilloscopes, calm error surface. AVAudioEngine for synth.

## Phase 6 — Doors framed, not walked  *(explicitly NOT v1 — keep the architecture ready)*

- [ ] Embedded "device-as-dumb-sink" demo (stream `DrawOp`s over BLE/serial). **[host]**
- [ ] Conditionals gated on a **sensor** (edge-collision / pointer-reactivity) so `als` becomes embodied, not abstract. **[surface]**
- [ ] Samples (host-curated named registry, no file path in the language — preserves §11). **[runtime]**
- [ ] User procedures with bodies + parameters (real functions, beyond curry-naming). **[surface]**
- [ ] A real `klopt` boolean type, if play-testing shows the child wants to store/print truth values. **[surface]**
- [ ] Maintained WASM web playground (only if kept green in CI). **[tooling/host]**

---

## Decisions still open (flagged in the specs, cheap to change before they're built)

1. **Note literals: solfège (`do re mi …`) vs letters (`a b c …`).** Solfège is the
   recommended default (letters collide with the variable names `LANGUAGE.md` §8's own
   examples use). Confirm before Phase 3.
2. **Trailing number on a note = duration-in-beats** (recommended) vs octave. Pick one.
3. **`pen <kleur>` re-dropping a lifted pen** — the prototype silently re-drops; decide
   whether setting a colour should leave pen up/down untouched (the compass dislikes the
   hidden state change).
4. **Keep `achteruit`?** Implemented and clean, but not in `LANGUAGE.md` §10's original
   vocab. Include deliberately or drop to keep the surface tiny.
5. **`play random`** over the seven notes — include in v1, or hold like conditionals were
   meant to be held?
