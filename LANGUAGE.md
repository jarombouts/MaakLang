# Maak — Language Design & Philosophy

> The programming language inside the playground. This document is for whoever implements the interpreter/runtime (likely Claude Code). It is the source of truth for semantics. Read `DESIGN_BRIEF.md` for the UI; you don't need it to build the language, but the two must agree on the transport model (§9) and the error surface (§8). Read `ARCHITECTURE.md` for how the runtime is actually built (one Rust core, a command stream, the framebuffer/audio/embedded story); this document stays the source of truth for the *language*, that one for the *machine*.

 Keywords are **Dutch and reserved**. There is one set of keywords; localisation comes later, if ever.

> **This-version additions.** §§1–12 are the original tightly-scoped design and remain the
> backbone. §§13–15 add three things decided after the first interactive prototype: an
> audio datatype model (§13), curry-named functions (§14), and a deliberately minimal
> conditional (§15). The governing rule for all three — and for the runtime growth in
> `ARCHITECTURE.md` — is the **surface/runtime split**: keep what the child *types* exactly
> as tiny as §4 demands; let the engine carry the depth. Where an addition touches §4,
> §10, or §11, those sections are updated in place and point here.

---

## 1. Philosophy — the one compass

There is a single design principle and every decision serves it:

> **This is a deterministic system you can build a theory of by poking it.**

The child is 6.5, sharp (reasons about derivatives from "the sound bounces off the car"), and the actual goal of the whole product is to find out whether *poking a predictable machine and watching it obey* is the thing that lights him up. So the language must be, above all, **modellable**. Same input, same output, always. No hidden state, no fuzzy heuristics, no "do the nearest reasonable thing." A machine that guesses is a machine you cannot form a stable theory of, and forming that theory *is the entire activity we're trying to provoke.*

Everything below is downstream of that compass. When in doubt during implementation, ask: **does this choice make the machine more of a thing the child can model, or less?** More is correct. Always.

Concrete consequences of the compass, each of which is load-bearing:

- **Strict, never fuzzy.** Errors are hard and explicit, never silently papered over. "Do the next reasonable thing" is *banned* — it is undefinable and it destroys modellability.
- **Determinism is required for the transport.** Replay-from-top (`Loop`) and step-through only make sense if the program has a defined initial state and reproduces exactly. So: no ambient nondeterminism. The *only* source of randomness is the explicit `random` token, and even that is bounded and typed (§5).
- **Types make the rules legible.** A small static-ish type system isn't grown-up ceremony; it's what lets the machine give a *precise* account of why it's stuck ("`vooruit` wanted a number, you handed it a turtle"). Types are the substrate the good errors are built on.
- **Command-as-incantation.** You name a thing and it exists (`maak pietje schildpad`). You boss a thing and it obeys, instantly, visibly. The declaration is not ceremony to be minimised — it *is* the magic. Do not add a default turtle; the child must summon one. That act is the toy.
- **Embodied over abstract.** Movement is heading-and-distance (`vooruit 100`, `draai links`), never absolute `x,y`. A six-year-old has a body that goes forward and turns; he does not have a coordinate plane in his head. The turtle is body-syntonic (Papert). This is why the turtle, not the canvas, is the primitive.
- **Abstraction earned, not lectured.** The child reaches `herhaal` because he got sick of typing `vooruit draai vooruit draai`, not because it's lesson three. The language must make the unrolled version *natural to write first* so the loop can be discovered as relief. (This is a usage note, not an implementation one — but don't add anything that forces abstraction early.)
- **Variables in a turtle costume.** A turtle is a named, mutable, stateful thing summoned by name and bossed in place. That *is* the variable concept — the named box with state — delivered lecture-free on line one. `maak score = 0` later is just the boring cousin of something he already owns. The implementation should treat turtles and plain variables as the same kind of thing (a named binding to a typed, mutable value) wherever possible.

---

## 2. Evaluation model — the pipeline parse

Statements are **read strictly left to right, each token consuming what came before.** This matches how the child reads. There is **no inside-out / Lisp-style nesting** at the statement level — that was considered and explicitly rejected because it forces right-to-left evaluation, the one thing a left-to-right reader shouldn't have to hold in his head.

A statement like:

```
draai links pietje
```

evaluates by **partial application held open until the typed slots are full.** A verb declares a set of **typed slots**; tokens fill those slots **by type, not by position**; the statement fires the instant every slot is filled. `draai` declares slots `{draairichting, schildpad}`. Reading `draai`:

1. `draai` is incomplete — it has two empty slots. It waits, partially applied.
2. `links` is a `draairichting`. It can only fill the draairichting slot. One slot left.
3. `pietje` is a `schildpad`. It can only fill the schildpad slot. All slots full → **fire**, mutating pietje in place.

