# Schildpad

> A strict, deterministic, Dutch-keyword programming language for a sharp six-year-old,
> living inside a two-up coding playground — a parent and a child on a couch, poking a
> predictable machine and watching it obey.

A turtle named `pietje` appears the moment you summon it, and does exactly what you say:

```
maak pietje schildpad
pen blauw pietje
herhaal 4 [
  vooruit 130 pietje
  draai rechts pietje
]
```

The one design compass, which decides every ambiguous call:

> **This is a deterministic system you can build a theory of by poking it.**
> Same input, same output, always. No hidden state, no fuzzy heuristics, no "do the
> nearest reasonable thing." When in doubt: *more modellable, or less? More is correct.*

## What's here

This repo currently holds the **design and the blueprint**; the implementation is being
built against it (see `ROADMAP.md`).

| Document | What it is |
|---|---|
| [`LANGUAGE.md`](LANGUAGE.md) | Source of truth for the **language** — semantics, types, the pipeline parse, errors, and (§§13–15) the audio / functions / conditionals additions. |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Source of truth for the **machine** — one Rust core emitting a command stream, the framebuffer/audio boundary, determinism, FFI to SwiftUI and embedded. |
| [`DESIGN_BRIEF.md`](DESIGN_BRIEF.md) | Source of truth for the **iPad UI** — the chunky-pixel aesthetic, transport, on-screen keyboard, status bar. |
| [`vocab.ron`](vocab.ron) | The single machine-readable vocabulary spec that drives the interpreter, the docs, the LSP, and the keyboard. |
| [`ROADMAP.md`](ROADMAP.md) | The phased build plan. Phases map to milestones; items map to issues. |
| [`prototype/`](prototype/) | The original interactive JS prototype — a working vertical slice, kept as a **golden-behaviour reference**, not the architecture being shipped. |

## The plan, in one breath

One **Rust core** (`schildpad-core`, `no_std`-friendly) is the sole source of truth for
the language. It owns no screen and no speaker: `step()` executes one statement and returns
a stream of draw/audio/error events. The same core powers a **Rust `egui` app**
(linux/windows/macos), a **SwiftUI iPad app** (via UniFFI), and — eventually — **weird
embedded screens** (via a C ABI). One machine, identical everywhere, because the whole point
is a machine you can trust to be the same every time.

## Status

Early. Design is settled; the core is next (`ROADMAP.md` Phase 0–1). Follow the
[issues](../../issues) and [milestones](../../milestones).

## License

[MIT](LICENSE) © 2026 Jeroen Rombouts
