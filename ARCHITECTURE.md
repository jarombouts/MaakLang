# Schildpad — Runtime & System Architecture

> How the language in `LANGUAGE.md` is actually built, and how one implementation
> serves every target: the iPad SwiftUI app, the Rust `egui` desktop app, and —
> later — weird/embedded screens. This document is the source of truth for the
> *machine*; `LANGUAGE.md` is the source of truth for the *language*; `DESIGN_BRIEF.md`
> is the source of truth for the *iPad UI*. The three must agree on the transport model
> (`LANGUAGE.md` §9), the error surface (§8), and the command stream defined here (§3).

The compass from `LANGUAGE.md` §1 governs this document too, with one corollary that
decides almost every architectural call here:

> **Keep the child-facing language exactly as tiny as `LANGUAGE.md` §4 demands.
> Grow the runtime/VM instead.**

Nearly every capability the project wants beyond the original v1 (weird screen sizes,
multiple framebuffers, embedded targets, the expressive audio model) is a *runtime*
concern, not a *surface* concern. The child never types a resolution, never names a
framebuffer, never writes an oscillator parameter. The engine grows; the keyboard
palette does not. Defend that line hard — it is the cheapest possible way to say "yes"
to ambitious capability without betraying the compass.

---

## 0. The decision, in one paragraph

The core interpreter is **one Rust crate** (`schildpad-core`), written `no_std`-friendly
(`alloc` only, no `std`, no I/O, no device access, no wall-clock, no ambient randomness).
It is the **single source of truth** for tokenizing, resolving, executing, and explaining
the language, identical on every target. It owns no framebuffer and no audio device:
`Engine::step()` executes exactly one statement and **returns a stream of typed
commands/events** (draw ops, audio specs, the current-line marker, structured errors).
The host — `egui`, SwiftUI, an embedded firmware loop — consumes that stream and does the
actual blitting and synthesis. `egui` links the crate directly; SwiftUI consumes it via a
UniFFI-generated Swift package wrapping an `xcframework`; embedded links a thin C-ABI shim;
WASM is an optional secondary build for a browser demo. This is what makes "same input,
same output, always" (the compass) true *across devices*, not just within one.

---

## 1. Why Rust, and why one crate

The product promise is a **single deterministic machine the child can build a theory of.**
If the interpreter is reimplemented per platform (Swift on iPad, Rust on desktop, C on a
device), you get three machines that will drift in exactly the places that matter most:
arithmetic edge cases, the `a-'i` dynamic-name resolver, loop-frame unwinding, slot
resolution order, and — critically — the verbose Dutch error sentences, which are *part of
the product* (read aloud by the parent, `DESIGN_BRIEF.md` §8). Two implementations of "the
error text must say `ik probeerde 'a-7' te vinden, opgebouwd uit 'a-' en de waarde van i`"
will not stay byte-identical. One canonical machine is the whole pitch; three machines is
three theories.

Rust specifically, over the alternatives that were considered:

- **vs. reimplement-per-platform** — rejected. Determinism drift (above) is fatal to the
  compass, and it is the quiet kind of failure that only shows up months later when two
  devices draw the same program differently.
- **vs. Rust → WASM embedded everywhere** — rejected as the *primary* boundary. WASM is one
  artifact but the wrong one for two of the three targets: Apple makes shipping a JIT WASM
  runtime painful and an interpret-only one slow, and a general WASM runtime cannot run on a
  tiny MCU with no allocator — which is the exact "weird/embedded" target this architecture
  is supposed to keep reachable. `egui` gets the native crate for free, so wrapping it in
  WASM there is pure loss. WASM survives only as an *optional* web-demo build of the same
  crate (§6).
- **vs. core in C / a split bytecode VM** — rejected. C throws away Rust's `enum`/`Result`/
  `no_std` story and memory safety in software children poke unpredictably. A shared-compiler/
  forked-VM split is *more* surface to keep in sync, for no benefit — there is no scenario
  where you want the compiler shared but the VM forked.

The prototype (`/tmp/maak-demo/schildpad-lang.js`) already validated this architecture in
the wrong language: its `Engine` takes a `ctx` and `step()`s one statement at a time with
the host owning the clock. That is the correct shape (see §4). The required change when
porting is to **invert** `ctx`-callbacks-that-mutate-a-canvas into a **returned command
stream** (§3).

---

## 2. Workspace layout