Because the slots are distinctly typed, **token order is free.** All of these resolve to the identical filled frame and execute identically:

```
draai links pietje
pietje draai links
draai pietje links
```

`pietje` can only be the schildpad; `links` can only be the draairichting; there is never a question of which token goes where. The child (and the parent) write whatever reads naturally — pipeline order (`vooruit 100 pietje`), Dutch subject-first order (`pietje vooruit 100`), whatever. The machine resolves by type and doesn't care.

This is **not** "do the nearest reasonable thing." Resolution is **total and unambiguous**: with pairwise-distinct slot types there is *exactly one* legal assignment of tokens to slots, always. That's deterministic and fully modellable — the compass holds. The single rule that keeps it that way:

> **A verb's slots must be pairwise distinct in type.** If a verb ever declares two slots of the *same* type, order *between those two* becomes positional (first-of-type fills first slot-of-type), or it is a hard error — **never** a guess.

Every v1 verb has pairwise-distinct slot types, so order is free *everywhere* right now. You only revisit this rule the day you add a verb taking two `getal`s. Write it down or it bites you in a year.

### 2.1 Slot metadata drives the help system (free affordance discovery)

The slot types aren't just for resolution — they are **introspectable**, and the GUI reads them to offer live, context-aware suggestions. The runtime must expose, at any cursor position, two queries:

- **"what can I `maak`?"** — the list of builtin types (and anything else summonable). Used for the empty-canvas help: clicking help prints `maak iets!` in a bar below the editor; clicking `maak` inserts it; then a recently-used-sorted list of everything you can `maak` (the builtin types) appears, each insertable on tap.
- **"given the tokens already on this line, what fits next?"** — driven by the partial frame. If the child has typed `vooruit`, the verb advertises unfilled slots `{getal, schildpad}`, so the GUI queries the environment for *every binding whose type can fill a remaining slot* — i.e. all turtles in scope — and offers them, plus a hint that a number is wanted. Conversely, typing a turtle name `pietje` first, the GUI can surface *every verb that has a `schildpad` slot* — pietje's capabilities — because the verbs' slot signatures are queryable. "what can this turtle do" and "what does this verb need" are the same introspection from two ends.

This is a direct, free consequence of resolution-by-typed-slots: the language already computes "what type goes here," so the UI just asks it out loud. The implementation should treat the slot signatures of all builtins and the type of every live binding as a queryable surface, not bury them in the parser. (UI rendering of this — the help bar, insert-on-tap, capability lists — is specified in `DESIGN_BRIEF.md`; the *data* behind it lives here.)

Note what this generalised from and what it did *not* break: the old fixed `verb args subject` order is still valid — it's just no longer the *only* valid order. Resolution is still **flat**; no inside-out / Lisp-style nesting was reintroduced. The "no nesting" decision survives intact.

