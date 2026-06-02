# Schildpad — Design Brief

> A two-up coding playground for a sharp 6.5-year-old and a parent sitting next to him.
> Platform: **SwiftUI, iPad, landscape only.** Attached hardware keyboard **or** on-screen keyboard.

This document is for the design agency. It describes **what to design and how it should feel** — layout, interaction, visual language, component states. It deliberately does *not* specify implementation. The language semantics and runtime live in `LANGUAGE.md`; you do not need to read that to do your job, but the one-paragraph summary below tells you what the thing actually does.

---

## 0. What this app is (read this first)

The child types short Dutch commands into an editor on the left. The result appears immediately in a viewport on the right: a chunky, low-resolution pixel canvas where a named turtle (a little character) moves and draws. Example, the entire first 60 seconds of use:

```
maak pietje schildpad
vooruit 100 pietje
draai links pietje
vooruit 100 pietje
```

`maak pietje schildpad` makes a turtle named pietje appear at the top-left. Each subsequent line moves or turns it, drawing a line as it goes. The magic is **command-as-incantation**: you name a thing and it exists, you boss it and it obeys, instantly, visibly.

**The real goal of this product is not teaching. It is observation.** The parent is testing whether this specific child leans in. So the app is a *shared instrument for two people on a couch*, not a self-serve tutor. Design for two heads looking at one screen, one of them six years old, the other reading error messages aloud and riffing on them. No gamification, no mascot nagging, no "great job!" confetti. The reward is that the machine does exactly what you said. That is the entire dopamine loop. Do not add another one.

---

## 1. Hard constraints

- **SwiftUI**, iPad, **landscape only**. No portrait, no iPhone, no phone-sized reflow.
- Must work with a **physical keyboard attached** and with the **on-screen keyboard**. These are two genuinely different ergonomic situations (see §6). The on-screen case is the hard one.
- A six-year-old's hands. Large hit targets, forgiving spacing, no precision gestures, no long-press-to-discover. If a feature needs a tooltip a child can't read, it needs a different design.
- Dutch is the language of the UI and the programming language both. (Localizable later; design as if Dutch is permanent.)
- The aesthetic is **chunky, deliberate, low-resolution-on-purpose**. Think a clean modern reinterpretation of an early-90s home computer (QBASIC / MCGA SCREEN 13), not skeuomorphic nostalgia and not flat corporate SaaS. The pixels are big *because the child should be able to see and point at individual pixels.*

---

## 2. Overall layout

A **split pane, landscape**:

```
┌───────────────────────────┬──────────────────────────────────┐
│                           │                                   │
│        CODE PANE          │           VIEWPORT                │
│      (editor, left)       │                                   │
│                           │   ┌───────────────────────────┐   │
│                           │   │       FRAMEBUFFER         │   │
│                           │   │     (the pixel canvas)    │   │
│                           │   │                           │   │
│                           │   └───────────────────────────┘   │
│                           │   ┌───────────────────────────┐   │
│                           │   │   STATUS BAR (ambient)    │   │
│   ┌───────────────────┐   │   └───────────────────────────┘   │
│   │ TRANSPORT CONTROLS│   │                                   │
│   └───────────────────┘   │                                   │
└───────────────────────────┴──────────────────────────────────┘
```

- Roughly **50/50 split**. The framebuffer wants to be a clean power-of-two-scaled rectangle (see §4), so the exact split should be driven by making the framebuffer land on a crisp integer scale factor, not by a fixed percentage. Design for that flexibility.
- The split divider is **not** user-draggable in v1. One less thing for a kid to break. Fixed, calm.
- The **transport controls** (step / play / pause / loop) anchor the code pane. They are the second-most-important interactive element after the editor itself. See §5.

---

## 3. The code pane (editor)

The editor is the heart of the interaction. The child *types*. This is a deliberate choice by the parent — typing is a skill he wants the child to build, not a wall to route around. So: **no block-dragging, no Scratch-style puzzle pieces.** It is a real text editor with real text.

What makes it child-usable instead of an intimidating IDE:

- **Big, friendly monospace.** Generous line height. A child reading effortfully needs air between lines.
- **Line numbers**, subtle. They exist because error messages refer to them ("op regel 4…") and because the transport highlights the current line. They are not decoration.
- **Live syntax highlighting that is also gentle error feedback.** As the child types, tokens are coloured by kind: verbs, names, types, numbers, strings, the magic word `maak`. Mechanical mistakes — a misspelled keyword, an unclosed quote, a stray character — show as a soft, non-alarming underline or desaturation *the instant they appear*, before anything runs. This is not a red-squiggle scolding. It is the editor quietly saying "this word isn't one I know yet." The visual weight of this must be low. A six-year-old hits one harsh red wall and deflates.
- **The machine never rewrites what the child typed.** No auto-capitalisation, no reformatting, no case-correction. The editor *colours* tokens but never *changes* them. (Considered and rejected: auto-Title-casing user names so they stand out. It would make the machine silently alter your text, which breaks the one promise — that it does exactly what you said. Colour carries the keyword-vs-name distinction instead, non-destructively.) **The colour-by-kind scheme is shared everywhere the language appears** — a verb is the same colour in the editor *and* on its key in the on-screen palette (§6), so the child learns one colour-vocabulary, not two.
- **The current line, during playback, is highlighted** — a calm full-width band — so both people can see "the machine is reading *this* line right now." This is the single most important affordance for joint attention on the couch.
- **Generous autocomplete / suggestion**, but presented as low-pressure options, not aggressive pop-ins that fight the child's typing. When he's typed `voo`, gently offer `vooruit`. Tab or tap to accept. Never auto-replace what he typed without an explicit accept.

Note for the agency: the *strict, verbose semantic error messages* (the "I tried to find pietje but he doesn't exist" kind) are a feature, not a bug, and they are read aloud by the parent. They need a home in the UI — see §7. They are different from the gentle inline syntax feedback above. One is a whisper while typing; the other is a clear, complete sentence shown when a line actually fails to run.

---

## 4. The framebuffer (the pixel canvas)

This is the payoff surface. Everything the child makes appears here.