```
schildpad/                       (cargo workspace)
  core/        crate schildpad-core   — #![no_std] + extern crate alloc.
               THE source of truth. tokenize · analyze · compile · Engine::step;
               typed-slot resolution; dynamic name resolution; expression eval;
               the command/event types; the 8×8 bitmap font + glyph rasterizer;
               fixed-point turtle math; the seeded PRNG; the introspect API
               (what_can_maak / fits_next / verbs_accepting / hover / diagnostics);
               structured error kinds (NOT formatted strings — see §7).
               Loads vocab.ron into its resolution tables.
               No std. No I/O. No clock. No device. No ambient randomness.

  ffi-uniffi/  crate schildpad-ffi    — thin UniFFI wrapper over core (needs std).
               Generates Schildpad.swift + the C shim; bundled as Schildpad.xcframework
               inside a local Swift Package the iPad/macOS SwiftUI app depends on.

  ffi-c/       crate schildpad-capi   — thin cbindgen #[no_mangle] C-ABI wrapper.
               The lowest-common-denominator escape hatch for embedded targets that
               UniFFI/Swift cannot reach.

  wasm/        crate schildpad-wasm   — wasm-bindgen wrapper. OPTIONAL web-demo/docs
               target only. Never the runtime a flagship host embeds. Kept green in CI
               or treated as throwaway — no middle ground (it must not bit-rot into
               disagreeing with the native path).

  lsp/         crate schildpad-lsp    — thin tower-lsp server over core::introspect.
               Contains ZERO language logic — pure JSON-RPC translation. Exists only for
               out-of-process editors (VS Code / Zed / Neovim) and a future web playground.
               Neither flagship app uses it (they call core::introspect in-process).

  cli/         crate schildpad        — dev/test harness; the golden-test runner;
               xtask subcommands: `gen-docs`, `gen-palette`, `gen-schema`.

  vocab.ron    THE single source of truth for vocabulary (see §8). serde-loaded,
               schemars-validated. Drives resolution tables, docs, LSP, keyboard palette,
               and error templates.

  palette.json The agency-owned name→hex map (colours, oscilloscope tints, chrome).
               SEPARATE from vocab.ron: the NAMES are the language's, the VALUES are the
               agency's (LANGUAGE.md §10). A visual redesign never touches the language spec.
```

**Host apps live outside this workspace** (or in sibling crates/targets), each consuming
the core:

```
app-egui/      Rust egui app (linux/windows/macos). `schildpad-core = { path=".." }`.
app-ios/       SwiftUI app (iPad landscape, later macOS). `import Schildpad`.
```

---

## 3. The core↔host boundary: a command/event stream

This is the single most important contract in the system. **The core owns no device.**
`Engine::step()` executes one statement and returns zero or more `Event`s. The host
interprets them. Everything that can differ between hosts must live behind this boundary
on the *core* side, so it cannot differ.

```rust
// Returned by Engine::step(). Stamped with the source line for the editor highlight.
pub enum Event {
    Line(u32),                       // "the machine is now reading this line" (joint attention)
    Draw(DrawOp),                    // a persistent mutation of a framebuffer
    Audio(AudioCmd),                 // a declarative sound description (host synthesizes)
    Error { line: u32, kind: ErrorKind, args: ErrorArgs },   // structured; host formats Dutch (§7)
    Done,                            // program reached end / halting condition
}

pub enum DrawOp {
    SetFramebuffer { id: FbId, cols: u16, rows: u16 },   // width is ALWAYS cols*8 (§5)
    SelectFramebuffer { id: FbId },                       // the implicit "active buffer"
    Clear      { fb: FbId },
    Plot       { fb: FbId, x: u16, y: u16, rgb: Rgb },    // ALREADY wrapped mod (W,H) by the core
    GlyphPixels{ fb: FbId, run: PixelRun },               // core rasterizes the 8×8 font → plots
    SetWrap    { fb: FbId, mode: WrapMode },              // Wrap (torus, default) | Clamp (klem)
}

pub enum AudioCmd {
    // Every `play` emits exactly ONE of these. freq/seq are special cases of one shape.
    Sequence { tempo_bpm: u16, voices: Vec<Voice> },
}
pub struct Voice {
    pitch_hz: Option<f32>,   // None = a rest (stilte). Pitch is PRE-RESOLVED to Hz in the core.
    beats:    u16,           // duration in beats; host turns into seconds via tempo
    osc:      OscId,         // a PRESET name (sinus/blok/zaag/driehoek). Host owns the waveform math.
    env:      EnvId,         // a PRESET name. Host owns the ADSR numbers.
}
```

