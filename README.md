# Maak

A turtle named `pietje` appears the moment you summon it, and does exactly what you say:

```
maak pietje schildpad
pen blauw pietje
herhaal 4 [
  vooruit 130 pietje
  draai rechts pietje
]
```

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
| [`prototype/`](prototype/) | The original interactive JS prototype |

## The plan

One **Rust core** (`schildpad-core`, `no_std`-friendly) is the sole source of truth for
the language. It owns no screen and no speaker: `step()` executes one statement and returns
a stream of draw/audio/error events. The same core powers a **Rust `egui` app**
(linux/windows/macos), a **SwiftUI iPad app** (via UniFFI), and — eventually — **weird
embedded screens** (via a C ABI). One machine, identical everywhere, because the whole point
is a machine you can trust to be the same every time.

## Building the iPad app

The app links `SchildpadFFI.xcframework`, which is a **build artifact** (not committed). Build
it from the Rust core first, then open the Xcode project:

```
bash MaakSwift/build-xcframework.sh        # builds device + simulator slices
open MaakSwift/Maak/Maak.xcodeproj
```

Re-run the script after any change to `core/` or `ffi-c/`. (Needs the Rust iOS targets:
`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.)

## License

[MIT](LICENSE) © 2026 Jeroen Rombouts / Strange Loop Software / whatever the legal relationship is between 99% clanker-generated code and software licensing.