- **Logical resolution: always 320 pixels wide, height adapts to the viewport.** The framebuffer is fixed at 320 logical px wide (unless a command changes it at runtime) and is **as tall as the available viewport** — so the canvas's logical height = viewport height ÷ scale factor, computed once at start. This means the visible canvas adapts to the device, and the char grid is 40 columns wide × (height ÷ 8) rows tall. Black background `#000000` by default.
- **Positions wrap (torus topology) by default.** A coordinate past an edge wraps modulo the dimension — x = 330 on a 320-wide canvas is 330 % 320 = 10; same on the y axis. A turtle walking off the right edge reappears on the left; the `print` cursor wraps the same way. This is the default runtime behaviour and is itself mutable from the language (a `wrapmode`-style command — see `LANGUAGE.md`). Design the wrap so it reads as continuous, not as a glitch.
- **Scaled up by an integer factor (target 4×) with no smoothing.** The pixels must be *visibly square and chunky*. A child should be able to put a fingertip on one pixel. This crunchiness is the whole aesthetic — it is charm and it is forgiveness (a one-pixel mistake doesn't look like a mistake at this scale). Do not anti-alias. Do not soften. Hard edges everywhere.
- **Text is drawn as 8×8 pixel glyphs**, white `#ffffff` by default, starting at a cursor at the top-left (0,0). At 320px wide / 8px glyph that's a 40-column grid; at 240 tall, 30 rows. The font should be a clean, legible 8×8 bitmap face — readable but unmistakably *pixels*, not a smooth system font shrunk down. Design or specify this bitmap font; it is a real deliverable and it sets the tone of the whole product.
- **The turtle(s)** are small sprites on this canvas — a recognisable little character (the parent/child will pick the species; design at least one default, a turtle/schildpad, plus design the system so multiple distinct turtles can coexist and be told apart at a glance — different colours/tints is fine). When idle, a turtle just sits there facing a direction. Its **heading must be visible** — the child needs to see which way "vooruit" will go. A little nose / arrow / facing indicator.
- **The drawn line follows the turtle**, in the current pen colour. Pen colours are **named Dutch colours** (`rood`, `blauw`, `groen`…), not hex. Design a small, punchy, legible palette of named colours that read well as single chunky pixels on black.

---

## 5. Transport controls

The program runs under a tape-deck metaphor. Four states, one control cluster, anchored in the code pane:

- **▶ Play** — run from the top until the program reaches its end / halting condition.
- **⟳ Loop** — run from the top, and when it ends, start again from the top. (This is the *whole-program* repeat. It is conceptually distinct from the in-code `herhaal` loop — see naming note below.)
- **⏸ Pause** — freeze. The framebuffer holds its current state; the current line stays highlighted.
- **⏭ Step** — execute exactly one line, advance the highlight to the next line, stop. This is the debugger-for-six-year-olds and it is also what saves the UI when the child inevitably writes a loop that runs ten thousand times. Step is not a power-user feature here; it is front and centre.

Additional interaction: **pressing Return / Enter at the end of a line executes that line and advances** (statements are line-terminated; Enter is the natural "do it" gesture). So there are two ways to drive execution — the transport buttons, and just typing-and-Enter — and they must feel consistent. Typing a line and hitting Enter is "step one line." The buttons are for replaying and for running the whole thing hands-free.

Design these as **large, obvious, physical-feeling buttons** with unmistakable iconography. A six-year-old should know what each does without reading. The current state (playing / paused / looping / stepping) must be visually unambiguous at a glance from across a couch.

Naming caution for copywriting: the **loop transport button** and the in-language **`herhaal`** keyword are two different "repeats." Keep their language and iconography clearly distinct so neither the child nor the parent conflates them.

---

## 6. Keyboard: attached vs on-screen

Two cases, both required:

**Attached hardware keyboard.** Easy case. The editor behaves like a normal text field. Design the focus states, the caret, and make sure transport controls remain reachable without leaving the keyboard (sensible key shortcuts for play/step/pause are welcome — the parent will use them).

**On-screen keyboard.** The hard case, and the one that decides whether a six-year-old can actually use this solo when there's no hardware keyboard around. The standard iOS keyboard is a poor fit: a child can't reliably find Shift, the quote and symbol characters are buried, and the NL layout hides things. 

Design a **custom input accessory bar** that sits above (or replaces, your call) the system keyboard and surfaces the things the language actually needs, as big tappable keys:

- the core **verbs and keywords** the child uses constantly: `maak`, `vooruit`, `draai`, `links`, `rechts`, `pen`, `print`, `herhaal`, `random`, the colour names, the type names (`schildpad`, `getal`, `draairichting`).
- **Coloured by kind, matching the editor (§3).** A verb key is the same colour as that verb in the code; a colour name (`rood`, `blauw`) carries its own hue. One colour-vocabulary across palette and editor.
- **Recently-used first, capped, with a "see all".** Show roughly the ten most-relevant keys (recently-used + common starters) in the bar; a `•••` key opens a calm modal with the *full* catalogue grouped by kind (verbs, words, types, colours, sounds). Keeps the bar uncluttered while making every word reachable as the vocabulary grows past what fits in a row.
- **Symbols are mostly the system keyboard's job.** On iPad the on-screen keyboard already has digits and `( ) + - * / =`, so the palette does **not** duplicate a number/symbol row — it surfaces only genuinely-buried symbols if play-testing shows a child can't find them. The palette's value is the *language words*, not re-implementing a number pad.
- These are a scaffold, **not a replacement for typing.** The child can always type the letters; the palette removes the friction of *hunting*. Training wheels that don't stop you pedalling.

Make this palette feel like part of the toy, not like an enterprise toolbar. Chunky keys, satisfying press states, organised so the most-used verbs are biggest and closest.

---

## 7. The status bar (ambient texture)

A thin strip beneath the framebuffer. **It teaches the child nothing and it is not pedagogy — it is the heartbeat of the machine, proof the thing is alive.** Design it as quiet, ambient, *non-obstructing* texture that a child might find quietly delightful even without understanding it, the way a kid likes a dashboard with blinky lights. It must never pull focus from the framebuffer.

Contents, in rough priority order:

- **Mouse / touch coordinates in framebuffer space, and button/touch state.** This is the *one* part of the status bar that's genuinely useful — "where is the pointer, in the same pixel coordinates the turtle lives in." Connects directly to making things happen. Give it real legibility.
- **Two small oscilloscopes** — audio output and audio input. These are **default-dim / dormant** and **light up when relevant.** When the child writes `play 440` or `play a b c d`, the output scope comes alive and shows the waveform of the sound he just made. That moment — *I heard a thing and now I can see the thing* — is the only reason these exist, and for this particular child (who reasons about sound bouncing off cars) it may land hard. Until audio happens, they idle quietly.
- **Keyboard input indicator** — a subtle readout of recent keypresses. Ambient.
- **A scrolling log of recent disk / network IO** — ambient, low-contrast, scrolling slowly. Texture. The "this machine is doing things under the hood" feeling. Never alarming, never demanding attention.

The visual treatment of the whole bar should be **subdued, monochrome-ish, low-contrast** against the loud black-and-named-colour framebuffer above it. It is the dashboard at the bottom of the windscreen, not the road.

---

## 8. Where the strict error message lives

When a line *runs* and fails for a conceptual reason (printing a nil value, bossing a turtle that doesn't exist, asking `random` of nothing), the runtime produces a **complete, plain-Dutch sentence** explaining what happened and often suggesting the fix. These are deliberately verbose and they are written partly *for the parent to read aloud.* Examples in `LANGUAGE.md`.

Design a calm, clear home for these — a message area that:

- appears clearly but **without alarm** (no big red modal, no shaking, no error klaxon),
- **references the offending line** and visually ties to the highlighted line in the editor,
- has room for a full sentence or two of Dutch text in a readable size,
- feels like *the machine explaining itself*, not *the machine scolding the child.*

The tone target: a patient, slightly pedantic, entirely non-judgmental machine. It always tells you exactly what it did and exactly where it got stuck. Never "oops!", never a sad face, never "try again!". Just: here is what happened, here is the line, here is possibly what you meant.

---

## 9. States to design (checklist)

- **Cold start / empty editor.** What does the child see before typing a single character? An empty black framebuffer and an empty editor is correct and calm — but consider one ghost-text line of invitation (e.g. a faint `maak pietje schildpad` placeholder) that disappears on first keystroke.
- **First turtle appears.** The moment `maak pietje schildpad` runs. This is *the* moment. Design it. A small, satisfying arrival.
- **Turtle moving / drawing**, live, line-by-line.
- **Multiple turtles** coexisting and visually distinguishable.
- **Playback running** (whole program), **looping**, **paused**, **stepping** — four clearly distinct visual states.
- **Inline syntax feedback** (gentle, while typing).
- **Runtime semantic error** (clear sentence, tied to a line).
- **Audio active** (oscilloscope wakes up).
- **On-screen keyboard up** vs **hardware keyboard** — the layout must stay sane when the on-screen keyboard + accessory palette eats the bottom third of the screen in landscape. This is a real constraint; design for it explicitly.

---

## 10. Deliverables expected from the agency

1. **Visual design system**: the chunky-pixel aesthetic, colour palette (UI chrome *and* the named in-canvas colours), the 8×8 bitmap font specification, iconography for transport + palette.
2. **Full landscape layout** at iPad sizes, both keyboard situations.
3. **The custom on-screen keyboard / input accessory palette** design.
4. **The transport control cluster** — icons, states, animation.
5. **The framebuffer + turtle visual language** — default turtle sprite, heading indicator, multi-turtle differentiation, pen rendering.
6. **The status bar** — all four elements, dormant and active states, the oscilloscope wake-up.
7. **The error / message surface** — calm, line-tied, readable.
8. **All the states in §9**, as flows.

The whole thing should feel like one coherent, slightly retro, deliberately chunky, deeply *calm* object that two people can sit in front of and poke at together. The machine is honest, predictable, and a little bit alive. That's the brief.