Design rules baked into this boundary:

1. **The core computes wrapping and movement; the host just blits.** `LANGUAGE.md` §9 says
   positions wrap modulo the dimensions. That math happens *in the core* so every platform
   wraps identically. The host receives already-wrapped integer logical-pixel coordinates and
   its only job is "put this colored pixel here, scaled by N, no smoothing."

2. **Turtles are NOT persistent draw ops.** The pen trail is ink (persistent `Plot`/`GlyphPixels`
   ops accumulated into the framebuffer). The turtle *sprite* is re-rendered every animation
   frame at its current pose. So the core exposes a **read-back sprite snapshot** the host reads
   each frame, separate from the op stream:
   ```rust
   pub struct Sprite { turtle: TurtleId, fb: FbId, x: u16, y: u16, heading_deg: u16, tint: u8, pen_down: bool }
   impl Engine { pub fn sprites(&self) -> &[Sprite] { … } }
   ```
   The `fb` field is load-bearing even in v1 (when there is only one buffer): it is what makes
   a second buffer work later without a rewrite.

3. **The core owns the 8×8 font and rasterizes `print` in-core** (emits `GlyphPixels`), so a
   320-wide iPad and a 128-wide embedded OLED render byte-identical text. The font bytes (~1 KB)
   live in the crate and fit in flash. *(Open option: also expose a high-level `Glyph{ch}` op for
   hosts that want their own typography — best-of-both at the cost of a little more code. Default:
   rasterize in-core.)*

