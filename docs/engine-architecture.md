# Engine Architecture Analysis & Redesign

Status: proposal. Goal: 100% accuracy (incl. edge cases), zero deps, perf-minded.

---

## 1. Current architecture (uvie-rs)

**Model: stateless single-pass, resolve-then-validate, full-replay per keystroke.**

```
raw_buffer (bytes)
  └─ render_str():  6 sequential passes
       1. Filter + tone-toggle      (strip s/f/r/x/j/z, cancel rules)
       2. Modifier bubbling          (reorder a/e/o/d)
       3. W bubbling                 (move w next to target)
       4. Resolver                   (ee→ê, aw→ă, ...)
       5. Validation                 (structural check on RESOLVED chars)
       6. Tone placement             (pick vowel, apply mark)
```

ReplayEngine wraps it: replays entire `raw_buf` through a fresh engine every key, then `compact()` shrinks buffer, plus a V-C-V boundary detector for auto-commit.

### Why it breaks

Core flaw: **validation runs on resolved output, not raw keystrokes. Resolver destroys the info validation needs.**

| Input | Raw V-C-V | After bubble+resolve | Validation sees | Result |
|-------|-----------|----------------------|-----------------|--------|
| `telex` | t-e-l-e-x | `tễl` (e's bubble together) | 1 vowel group, valid | `tễl` WRONG |
| `memef` | m-e-m-e-f | `mềm` | 1 vowel group, valid | `mềm` correct |

Both have identical raw shape (V-C-V + tone). After bubbling merges the two `e`s, the V-C-V signal is gone. Validation can't distinguish English from Vietnamese free-style. **Unfixable at validation stage — info already lost upstream.**

### Structural problems

1. **Multi-pass heuristic soup.** 6 passes each guess intent; they interact in surprising ways. Bubbling + triple-cancel + w-bubble each spawned their own bug class (the `neeboo`, `neeeb`, `telex` bugs all live here).
2. **Bubbling is a hack.** "Free-style" reordering (`tieengs`→`tiếng`) forces non-adjacent modifiers together, which is exactly what erases the V-C-V boundary.
3. **Stateless replay + compact + VCV-detector** = 3 layers of workaround to fake the state a stateful engine would just have. O(n) per key, plus `compact()` re-runs the engine n times.
4. **No per-char state.** Can't represent "this `e` carries circumflex + sắc". So tone re-placement on diphthongs is a fragile positional guess (`apply_tone_in_place` is 60 lines of special cases).
5. **phonetics.rs validates the wrong thing.** It checks resolved-char structure (onset/nucleus/coda blacklists). Good for catching impossible *output*, useless for catching valid-looking output from English *input*.

---

## 2. OpenKey architecture (why it works)

**Model: incremental-stateful, validate-then-transform, per-char bitmask buffer.**

### State

```
TypingWord[]: array of Uint32, one per typed char
  bits 0-15  : key/char code
  bit 16     : CAPS
  bit 17     : TONE_MASK   (^ circumflex)
  bit 18     : TONEW_MASK  (w breve/horn)
  bits 19-23 : MARK1..5    (sắc huyền hỏi ngã nặng)
_index       : buffer length
KeyStates[]  : parallel raw-key snapshot for undo
```

Each char owns its full state. Transform = flip bits on the right element, recompute only the affected suffix.

### Flow per keystroke (`vKeyHandleEvent`)

```
key →
  word-break?  → checkSpelling, maybe restore, new session
  consonant?   → insertKey (append literal), checkSpelling
  vowel/tone?  → handleMainKey:
                   pattern-match RAW keys against _vowel / _vowelCombine tables
                   match?   → flip tone/mark bits, emit diff (hBPC backspaces + hNCC chars)
                   no match → insertKey literal  ← English passthrough falls out free
```

### Key wins (architecture, not features)

**A. Validate raw keystrokes against syllable pattern tables — before any transform.**
`checkCorrectVowel` matches the typed sequence backward against legal Vietnamese patterns (`_vowel[KEY_E] = {{E,N,H},{E,N,G},{E,C},{E},...}`). `telex`: at `l`, no `{E,L}` pattern → reject → `l` stays literal. English passthrough is **automatic**, not a blacklist. This is the single most important difference.

**B. Per-char bitmask state.** Tone/mark/caps live on the char they modify. Re-placing a tone = clear bit on old vowel, set on new. No reordering, no re-resolution.

**C. Minimal-diff output built in.** Engine emits `(backspaceCount, newChars)` directly — it knows exactly which suffix changed. No external diffing.

**D. O(1) undo via snapshot.** `KeyStates[]` holds raw keys; restore = blast entire word, re-emit raw. Drives English-correction ("tel ex" → restore).

**E. Deterministic, table-driven syllable rules.** Diphthong tone placement (`handleModernMark`/`handleOldMark`), horn placement (`insertW`) are explicit lookups keyed on the matched pattern, not positional heuristics.

### Net: OpenKey never guesses. It knows the syllable shape from raw keys, so every transform is deterministic and reversible.

---

## 3. Verdict on single-pass direction

**Single-pass stateless-replay is the wrong model for Vietnamese.** Vietnamese input is inherently stateful and incremental:
- a keystroke's meaning depends on accumulated syllable state (is this `s` a tone or a coda?),
- tone position depends on the full nucleus seen so far,
- correctness needs the *raw* sequence, which resolution discards.

Forcing this into a stateless pipeline is why every fix spawns a new edge case. The replay+compact+vcv-detector stack is the symptom.

Keep "single computation per key" as a *perf* goal — fine. Drop "stateless multi-pass resolve-then-validate" as the *model*.

---

## 4. Proposed architecture

**Incremental-stateful, validate-then-transform, per-char attribute buffer. OpenKey's model, Rust-idiomatic, zero deps.**

### 4.1 State

```rust
#[derive(Clone, Copy, Default)]
struct Syl {                 // one entry per typed key kept in the word
    base: u8,                // raw ascii key ('a','e','d','s',...)
    out:  char,              // resolved display char (â, ế, đ, ...)
    tone: u8,                // 0..5 sắc/huyền/hỏi/ngã/nặng
    flags: u8,               // CIRCUMFLEX | HORN | CAPS | LITERAL
}

struct Engine {
    buf: [Syl; 24],          // current word, fixed stack array
    len: usize,
    raw: [u8; 24],           // raw key snapshot for restore/passthrough
    raw_len: usize,
    valid: bool,             // spell state of current word
    mode: Mode,              // telex/vni tables
}
```

No heap. No `String` in the hot path (render into caller buffer / FFI buffer).

### 4.2 Per-key flow

```
feed(key):
  1. classify(key) via mode table  → Vowel | ToneKey | Modifier(w/d) | Consonant | Boundary
  2. dispatch:
       Boundary    → commit(), passthrough
       Consonant   → push literal, revalidate onset/coda
       Vowel       → push, recompute nucleus + tone position
       ToneKey     → try place tone on current nucleus; else literal (toggle/cancel rule)
       Modifier    → try apply circumflex/horn to matching base; else literal
  3. validate_raw():  match raw[..] against syllable pattern tables
       invalid → mark word literal (passthrough), keep raw
  4. emit diff: compare new render vs prev render → (backspaces, suffix)
```

Validation is **incremental** (only re-check the touched segment) and operates on **raw keys**, not resolved chars — the OpenKey fix for `telex`.

### 4.3 Pattern tables (the core of correctness)

Replace `phonetics.rs` blacklists with **positive** syllable tables:

```
ONSET:   set of legal initial consonant clusters (b, ph, ngh, tr, ...)
NUCLEUS: set of legal vowel cores, each tagged with tone-target index
         (a, ê, oa[→1], uyê[→2], ươ[→1], ...)
CODA:    set of legal finals (c, ch, ng, nh, n, m, p, t, y, i)
+ tone-coda constraints (c/ch/p/t → only sắc/nặng)
```

A word is Vietnamese iff `onset · nucleus · coda` all match. Anything else → literal passthrough. This is closed-form: no heuristics, no bubbling.

`tone-target index` per nucleus kills the entire `apply_tone_in_place` special-case pile.

### 4.4 Drop entirely

- modifier bubbling, w-bubbling passes (state makes them unnecessary)
- triple-cancel early-exit hack
- stateless ReplayEngine + `compact()` + raw-VCV boundary detector
- resolved-char structural validation in `phonetics.rs`

### 4.5 Keep

- mode classify tables (`modes.rs`) — already table-driven, good
- tone Unicode mapping (`tone.rs`)
- FFI surface (`ffi.rs`) — but back it with the new stateful engine directly; no replay wrapper

---

## 5. Migration plan

1. Build pattern tables (onset/nucleus/coda + tone-target) from OpenKey's `_vowel`/`_vowelCombine`/`_consonantTable`. **Data, not logic.**
2. New `Syl` buffer + incremental `feed`. Validate-then-transform.
3. Port test suite as-is (it's the spec). Add the failing cases: `telex`, `expect`, free-style `memef`/`tieengs`, diphthong tones, undo.
4. Replace ReplayEngine internals with direct stateful engine; keep its `(backspaces, suffix)` API so Swift/FFI unchanged.
5. Delete dead passes + `phonetics.rs` blacklist.

Perf later: tables are `const`, buffer is stack, per-key work is O(syllable length) ≈ O(1). Already faster than current O(n)-replay-per-key.

---

## 6. TL;DR

- Current engine: stateless, resolve-first, validate-resolved-output → loses raw info → can't separate `telex` from `memef`. Multi-pass heuristics generate endless edge bugs.
- OpenKey: stateful per-char buffer, **validate raw keys against syllable tables first**, transform deterministically, diff output, snapshot undo. Never guesses.
- Recommend: rewrite to stateful validate-then-transform with positive pattern tables. Zero deps, stack-only, O(1)/key. Keep mode/tone tables + FFI API.