A consequence worth knowing (don't build it in v1): a partially-applied verb like `draai links` is a real value — a "turn-left action" waiting for a turtle. That makes it *nameable*: `maak draailinks = draai links` then `draailinks pietje`. That's a function. **Out of scope for v1** (§11), but the currying model means user-defined verbs are latent and nearly free whenever you want them later. The door is framed; don't walk through it yet.

Arithmetic *expressions* inside a slot evaluate normally — `(a + b) / 2` is ordinary infix math, parens and precedence as usual. Don't confuse the two layers: free-order type resolution is the *statement* level; infix math is the *expression* level inside a slot.

---

## 3. Types

A small type system, mostly invisible to the child, entirely load-bearing for the errors.

Built-in types:

- **`schildpad`** — a turtle. Mutable state: position (in framebuffer pixel space), heading, pen colour, pen up/down. Summoned by `maak <naam> schildpad`. Starts at (0,0), heading **facing right / +x**, pen down, a default colour. (Pen colour and pen up/down are *state on the turtle*, mutated by the `pen` verb — there is no standalone pen object; see §10.)
- **`getal`** — a number. Integers are the common case; decide whether to support reals (suggest: yes, but render cleanly — `12`, not `12.0`). Arithmetic operates on these.
- **`draairichting`** — a *signed angle*. The built-in constants `links` (−90°) and `rechts` (+90°) are values of this type. The child can make his own: `maak schuin draairichting = 45` gives a 45° turn, and `draai schuin pietje` then turns 45°. So `draai` uniformly takes *any* `draairichting`, builtin or user-made — no special-casing links/rechts.
- **nil** — the type of a freshly-`maak`'d binding with no type and no value yet. nil is a real, nameable state, not an error in itself. Printing it *is* an error (§8).

### 3.1 Type names are reserved and act as postfix casts

A type name appearing after a name **casts/sets the type of the thing to its left.** This is how declaration-with-type works without any `::` syntax (the `::` form was considered and discarded as ugly and fiddly for a child's hands). The progression the parser must accept:

```
maak schuin                      # schuin: a nil-typed binding, exists, no value
maak schuin draairichting        # schuin's type mutates to draairichting
maak schuin draairichting = 45   # ...and is assigned 45
```

And, because the cast is just an operator on the thing to its left, all of these are equivalent and **all must be accepted**:

```
maak schuin draairichting = 45
```
```
maak schuin draairichting
draairichting = 45               # NB: only valid if it reads as "assign to the binding being built"; see note
```
```
maak schuin
schuin = 45 draairichting        # cast-on-the-right: take schuin=45, then cast type to draairichting
```

The general rule after `maak <naam>`:

| what follows | meaning |
|---|---|
| (nothing) | nil-typed binding named `<naam>` |
| a **type name** | cast `<naam>` to that type |
| `=` | assignment; evaluate the right-hand expression and bind it |
| type name then `=` | cast, then assign |

Four continuations on one grammatical position, disambiguated by *is this token a known type name, an `=`, or nothing.* This is parseable and deterministic **only because type names are reserved words.** Enforce that hard: `schildpad`, `getal`, `draairichting` (and any future type) **can never be used as variable names.** Without reservation, `maak schuin schuin` becomes ambiguous (cast? or variable named like the type?) and you've broken the compass. Reserve them.

`maak` itself is therefore mildly polymorphic — `maak <naam> <type>` vs `maak <naam> = <expr>` — and that's the accepted cost of keeping the magic word `maak` as the single gateway to bringing things into existence. It's the right trade.

---

## 4. Statement shapes (keep this set tiny)

The whole language should be expressible in a handful of sentence shapes. Shape-consistency is most of what separates "a learnable language" from "arbitrary rules." Do not let this set crawl toward ten.

```
<verb> <slots in any order>     # vooruit 100 pietje ; pietje vooruit 100 ; draai pietje links
maak <naam> [<type>]            # maak pietje schildpad ; maak schildpad pietje  (naam/type: any order)
maak <naam>|<type> = <expr>     # maak score = 0 ; maak Score getal = 0 ; maak getal Score = 0
print <expr>                    # print a + b
<naam> = <expr>                 # score = score + 1   (reassign existing binding; see §8)
herhaal <getal> [ <stmts> ]     # bounded loop
doe <getal> keer [ <stmts> ]    # synonym surface form of herhaal (see §6)
doe [ <stmts> ]                 # unbounded loop (relies on transport Step/Pause to be tractable)
stop                            # break out of the innermost herhaal/doe (see §6)

# this-version additions (see the noted sections):
maak <naam> = <verb> <some slots>            # curry-named function — just the <expr> form with a
                                             #   partial-verb value on the right. see §14
als <expr> <vergelijk> <expr> [ <stmts> ]    # conditional, header-only compare. see §15
anders [ <stmts> ]                           # optional else for the preceding als. see §15
```

`play` is **not** a separate shape: as of §13 it is an ordinary verb, so `play <deuntje>` /
`play <toon>` / `play 440` are all the first shape (`<verb> <slots in any order>`). And the
curry-named function is **not** really a new shape either — it is the existing
`maak <naam> = <expr>` where the `<expr>` happens to be a partially-applied verb (§14). So the
only genuinely new *shapes* this version adds are `als` and `anders` (§15). Hold that line:
§4's warning ("do not let this set crawl toward ten") is still in force.

That's it. Everything the child does is one of these. The first shape — a verb and its typed slots — is **order-free** (§2): the slots resolve by type, so `pietje pen rood`, `pen rood pietje`, `pen pietje rood` are all the same statement. There is **no separate `=` form for verbs** — you don't write `pietje pen = rood`, because `pietje pen rood` already works and adding `=` would be a second syntax for the same act. One shape, any order.

**`maak` is positional only around `=`; the name and type are order-free.** Everything to the *left* of `=` (or the whole tail, if there is no `=`) is the binding target: exactly one new **name** plus an optional **type**, in either order. Because type names are reserved (§3.1), the type token can *only* be the type and the other token can *only* be the name — so `maak pietje schildpad` and `maak schildpad pietje` are identical, as are `maak Score getal = 0` and `maak getal Score = 0`. This is the same type-driven resolution the verbs use (§2), now applied to the name/type pair. What stays fixed is the **value**: it is whatever follows `=`, on the right. `maak 0 = score` is illegal — the left of `=` must be a nameable word, not a value — and a reserved word can never be the name. (The value's position is fixed precisely because a value isn't type-matched against a slot; it's the thing being bound, not a slot-filler.) Arithmetic expressions nest inside the `<expr>` and slot positions with normal infix precedence and parentheses.

---

## 5. `random` — bounded, typed, never global

`random` is the **only** source of nondeterminism, and it is tightly leashed so it never violates the compass.

`random` is **not** a global RNG. It is a token that asks **the pending verb for its sampler.** The pending verb defines the domain:

- `vooruit random pietje` — `vooruit` exposes a sampler over distances, uniform `[0 .. 100]`. So this moves pietje a random distance in that range.
- `draai random pietje` — `draai` exposes a sampler over its direction domain, uniform over `{ links, rechts }`. Turns pietje left or right, ±90.

Because the sampler comes from the verb, `random` is **polymorphic by what holds it** and *cannot produce a nonsense value*: `vooruit` can never roll a direction, `draai` can never roll a distance. The type system makes `random` safe for free.

This gives the child a real, holdable idea: **bounded unpredictability** (`draai random` *will* go left or right — you just don't know which) versus total chaos. He can predict the domain without predicting the draw. That distinction is worth a lot and it's only possible because `random` is constrained.

**Bare `random` does not exist.** `maak x = random` must **error** — `random` of *what*? There is no pending verb, so there is no sampler, so there is no domain, so there is no value. Producing one would silently reintroduce the fuzzy global the whole design rejects. So:

```
maak x = random
# ERROR: "random waarvan? ik kan alleen willekeurig kiezen als er een
#         werkwoord voor staat dat weet waaruit het mag kiezen, zoals
#         'vooruit random' of 'draai random'."
```

(Note on determinism vs `random`: the transport's `Loop`/replay reruns the program. A run *with* `random` will differ between replays — that's correct and intended; the child *asked* for a draw. Determinism means "no *hidden* nondeterminism." `random` is explicit, visible, and domain-bounded, so it doesn't break modellability — the child knows exactly where the unpredictability enters.)

---

## 6. Loops, and the two "repeats"

There are two distinct repeat concepts and they must never be conflated in the implementation, the keywords, or the UI:

- **`herhaal <n> [ ... ]`** (also written `doe <n> keer [ ... ]`) — an *in-code* bounded loop. Runs its body `n` times. This is the abstraction the child discovers when he's tired of repeating himself.
- **The transport `Loop` button** — replays the *whole program* from the top, over and over. This is a runtime control, not a language construct. See `DESIGN_BRIEF.md` §5.

`doe [ ... ]` with no count is an **unbounded** loop. It is only tractable because the transport offers **Step** and **Pause** (§9) — the implementation must yield control between iterations frequently enough that Pause/Step actually interrupt a runaway loop. A child *will* write `herhaal 99999` or an unbounded `doe`; the runtime must stay responsive and interruptible. This is a hard requirement, not a nicety.

### 6.1 `stop` — breaking out of a loop *(this-iteration addition)*

`stop` immediately exits the **innermost enclosing `herhaal`/`doe`** and continues after it — Python's `break`. It is the natural companion to `als` (§15): the way you leave a loop early is to test something and `stop`.

```
doe
  vooruit 10 pietje
  als pietje-x groter 300 [ stop ]   # (illustrative; needs a sensor for pietje-x, §11)
```

Semantics: `stop` unwinds the execution-frame stack up to and including the nearest loop frame; outside any loop it is a no-op (or a gentle error — implementer's call, lean no-op). With unbounded `doe`, `stop` is the *only* in-language way out (otherwise only the transport's Pause stops it). One new keyword, one new statement shape — held to exactly that; there is no `ga door`/`continue`, no labelled break.

---

## 7. Runtime name resolution (the gnarly bit — under the hood, never taught)

This is the most complex thing the language can express. **The parent will not teach it.** But it must work, because it composes cleanly out of the rest of the model and the day the child wants ten turtles in a row, the machinery is already there and does exactly what the name says.

The construct is **dynamic, late-bound name resolution** via implicit string concatenation. A name like `a-'i` is:

1. an expression: the literal `"a-"` concatenated with `i` coerced to string — i.e. `"a-" || (i as string)`,
2. followed by **dereferencing the resulting symbol** — looking up the binding whose name is that computed string.

So this is reflection / `getattr(obj, "a-" + str(i))`, smuggled into a children's language under the hood. The full worked example (the most complex program the language will ever need to support — never shown to the child):

```
maak i = 0
doe 10 keer
  maak a-'i schildpad        # makes turtles a-0, a-1, ... a-9
  i = i + 1

maak i = 0                   # fine to re-instantiate
doe
   doe 10 keer
      draai random a-'i      # boss turtle a-<current i>, randomly
      vooruit random a-'i
      i = i + 1
```

### 7.1 The error falls out for free

Because the interpreter holds **both halves** of the computed name at resolution time (the literal `"a-"` and the runtime value of `i`), a failed lookup can reconstruct the child's intent precisely, with no special-cased error machinery:

```
# if a-7 was never made:
# ERROR: "ik probeerde 'a-7' te vinden, opgebouwd uit 'a-' en de waarde van i,
#         maar die bestaat niet."
```

And `vooruit random a-'(i + 1)` will, on the last iteration, compute a name one past the end and error in exactly the same shape. The error *is a byproduct of how resolution works*, not a feature bolted on top. Implement resolution this way and the good errors come free.

---

## 8. Errors — the catalogue and the tone

Two distinct tiers (mirrored by two distinct UI surfaces — see `DESIGN_BRIEF.md` §3 and §8):

**Tier 1 — mechanical, caught while typing (gentle).** Misspelled keyword, unclosed quote, stray/unknown token. These are handled by **live syntax feedback in the editor before anything runs** — a soft visual cue, not a sentence, not a hard stop. The runtime mostly doesn't see these because the editor flags them first.

**Tier 2 — semantic, caught at run time (verbose, complete Dutch sentences).** These fire only on *conceptual* mistakes, which for a thinking child are the genuinely teachable moments — and precisely *because* Tier 1 eats the typos, Tier 2 stays rare and earned. They are written partly for the parent to read aloud. They must: name what the machine tried to do, name where it got stuck (line number), and where possible suggest what the child might have meant. **Tone: patient, pedantic, never scolding, never cute.** No "oops", no sad faces, no "try again!". The machine simply gives an honest account of itself.

Canonical Tier-2 errors (match this register; these are the source of truth for tone):

```
# assignment to a non-existent binding
d = b - a
# "je probeerde '(b - a)' toe te kennen aan 'd' op regel N, maar 'd'
#  bestaat op dat moment nog niet. misschien wil je 'd' eerst maken
#  met 'maak d'?"
```

```
# printing nil
maak leeg
print leeg
# "je probeerde 'leeg' te printen, maar die had op dat moment de waarde
#  nil. ik weet niet hoe ik nil moet printen."
```

```
# bare random
maak x = random
# "random waarvan? ik kan alleen willekeurig kiezen als er een werkwoord
#  voor staat dat weet waaruit het mag kiezen, zoals 'vooruit random'."
```

```
# failed dynamic name resolution
draai random a-'7    # a-7 never made
# "ik probeerde 'a-7' te vinden, opgebouwd uit 'a-' en de waarde van i,
#  maar die bestaat niet."
```

Every error must reference the **line** so the UI can tie it to the highlighted editor line. Errors **halt execution** at that line (the transport drops to Paused on the offending line). They never silently continue and never guess a fix. The fix is *suggested in words* and applied by the human.

---

## 9. Execution / transport semantics (must agree with the UI)

The program runs under a four-state transport (UI in `DESIGN_BRIEF.md` §5). Runtime contract:

- **Defined initial state.** Before any run, framebuffer is cleared to black, the glyph cursor is at (0,0), and the binding environment is empty. Replay reproduces exactly (modulo explicit `random` draws — §5). This determinism is non-negotiable; it's what makes replay and step meaningful.
- **Framebuffer dimensions & wrapping.** The framebuffer is **always 320 logical px wide** unless a command changes it at runtime; its **height = the viewport height** (in logical px), fixed once at program start. Positions **wrap modulo the dimensions by default** (torus topology): x = 330 on a 320-wide canvas resolves to 330 % 320 = 10; the y axis wraps the same way; the `print` glyph cursor wraps identically. Wrapping is the *default runtime behaviour* and is itself mutable from the language via a `wrapmode` command (§10) that sets the policy — `wrap` (default), or an alternative such as `klem`/clamp-at-edge. Note: because height is taken from the device viewport, a program replayed on a different-height device wraps differently on the y axis; this is acceptable — determinism holds *within a device/run*, the height is a fixed runtime constant, not hidden state.
- **Line is the unit.** Statements are line-terminated. **Step** executes exactly one line and advances the current-line marker. **Play** runs lines until end-of-program or a halting condition. **Loop** runs to end then restarts from the top. **Pause** freezes between lines.
- **Live per-line execution.** When the child types a line and presses **Return**, that line executes immediately (equivalent to Step). The turtle must move *the instant the line lands* — minimal latency between intent and effect. The left pane thus doubles as both program text and execution history.
- **Interruptibility.** Loops (`herhaal`, unbounded `doe`) must yield frequently enough that **Pause** and **Step** can interrupt a runaway. A child writing `herhaal 99999` must not be able to wedge the app. Hard requirement.
- **Errors → Paused-on-line.** A Tier-2 error halts and drops the transport into Paused, parked on the offending line, with the error sentence shown (§8).

---

## 10. Built-in vocabulary (v1)

Keywords (all reserved, all Dutch):

- **`maak`** — declare-and-bind. The magic word. Brings a thing into existence.
- **`print`** — print an expression's value to the framebuffer at the glyph cursor (8×8 white glyphs, advances cursor; cursor wraps the 40-col grid).
- **verbs that boss turtles:** `vooruit <getal> <schildpad>`, `draai <draairichting> <schildpad>`, `pen <kleur> <schildpad>` (and consider `penomhoog` / `penomlaag` for pen up/down).
- **directions (`draairichting` constants):** `links` (−90), `rechts` (+90).
- **`random`** — verb-bound sampler (§5).
- **`herhaal <n> [ ... ]`** / **`doe <n> keer [ ... ]`** / **`doe [ ... ]`** — loops (§6).
- **`stop`** — break out of the innermost loop (§6.1). *(this-iteration addition)*
- **type names (reserved):** `schildpad`, `getal`, `draairichting`.
- **named colours** for `pen`: a small Dutch palette — `rood`, `groen`, `blauw`, `geel`, `wit`, `zwart`, plus a few. (Final palette is a design deliverable; the *names* are the language's, the *values* are the agency's.)
- **arithmetic:** `+ - * /`, parentheses, normal precedence, operating on `getal`.
- **string literals:** double quotes, e.g. `print "Hallo, wereld!"`. Single bare words may be allowed unquoted where unambiguous — decide and document.
- **`play`** — now an ordinary **verb** (§13) with one slot accepting a `deuntje`, a `toon`, or a `getal` (raw Hz, e.g. `play 440`). Plays the sound and wakes the output oscilloscope (`DESIGN_BRIEF.md` §7). The note syntax, the `deuntje`/`toon` types, and `play random` are specified in §13. *(The original `play a b c d` letter syntax is superseded by solfège note literals — see §13 and the open decision in `ROADMAP.md`.)*

This-version additions to the vocabulary (full semantics in the noted sections):

- **audio types (reserved):** `toon` (a tone = pitch + duration), `deuntje` (a tune = a sequence of tones and `stilte` rests). `stilte` is a builtin **value** of type `toon`. See §13.
- **audio presets (NOT reserved, value-like — names here, sound host-side):** oscillators `sinus`/`blok`/`zaag`/`driehoek`, a tiny set of named envelopes. See §13.
- **comparison words (reserved, header-only):** `groter`, `kleiner`, `gelijk` — legal **only** inside an `als` header (§15). They never produce a free-floating value; there is deliberately **no boolean type**.
- **`wrapmode`** — sets the edge policy at runtime: `wrap` (torus, the default per §9) or `klem` (clamp at edge). Present in §9/§10 but unreachable in the prototype; it is a real command.

---

## 11. Scope — what's in, what's earned, what's banned

The original v1 was deliberately tiny. This version grows it on purpose, but only along the
surface/runtime split (`ARCHITECTURE.md` §0). The honest ledger:

**Moved IN this version (each kept minimal, see the noted section):**

- **Curry-named functions** — `maak roodpen = pen rood` then `roodpen pietje`. This is *not* the
  "user-defined verbs" the original v1 excluded; it is the partial-application door §2 already
  framed, and it adds **zero new statement shapes**. Hard boundary: a named action fills the
  remaining typed slots of **exactly one verb** — never a multi-statement body (that bigger thing
  stays out, below). The original "discover `herhaal` first" reasoning is preserved as a *UI/
  pedagogy* rule: keep named actions off the keyboard palette and help-bar so the child still
  reaches `herhaal` as relief first; the parent introduces named actions on the repetition itch.
  See §14.
- **A minimal conditional** — `als <expr> <vergelijk> <expr> [ … ]` with optional `anders`. The
  original v1 made conditionals contingent on play-tested demand; they are included now by owner
  decision, built the **surface-minimal** way: the comparison words live only in the `als` header,
  so **no boolean type is created, reserved, or printable** — the truth value never escapes. See
  §15. *(The fuller `klopt` boolean stays out until play-testing asks for stored truth values.)*
- **An audio datatype model** — `toon`/`deuntje`/`stilte` plus host-rendered oscillator/envelope
  presets. Two new reserved nouns; all the depth lives in the runtime, not the syntax. See §13.

**Runtime-only growth (no surface change at all; specified in `ARCHITECTURE.md`):**

- Configurable framebuffer sizes (weird/embedded screens) — the child never types a resolution.
- Multiple framebuffers — ambient state, drawables scoped to the active buffer; v1 ships exactly
  one and exposes no keyword to add or switch.
- Embedded targeting — a crate-boundary discipline, not a v1 feature/demo.

**Still explicitly OUT (so nobody gilds the lily):**

- No **multi-statement procedures / parameters** (real functions with bodies). Curry-naming is the
  whole function story this version; a body needs params and "which turtle?" and is a later step.
- No **file/network access from the language** (so: no samples-from-a-file in v1 — deferred; when
  samples land they will be host-curated *named* values, no path ever in the language). The IO log
  in the status bar stays ambient texture only.
- No **absolute-coordinate drawing primitives**. Movement is embodied (heading + distance) only.
  Principled exclusion, not an omission.
- No **"do the nearest reasonable thing" recovery**, anywhere, ever. (The compass.) This version
  *tightens* it: the prototype's silent token-drops become hard errors.

---

## 12. Summary for the implementer

Build a strict, deterministic, left-to-right pipeline interpreter over a tiny set of statement shapes, with a small reserved type system whose entire job is to make the runtime's self-explanations precise. Turtles are stateful named bindings summoned by `maak`. Randomness is verb-bound and domain-typed; there is no global RNG and no bare `random`. Dynamic name resolution (`a-'i`) is implicit-concat-then-deref and gives precise "I built this name and it doesn't exist" errors for free. Two error tiers: gentle inline syntax feedback while typing, verbose patient Dutch sentences at run time on conceptual mistakes only. Everything runs under a step/play/pause/loop transport over a defined initial state, and a child must never be able to wedge it.

The compass, one more time, because it decides every ambiguous call: **more modellable, or less? More is correct.**

(§§13–15 below layer this version's three surface additions on top of that core. They follow
the same compass and the same tiny-surface discipline; the runtime machinery behind them lives
in `ARCHITECTURE.md`, and the machine-readable vocabulary for all of it lives in `vocab.ron`.)

---

## 13. Audio — `toon`, `deuntje`, and `play`

The child gains exactly **two new nouns** and the depth lives in the type system and the host
synthesiser, never in the syntax. This is the framebuffer/runtime split applied to sound.

- **`toon`** — one tone: a pitch plus a duration (in beats). The atom. A bare note literal *is* a
  `toon` value. Default sound is a simple sine.
- **`deuntje`** — a tune: a sequence of `toon`s (and `stilte` rests). This is the one audio thing a
  child names and builds. A `deuntje` is uniformly *a sequence of `toon`* — which is exactly why
  `stilte` is a **value** of type `toon` (a rest), not its own type: just as `links`/`rechts` are
  values of `draairichting` so `draai` never special-cases them, a rest is a `toon` so a `deuntje`
  never special-cases silence.

Both are reserved types and slot into the existing `maak`/postfix-cast grammar (§3.1) with **zero
new grammar**:

```
maak liedje deuntje = do re mi fa        # a deuntje built from four toon literals
maak hoog toon = sol                     # one reusable tone
play liedje                              # play it
play do re mi                            # bare note-words gathered into an inline deuntje
play 440                                 # a getal in play's slot = a raw frequency in Hz
play stilte do stilte do                 # rests between notes
```

A run of bare note literals in a `deuntje`/`play` slot is **gathered** into the sequence (the first
variadic slot in the language — mechanical, but write its errors precisely: "play wil noten of een
deuntje"). Note literals use **solfège** (`do re mi fa sol la si`).

**Duration (implemented).** A note carries an optional duration suffix, attached with no space:

```
do        # 1 beat (the default)
do2       # 2 beats — a trailing whole number = beats
do/2      # half a beat — `/N` = one-N-th of a beat (do/3, do/4, …)
play do2 re mi/2 sol/4   # mix freely
```

So a trailing number means **beats, not octave** (decided + built; #43). The lexer reads `do2` /
`do/4` as a *single* note token (not `do` + `2`), so `do2` can no longer be a variable name — but a
lookalike like `dog` (base isn't a note) stays an ordinary name, and `x/2` (non-note) stays division.
A raw `getal` in a `play` slot (`play 440`) and `stilte` are 1 beat; per-rest duration isn't wired
yet. Tempo is a fixed 120 bpm for now. Solfège (not `a b c`) avoids colliding with the single-letter
variable names §8's own examples use.

**`play` is an ordinary verb** (one slot accepting `deuntje | toon | getal`), so it gets free-order
resolution and slot-introspection for the help-bar for free, and `play random` samples uniformly
over the seven notes (§5, verb-bound). The single logical slot is resolved unambiguously: a named
`deuntje` must appear alone; otherwise gather note/`getal` literals left-to-right into one inline
sequence. Write this rule next to §2's pairwise-distinct note — it is the one place a slot accepts
more than one type.

**Composite tone = opt-in, named, never inline.** The simple surface (95% of use) is just pitch +
duration. Depth is reached only by `maak`-ing a named preset and dropping it into a `play`/`toon`
slot as a *whole-tune override* — exactly how a child meets a custom `draairichting`:

```
maak fluit oscillator = sinus
play liedje fluit                        # same tune, fluit oscillator for every tone
maak zacht omhullende = langzaam-aan
play liedje fluit zacht                  # ...and a soft attack
```

`oscillator` (`sinus`/`blok`/`zaag`/`driehoek`) and `omhullende` (a tiny set of named envelope
presets) are **not** reserved type-words — they behave like `kleur`: the *names* are the language's,
the *sound* (waveform math, ADSR numbers) is the host's, just as colour hex is (§10). There is **no
per-note composite syntax** on the surface — that is where it would get overwhelming, so it is banned
from the surface even though the runtime can represent it.

Runtime contract (full form in `ARCHITECTURE.md` §3): every `play` emits one declarative
`AudioCmd::Sequence { tempo, voices: [{ pitch_hz, beats, osc, env }] }`; pitch is pre-resolved to Hz
in the core (determinism preserved), the host synthesises and wakes the scope. **Samples (sound from
a file) are out of scope this version** (§11).

## 14. Functions — naming a half-finished verb

§2 already observed that a partially-applied verb (`draai links`) is a real value — "a turn-left
action waiting for a turtle" — and that this makes it nameable. This version walks through that
framed door, and only that far:

```
maak roodpen = pen rood            # a 'set-the-pen-red' action, waiting for a schildpad
roodpen pietje                     # fill the remaining schildpad slot → fires
maak stap = vooruit 50             # a 'go-50' action
stap pietje
```

This is just the existing `maak <naam> = <expr>` shape where the right-hand side is a partially
applied verb. A named action fills the **remaining typed slots of exactly one verb** when invoked.
Two semantic commitments, both dictated by the compass:

- **Snapshot at `maak`-time, not late-bound.** `maak hoek = draai schuin` freezes `schuin`'s value at
  definition. Late-binding to the live `schuin` would reintroduce hidden, spooky-action state — the
  cardinal sin of §1. (This has a teachable edge: if the child later changes `schuin`, `hoek` does not
  follow. Design an explanation for that surprise; do not "fix" it by going late-bound.)
- **One verb, never a body.** `maak roodpen = pen rood` is allowed; `maak vierkant = herhaal 4 [ … ]`
  is **not** — a multi-statement procedure needs parameters and a notion of "which turtle the body
  applies to," which is real functions with full surface cost (§11, still out). Hold this boundary in
  the parser or it crawls toward ten shapes.
- **`random` in a curry is a hard error, not a frozen draw.** `maak hoe_ver = vooruit random` is
  **rejected**. Because capture is snapshot-not-late-bound (above), currying `random` would freeze a
  *single* draw — every `hoe_ver pietje` would then move the *same* distance — which is almost never
  what was meant, and "make a reusable random-distance action" only makes sense under the late-binding
  the compass forbids. So rather than silently freeze a value, the machine errors and points at the
  inline form: write `vooruit random pietje` directly in your loop to roll each time. (Strict, never
  fuzzy. Decided in the feedback round; see `ROADMAP.md`.)

Pedagogy (a `DESIGN_BRIEF.md` §6 concern, noted here so the two stay in sync): named actions are
**kept out of the on-screen keyboard palette and the help-bar**, so the child still discovers
`herhaal` as relief first. The parent introduces a named action when the child's "I keep typing the
same boss-command" itch appears. The capability is mechanically open; it is *visually* withheld.

## 15. Conditionals — `als`, kept as small as a branch can be

A branch needs a truth value, and this language has no booleans and nothing to compare — adding a
free-floating boolean would drag in a comparison operator, a fifth reserved type, and an `=`-overload
ambiguity. So the conditional is built to **never create a boolean at all**: the comparison lives
*only inside the `als` header* and its truth value never escapes.

```
als score groter 10 [
  pen rood pietje
]
anders [
  pen blauw pietje
]
```

- One new shape: `als <expr> <vergelijk> <expr> [ <stmts> ]`, with `<vergelijk>` ∈
  `{ groter, kleiner, gelijk }`. Optional `anders [ <stmts> ]` is the only extension, added only if
  the child reaches for "the other case."
- The comparison words are legal **only** in the `als` header. There is **no `klopt`/boolean type**,
  nothing to `print`, no `maak x = (a > b)`. This collapses the whole "branching" subtree
  (operator + boolean type + reserved word + shapes) down to one shape and three header-only words.
- This keeps §2's pairwise-distinct-slot rule untouched (no comparison *verb* with two `getal` slots)
  and the §3 `=`-as-assignment meaning unambiguous (no `=`-as-equality anywhere).

Honest note for the implementer and the parent: a conditional is the first construct where the same
program text behaves differently depending on state, and there is little to branch *on* yet (no
sensors, no input). It is included by deliberate decision; if it sits unused on the couch, that is a
signal, not a defect. The natural next step that makes `als` *embodied* rather than abstract is a
**sensor** (edge-collision, pointer coordinates from the status bar) — see `ROADMAP.md` Phase 6. If
play-testing later shows the child wants to *store* a truth value, the header-only design upgrades
cleanly to a real `klopt` boolean; until then, it deliberately does not exist.