4. **Audio synthesis is host-side; the core emits a fully-resolved declarative description.**
   `osc`/`env` are *preset names*, not numeric synthesis parameters — exactly the colour split
   (`LANGUAGE.md` §10: names are the language's, values are the host's/agency's). Pitch is
   pre-resolved to Hz in the core so the host never parses note names and determinism is preserved.
   The whole tune is emitted as one fire-and-forget event so Step/Pause/Loop semantics stay intact
   (`play` is one line = one step); the host's audio scheduler runs independently and **must cancel
   pending voices on re-run/Loop** or a looping program stacks overlapping audio.

5. **Latency.** `LANGUAGE.md` §9 requires the turtle to move "the instant the line lands." Buffer
   the ops produced by a `step()` and flush them to the renderer per step; do not regress the
   instant-feedback feel when moving from the prototype's imperative draw to the op stream.

---

## 4. The execution / transport contract

The transport (`DESIGN_BRIEF.md` §5, `LANGUAGE.md` §9) is four states over a defined initial
state. The runtime contract that makes it real:

- **`step()` is the only execution primitive.** It executes exactly one statement against an
  explicit environment (`env`) and an explicit stack of loop frames, and returns. There is **no
  synchronous run-to-completion path anywhere in the core.** The host owns the clock — it drives
  `step()` from its own loop/timer for Play and Loop, and calls it once for Step.
  > The prototype violated this: `runAllCurrent` ran a synchronous `while (…) step()` with a hard
  > 50,000-iteration cap that **silently** stopped and reported "done" (a compass violation — silent
  > fuzzy truncation). That hack must not survive the port. A `herhaal 99999` or unbounded `doe`
  > stays responsive because the host yields between steps, never because the core gives up counting.

- **Defined initial state.** Before any run: every framebuffer cleared to black, each buffer's glyph
  cursor at (0,0), the binding environment empty, the PRNG reset to its seed. `reset(seed)` makes
  replay reproducible for debugging; a fresh seed makes `random` draw differently — both intended
  (`LANGUAGE.md` §5).

- **Live per-line execution.** When the child types a line and presses Return, the host calls
  `Engine::run_line(src, line_no)` — equivalent to Step against the live env. The left pane doubles
  as program text and execution history.

- **Errors → Paused-on-line.** A Tier-2 error is returned as `Event::Error{line, …}`; the host drops
  the transport into Paused parked on that line and shows the formatted sentence (§7). Execution
  halts there; it never silently continues and never guesses a fix.

---

## 5. Framebuffers: descriptor + multiple buffers (runtime-only)

Neither of these adds a single child-facing keyword. Both are pure runtime.

**The `RenderTarget` descriptor** decouples logical resolution from the host display, computed once
at program start and frozen for the run (determinism, `LANGUAGE.md` §9):

```rust
pub struct RenderTarget {
    cols: u16,                    // logical width in glyph columns. width_px = cols*8 ALWAYS.
    rows: u16,                    // logical height in glyph rows.
    height_policy: HeightPolicy,  // Fixed(rows) | FromViewport
    wrap_default: WrapMode,       // Wrap (torus) | Clamp (klem)
}
```

- **Fixed invariants** (never configurable): width is always `cols*8` (preserves the 8-px glyph
  grid that makes "40 cols = 320 px" hold); the logical pixel is the unit the turtle lives in
  (`vooruit 100` = 100 logical px on every target); integer-scale, no-smoothing presentation; the
  defined initial state.
- **Configurable** (host-supplied, *not* language-supplied): `cols`, `rows`, the height policy, the
  default wrap mode. The iPad couch default stays 320-wide (`cols: 40`) with `FromViewport` height.
  An embedded 128×64 panel passes `cols: 16, rows: 8` with `Fixed` height.
- **Torus wrap survives any size for free** because it is defined purely in terms of the descriptor's
  dimensions: `((p % dim) + dim) % dim`. On a 128-wide panel a turtle at x=130 reappears at x=2. The
  `wrapmode`/`klem` command (`LANGUAGE.md` §10) flips the policy and is dimension-agnostic.
- **Height-from-viewport vs. fixed panel** is the one genuine difference, captured by `HeightPolicy`.
  Both collapse to "rows is frozen at reset"; the only difference is *who computes it* (a CSS/Auto
  Layout measurement vs. a hardware constant). The language and the turtle code never see the
  difference. `LANGUAGE.md` §9 already blesses the consequence: a program replayed on a
  different-height device wraps differently on the y-axis — determinism holds *within a device/run*.

**Multiple framebuffers** are ambient runtime state, **not a `maak`-able typed thing.** This is the
decisive call and the compass dictates it: the things a child `maak`s are the things he bosses and
watches obey (turtles, numbers, directions, tunes). A framebuffer is the *world those things live in*,
not an actor in it. So:

- The runtime holds `Vec<RenderTarget>` and an **implicit active-buffer index** (default 0).
  `LANGUAGE.md` §9's "the framebuffer" becomes "the active framebuffer," singular from the child's view.
- **A turtle draws into whichever buffer was active when it was summoned** — bind the active index
  onto the turtle at `summon()` time. This is the body-syntonic answer to "which buffer?": "you made
  pietje *here*, so pietje draws *here*."
- Per-buffer state: glyph cursor, font variant, wrap mode all move onto each `RenderTarget`.
- **v1 ships exactly one buffer (id 0) and no language verb to add or switch buffers.** The plumbing
  exists (ops carry `fb: FbId`, turtles carry their buffer); the surface to reach it is absent. The day
  a second buffer is wanted (a HUD layer, a device with two panels), the VM already routes by `FbId`
  and you add a host/debug-only way to allocate/activate — never necessarily a child keyword. This is
  the §11 "frame the door, don't walk through it" pattern, applied to rendering.

---

## 6. "Compile the GUI to embedded": what it actually means

Be honest about this: it is a **keep-the-door-open architecture commitment, not a v1 feature**, and
it is cheap to keep open *only because* of the §3/§5 decisions. The wording "compile the GUI" is the
wrong mental model and should be gently retired:

- The **IDE/GUI** (SwiftUI/egui editor, transport, keyboard) does **not** go on the device. What goes
  on the device is the **core interpreter running a program, emitting `DrawOp`s into a `RenderTarget`
  whose dimensions match the panel, with the device's display driver as the `DrawOp` sink.** Authoring
  stays on the iPad/desktop.
- Two genuinely different embedded modes:
  1. **Device as dumb sink** — the iPad runs the core and streams `DrawOp`s to the panel over BLE/serial.
     Trivial the moment the op stream exists; the panel needs only a `Plot`/blit handler. This is the
     realistic near-term demo *if one is ever wanted*.
  2. **Device runs the core** (`no_std` Rust on the MCU). The real "compile to embedded," gated entirely
     on the core staying `no_std`-clean: font in flash, bounded env, glyph rasterization in-core,
     `alloc`-only.
- **The one constraint to honor now** so the door stays open: keep the `core` crate free of host
  assumptions — no display, no audio device, no clock, no filesystem. Everything it needs from the world
  arrives as the `RenderTarget` descriptor + a PRNG seed + (later) pre-registered sample handles;
  everything it produces leaves as `Event`s + the sprite snapshot.

**Verdict:** embedded is a *non-goal for v1 deliverables* but a *hard constraint on the core's crate
boundary.* Don't build a device demo now. Do write the core `no_std`-clean with the `RenderTarget` +
command-stream seams from day one. That satisfies "from the get-go" at the cost of architectural
discipline rather than feature work.

---

## 7. Errors: structured in the core, formatted in the host

The Tier-2 Dutch error sentences (`LANGUAGE.md` §8) are product copy — verbose, patient, read aloud by
the parent. They must be byte-identical across hosts and they must not couple the `no_std` core to heavy
string formatting or to a single language. Therefore:

- The core emits **structured** errors: `Event::Error { line, kind: ErrorKind, args: ErrorArgs }`, where
  `kind` is an enum (e.g. `AssignToUndeclared`, `PrintNil`, `BareRandom`, `DynamicNameNotFound`,
  `VerbWantsTurtle`, …) and `args` carries the reconstructed intent (the offending name, the *literal
  prefix* and the *evaluated value* of a dynamic name, the RHS expression text, the wanted/given types).
- The Dutch sentence templates live in `vocab.ron` (`ErrorTmpl`), and a formatter — in the host, or in a
  `std`-enabled core feature for tools — renders `kind + args → sentence`. This keeps the core `no_std`
  and keeps localization a future possibility without recompiling the core.
- **The dynamic-name error must be a true byproduct of resolution, not string surgery.** The resolver
  returns the structured decomposition `(literal_segments, evaluated_exprs)`; the `DynamicNameNotFound`
  args carry it directly, so `ik probeerde 'a-7' te vinden, opgebouwd uit 'a-' en de waarde van i, maar
  die bestaat niet` is reconstructed from real data, computed once — fulfilling `LANGUAGE.md` §7.1's
  promise (the prototype faked this with `raw.split("'")` at two duplicated call sites).

The full set of templates is doctested (§9): every template is exercised by a program that triggers it,
and the test pins the exact Dutch output.

---

## 8. The single vocabulary spec drives everything

`vocab.ron` is the one machine-readable source of truth (see the file itself for the schema and the v1
content). The prototype proved `LANGUAGE.md` §2.1's thesis ("the language already computes what type goes
here, so the UI just asks it out loud") but implemented it *four times* — `VERBS`/`COLORS`/`DIRECTIONS` in
the interpreter, `KB_VERBS`/`KB_COLORS` in the keyboard, the capability chips in `computeSuggestion`, and
the inline note table — and the copies have **already drifted** (`zwart`, `achteruit`, `penomlaag` are
missing from one list or another). One spec, many projections:

```
vocab.ron ──serde──> schildpad-core (resolution tables · introspect API · error templates)
    │                       │
    │                       ├──> app-egui      (direct link)
    │                       ├──> app-ios        (UniFFI C-ABI binding)
    │                       └──> schildpad-lsp  (tower-lsp adapter) ──JSON-RPC──> VS Code / Zed / web
    │
    ├── xtask gen-docs    ──> docs/{verbs,types,keywords}/*.md  +  the "wat kan ik maak?" index
    │       every example is a (program, expected) pair ──> cargo test ──> run through the REAL core ──> assert
    │
    └── xtask gen-palette ──> keyboard_palette manifest (verbs grouped, colour keys)
                              display hex joined from the agency-owned palette.json ──> egui & SwiftUI keyboards
```

- **`fits_next(tokens, env)` is one function with three surfaces:** the `LANGUAGE.md` §2.1 "what fits
  next" help-bar feed, the LSP `textDocument/completion` handler, and the IDE autocomplete — *the same
  partial-frame computation as execution, minus the firing.* This is what guarantees the autocomplete can
  never suggest something the interpreter would reject: they are the same code path over the same spec.
- **The pairwise-distinct-slot invariant (`LANGUAGE.md` §2) is validated at spec-load time** and the core
  refuses to start if a verb declares two slots of the same type without the documented positional
  tie-break. The "write it down or it bites you in a year" warning becomes a hard assertion.
- **Hand-authoring is confined to** the prose `doc` strings, the `examples`, and the `ErrorTmpl` Dutch
  templates — all *in the spec*. Signatures, the free-order note, sampler descriptions, completion lists,
  keyboard groups, and the type index are all generated. A test counts statement shapes so §4's "don't let
  the set crawl toward ten" is enforceable.

---

## 9. The LSP, concretely

Both flagship apps consume language intelligence **in-process**, not over a socket:

- **`egui`** links `schildpad-core` and calls `core::introspect::fits_next(...)` on every edit. Zero IPC,
  sub-millisecond, which the live-per-line-execution latency budget (`LANGUAGE.md` §9) demands.
- **SwiftUI/iPad** calls the same functions over the UniFFI binding (`fitsNext(line, env) -> [Completion]`).
  Spawning a language-server subprocess on iPad is hostile/impossible anyway.
- **`schildpad-lsp`** (tower-lsp, JSON-RPC) is a *thin adapter* of the same API for out-of-process editors
  (VS Code/Zed/Neovim) and a future web playground. It contains zero language logic, so it can never be a
  second place behaviour drifts. **Deferred** — built only when an external editor actually needs it.

So the answer to "is the LSP the same crate?" is: the **introspection API** is the real interface, living in
the core; LSP/JSON-RPC is one optional adapter of it. Don't impose a client/server split on an engine that
already lives inside the IDE.

---

## 10. Determinism: the two fixes that are not optional

The compass *is* determinism. Two things in the prototype silently break it once there are three hosts, and
both must be fixed **in the core**:

1. **No floating-point trig.** The prototype's turtle movement uses `Math.cos/Math.sin`. `cos(deg)` differs
   in the last bits between platforms' libms, and over a long `herhaal` those errors accumulate into
   *visibly different drawings* on iPad vs. desktop vs. embedded — a direct violation of "replay reproduces
   exactly." Movement is heading+distance, headings are integers (links/rechts = ±90, plus user
   `draairichting` values), so use **fixed-point integer pixel math with a deterministic sin/cos table**
   (exact cases for the cardinal headings the child overwhelmingly uses). **Consequence to accept up front:
   the canonical drawing will look slightly different from the float-based JS prototype. That is correct,
   not a regression** — the integer drawing is now *the* drawing, on every device.

2. **A core-owned seeded PRNG.** `random` (`LANGUAGE.md` §5) is the only nondeterminism and it must come from
   a small seeded PRNG (xorshift/PCG) owned by the core and reset by `reset(seed)`, **not** from the host's
   `Math.random()`/`arc4random()`. Then "replay with the same seed" is reproducible (debugging) and "replay
   with a new seed" is the intended fresh draw.

Net rule: **everything that affects output lives in the core; the host supplies only the clock, the surface,
the audio device, and the PRNG seed.** If a behaviour can differ between hosts, it is in the wrong place.
This is enforced by **golden command-stream tests** in CI — run a program, hash the emitted `Event` stream,
assert it is identical across at least two target builds. Without those tests, "same input, same output" is
an aspiration, not a guarantee, and the entire product rests on it.

---

## 11. What the prototype taught us (port the feel, rebuild the engine)

`/tmp/maak-demo/schildpad-lang.js` is ~70% of the spec and a genuine proof that the *feel* works. Treat it
as a **golden-behaviour reference**, not an architecture to transcribe.

**Lift verbatim into the new core's tests/spec** — these are the hardest-to-reproduce assets: the verbose
Dutch error sentences and their tone (§8 catalogue made concrete), the vocab tables and sampler definitions,
`fmtNum`, the arithmetic grammar, and the framebuffer/wrap/turtle/8×8-font model (`schildpad-turtle.js` +
`schildpad-font.js` are clean and directly portable).

**Rebuild, do not transcribe** — these are exactly the mechanisms the planned expansion stresses hardest:

1. **Typed-slot resolution.** The prototype's `doVerb` is a kind-dispatch (dir→draairichting, colour→kleur,
   everything-numeric→one bucket) that is correct *only by coincidence* that no current verb has two
   same-typed slots; the §2 pairwise-distinct invariant is unenforced and unenforceable in that shape. Build
   a real `PartialFrame { verb, filled: Map<Type, Value>, remaining: Vec<Type> }` that (a) drives resolution,
   (b) validates pairwise-distinctness at registration, (c) is directly queryable for §2.1's help system, and
   (d) makes the curry-as-function door (below, and `LANGUAGE.md` §15) reachable.
2. **Dynamic name resolution** returns its structured decomposition so the §7.1 error is a real byproduct (§7).
3. **The step/yield contract** owns interruptibility; **no** synchronous truncating run-loop survives (§4).
4. **Every silent token-drop becomes a hard Tier-2 error** — the prototype quietly drops the cast in
   `schuin = 45 draairichting`, discards the value in `maak x getal = 5` for non-value types, and never asserts
   that an expression consumed all its tokens. "Strict, never fuzzy" forbids all three.

---

## 12. Where the new surface features land in the engine

The three child-facing additions decided for this version (`LANGUAGE.md` §13–§16) map onto the engine as
follows. Two of them are why getting the `PartialFrame` model (§11.1) right *now* is high-leverage.

- **Curry-named functions** (`maak roodpen = pen rood` → `roodpen pietje`): a `PartialFrame` becomes a
  first-class **value** that `maak` can bind. Captured slot values are **snapshotted at `maak`-time**, not
  late-bound (late-binding reintroduces hidden state — the compass's cardinal sin). The statement dispatcher
  gains one branch: a leading name bound to a `PartialFrame` enters verb execution starting from that frame.
  Hard boundary, enforced in the parser/resolver: a named action fills the remaining typed slots of **exactly
  one verb** — it is never a multi-statement body.
- **Audio types** (`deuntje`, `toon`, `stilte`): two new reserved types in the spec's `TYPES`, slotting into
  the existing `maak`/postfix-cast grammar with zero new grammar. `play` graduates from a special-cased
  statement into a real verb with one slot accepting `deuntje | toon | getal`; the old `doPlay` path is
  deleted so there is one resolution story. The depth (oscillators, envelopes) lives in runtime preset types
  (`oscillator`, `omhullende`) and the host renderer, never on the surface. Emits one `AudioCmd::Sequence`
  per `play` (§3).
- **Conditionals** (`als <expr> <vergelijk> <expr> [ … ]`, optional `anders`): the surface-minimal form from
  `LANGUAGE.md` §16. Comparison words (`groter`/`kleiner`/`gelijk`) are legal **only inside the `als` header**,
  so **no first-class boolean type is ever created, reserved, or printable** — the truth value never escapes
  the header. In the engine this is one new statement node with a guarded body push, evaluated against the
  existing expression evaluator plus a header-only comparison step. (Note: unlike the other two, a conditional
  is pure *surface* — it cannot be "architected in quietly," which is exactly why it is the one addition that
  spends real surface budget. It is included here per explicit owner decision, built the minimal way so it
  stays trivially upgradeable to a real `klopt` boolean if play-testing later demands one.)

---

## 13. Risk register (read before committing)

- **The Rust→Swift xcframework pipeline is well-trodden but fiddly** (arch matrices, signing, keeping the
  generated Swift in sync). Budget time for a one-button `make ios` + CI that rebuilds it, or the SwiftUI app
  will silently drift from the core.
- **`no_std` discipline is a permanent tax.** No `std::HashMap` (use `alloc::BTreeMap` or `hashbrown`), care
  with `format!`, no incidental `std` deps creeping in via a convenience crate. Mitigate with a CI job that
  builds `core` for a bare-metal target (e.g. `thumbv*-none-eabi`) so an accidental `std` import fails fast.
- **Determinism is the highest-risk, easiest-to-subtly-break area.** Golden command-stream tests across ≥2
  targets are not optional. Fixed-point trig changing the exact pixels vs. the float prototype must be
  accepted as the canonical look.
- **The command/event enum is now the contract every host implements.** Each new variant (multi-framebuffer,
  audio osc/env, sprites) is a breaking change every host must handle. Version it; add variants only when a
  surface feature actually ships, or hosts accumulate dead match arms.
- **Curry-naming's silent ceiling will tempt scope creep.** The instant `maak roodpen = pen rood` works, the
  obvious next wish is `maak vierkant = [ vooruit 100 draai rechts … ]` — a procedure body, which needs params
  and "which turtle?" and is the real-functions feature `LANGUAGE.md` §11 guards. Hold the single-verb boundary
  in the spec or it crawls past ten shapes.
- **Pedagogy depends on UI discipline.** "Functions available but hidden from the child" only holds if the
  keyboard palette and help-bar genuinely omit user-named actions. The language decision and the
  `DESIGN_BRIEF.md` §6 palette decision must stay in sync.
```
