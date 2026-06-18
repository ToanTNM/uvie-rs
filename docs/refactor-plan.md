# uvie-rs Engine Refactor Plan

**Audience:** coding agents implementing the rewrite.
**Prereq reading:** `docs/engine-architecture.md` (the architectural rationale). This document is the *executable* plan: what to build, in what order, with acceptance criteria.

**Tone for implementers:** follow this plan literally. When a decision is ambiguous, prefer (a) matching OpenKey behavior, then (b) matching the existing test in `src/tests.rs`. Do not invent new heuristics.

---

## 0. Context & Motivation

### Why we are doing this

The current engine (`src/engine.rs`) uses a **stateless, single-pass, resolve-then-validate** pipeline replayed in full on every keystroke (`src/replay.rs`). It has a structural flaw documented in `docs/engine-architecture.md §1`:

- **Validation runs on resolved output, not raw keystrokes.** The resolver (`ee→ê`, modifier bubbling, w-bubbling) destroys the raw V-C-V signal that validation needs to reject English words.
- Concretely: `telex` → `tễl` (wrong) while `memef` → `mềm` (correct), and the engine **cannot distinguish them** because both have identical resolved structure.
- The pipeline is 6 interacting heuristic passes. Each pass spawned its own bug class (the `neeboo`, `neeeb`, `telex`, `expect` bugs).
- `ReplayEngine` + `compact()` + V-C-V boundary detector are three layers of workaround faking the state a stateful engine would have natively.

### What "done" means

1. **100% accuracy** on the existing test suite (`src/tests.rs`, `tests/replay_tests.rs`) — these tests are the behavioral spec.
2. New tests pass: `telex`→`telex`, `expect`→`expect`, plus the diphthong-tone and undo cases currently marked `ignored` in `tests/replay_tests.rs`.
3. No regressions in free-style typing (`tieengs`→`tiếng`, `memef`→`mềm`, `huow`→`hươ`).
4. Zero new runtime dependencies. `no_std` + `heapless` build still works.
5. FFI ABI (`include/uvie.h`) unchanged — the macOS app (`UVieKey/`) must keep working without Swift changes.

### Non-goals (do NOT do these now)

- Do not change the FFI function signatures or `include/uvie.h`.
- Do not add a dictionary / word-frequency model.
- Do not optimize for SIMD or micro-perf yet. Correctness first; the new design is already O(syllable-length) per key.
- Do not touch the Swift code in `UVieKey/` except if FFI behavior demands it (it should not).
- Do not remove `vi` dev-dependency benchmark comparison.

---

## 1. Target Architecture (summary)

**Incremental-stateful, validate-then-transform, per-char attribute buffer.** Full rationale in `docs/engine-architecture.md §2,§4`.

Core idea: keep a buffer of typed keys where each entry carries its own state (base key, resolved char, tone, modifier flags). On each keystroke:

1. classify the key,
2. attempt to apply it as a Vietnamese transform (tone/modifier) to the current syllable,
3. **validate the raw key sequence against positive syllable pattern tables**,
4. if invalid → the whole word becomes literal passthrough (this is what fixes `telex`),
5. emit a minimal `(backspaces, suffix)` diff.

The key invariant: **the raw key sequence is always preserved** (`raw[..raw_len]`), so we can always fall back to literal output, and validation always operates on raw keys — never on resolved chars.

---

## 2. Module Layout (target)

Keep the crate structure; replace internals. New/changed files:

| File | Status | Responsibility |
|------|--------|----------------|
| `src/syllable.rs` | NEW | `Syl` struct, per-char state, flag constants |
| `src/tables.rs` | NEW | Positive syllable pattern tables (onset/nucleus/coda + tone-target), ported from OpenKey |
| `src/engine.rs` | REWRITE | Stateful incremental engine; same public API surface as today |
| `src/modes.rs` | KEEP (extend) | Classify tables already good; add modifier-target metadata if needed |
| `src/tone.rs` | KEEP | `map_vowel_with_tone` + `TONE_VOWELS` are correct, reuse as-is |
| `src/phonetics.rs` | DELETE (after migration) | Replaced by `tables.rs` positive validation |
| `src/replay.rs` | REWRITE → thin | Becomes a thin adapter over the stateful engine (diff API only), OR folded into engine |
| `src/buffers.rs` | KEEP | Buffer types still used for output rendering |
| `src/ffi.rs` | KEEP (rewire) | Keep all `extern "C"` fns + signatures; point them at the new engine |
| `src/tests.rs` | KEEP + EXTEND | Existing tests are the spec; add new cases |
| `tests/replay_tests.rs` | KEEP + UN-IGNORE | Un-ignore diphthong/VNI tests once supported |

> **Decision point — `replay.rs`:** The cleanest outcome is that the new `engine.rs` natively produces `(backspaces, suffix)` diffs and tracks committed text, making `ReplayEngine` a 20-line wrapper that just forwards. Keep `ReplayEngine` as the FFI-facing type (`uvie_replay_*` functions depend on it) but strip its logic. See Phase 5.

---

## 3. Detailed Design

### 3.1 `src/syllable.rs`

```rust
// Per-key entry. One Syl per physical key kept in the current word.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Syl {
    pub base: u8,    // raw ascii key as typed, lowercased ('a','e','d','s',...)
    pub out: char,   // current resolved display char ('a','â','ế','đ',...)
    pub tone: u8,    // 0=none-or-level .. but store 0..=5 where 0 means no tone applied
    pub flags: u8,   // bitset, see below
}

// flags
pub const F_CIRCUMFLEX: u8 = 1 << 0; // â ê ô   (telex aa/ee/oo, vni 6)
pub const F_HORN:       u8 = 1 << 1; // ă ơ ư   (telex aw/ow/uw, vni 7/8)
pub const F_CAPS:       u8 = 1 << 2; // uppercase as typed
pub const F_LITERAL:    u8 = 1 << 3; // force literal (no transform) for this entry
pub const F_TONE_SET:   u8 = 1 << 4; // tone field is meaningful (distinguishes tone 0 "level"/"removed")
```

**Notes for implementer:**
- `tone` uses the same numbering as `src/modes.rs::TONE_TELEX` / `TONE_VNI`: `1=sắc(s), 2=huyền(f), 3=hỏi(r), 4=ngã(x), 5=nặng(j), 0=remove(z)`.
- Use `F_TONE_SET` to distinguish "tone explicitly removed (z)" from "never had a tone". Tone `0` + `F_TONE_SET` means level tone (cleared).
- `out` is recomputed from `base + flags + tone` via `tables.rs`/`tone.rs`. Keep a helper `Syl::render(&self) -> char`.

### 3.2 `src/tables.rs` — the heart of correctness

Port OpenKey's pattern data (it is *data*, not logic). Sources in `/Users/thupx/Documents/Workspace/OpenKey/Sources/OpenKey/engine/Vietnamese.cpp`:

- `_vowel` (Vietnamese.cpp:19–97) → legal nucleus + nucleus-with-coda patterns per starting vowel.
- `_vowelCombine` (Vietnamese.cpp:99–168) → legal diphthong/triphthong combos.
- `_consonantTable` (Vietnamese.cpp:170+) → legal onsets.
- `_endConsonantTable` → legal codas.

Express them as **positive Rust tables**. Required public API:

```rust
/// Returns true if `onset` (slice of base chars before the nucleus) is a legal
/// Vietnamese initial cluster. Empty onset is legal.
pub fn is_legal_onset(onset: &[char]) -> bool;

/// Returns Some(tone_target_index) if `nucleus` (1..=3 vowels, already resolved
/// with circumflex/horn) is a legal Vietnamese vowel core, where the index is
/// the position WITHIN the nucleus that receives the tone mark.
/// Returns None if the vowel sequence is not a legal Vietnamese nucleus.
pub fn nucleus_tone_target(nucleus: &[char]) -> Option<usize>;

/// Returns true if `coda` (slice of base chars after the nucleus) is a legal final.
/// Empty coda is legal.
pub fn is_legal_coda(coda: &[char]) -> bool;

/// Tone-coda phonotactic constraint: codas c/ch/p/t only allow sắc(1)/nặng(5).
pub fn tone_allowed_for_coda(coda: &[char], tone: u8) -> bool;
```

**This replaces all of `src/phonetics.rs`.** The difference vs today: these are *positive* tables (a thing is legal iff it appears), not blacklists. English words fail to match → passthrough.

**Implementer guidance on tone-target index** (kills `apply_tone_in_place`'s 60 lines of special cases):
- Each nucleus entry stores which vowel gets the tone. Examples (modern orthography):
  - `a` → 0, `oa` → 1, `oai` → 1, `uy` → 1, `uya` → 1, `uyê` → 2, `ươ` → 1, `iê`/`yê` → 1, `uô` → 1, `ưa`→0, `ua`→0... 
- Build the table from OpenKey's `handleModernMark`/`handleOldMark` (Engine.cpp:629–751). Gate old-vs-modern behind a flag mirroring `enable_modern_orthography` (already on the engine).
- **Verify each entry against `src/tests.rs`** — e.g. `tone_placement_two_vowels_no_coda` (`hoas`→`hoá`), `regression_qu_gi_placement` (`quas`→`quá`, `gias`→`giá`). The `qu`/`gi` prefix special cases (Engine.cpp:470–472, 624–636) must be preserved: after `qu`/`gi`, the `u`/`i` is a glide, not part of the nucleus.

### 3.3 `src/engine.rs` (rewrite)

**Public API — MUST keep these (FFI + tests depend on them):**

```rust
pub struct UltraFastViEngine { /* new internals */ }

impl UltraFastViEngine {
    pub fn new() -> Self;
    pub fn set_input_method(&mut self, method: InputMethod);
    pub fn input_method(&self) -> InputMethod;
    pub fn set_quick_start(&mut self, enabled: bool);
    pub fn quick_start(&self) -> bool;
    pub fn set_quick_telex(&mut self, enabled: bool);
    pub fn quick_telex(&self) -> bool;
    pub fn set_modern_orthography(&mut self, enabled: bool);
    pub fn modern_orthography(&self) -> bool;
    pub fn clear(&mut self);
    pub fn commit(&mut self);
    pub fn backspace(&mut self) -> &str;
    pub fn feed(&mut self, key: char) -> &str;      // returns current composing
    pub fn is_empty(&self) -> bool;
    pub fn is_composing(&self) -> bool;
    pub fn current_composing(&self) -> &str;
    pub fn committed_text(&self) -> &str;
    #[cfg(feature = "std")]
    pub fn current_output(&self) -> String;
}
```

> The existing `src/tests.rs::type_seq` helper calls `feed`, `current_composing`, `commit`, `committed_text`. Keep their semantics identical: `feed` of whitespace commits the composing word into `committed` and appends the whitespace; `current_composing` returns the live word.

**New internal state:**

```rust
struct UltraFastViEngine {
    buf: [Syl; 24],     // current word entries
    len: usize,
    raw: [u8; 24],      // raw keys as typed (for passthrough/validation)
    raw_len: usize,
    word_is_literal: bool, // set once raw sequence fails validation; sticky until word reset

    out_buffer: OutBuffer,    // rendered composing string (reuse buffers.rs)
    committed: OutBuffer,

    input_method: InputMethod,
    mode: &'static Mode,      // reuse modes.rs classify/tone tables
    enable_quick_start: bool,
    enable_quick_telex: bool,
    enable_modern_orthography: bool,
}
```

**`feed(key)` algorithm (the contract):**

```
feed(key):
  if key.is_whitespace():
      render();                      # finalize out_buffer
      committed += out_buffer; committed.push(key)
      reset_word()                   # buf/len/raw/raw_len/word_is_literal cleared
      out_buffer.clear()
      return out_buffer              # empty, matches current behavior

  lower = key.to_ascii_lowercase()
  apply quick_start / quick_telex expansion to the RAW input (same rules as current feed,
      src/engine.rs lines 129–161) — these expand keystrokes BEFORE classification.

  push lower into raw[]; remember caps.

  if word_is_literal:                # already decided this word is non-Vietnamese
      render_literal(); return

  classify(lower):
      Consonant      → push_consonant(lower, caps)
      Vowel          → push_vowel(lower, caps)
      ToneKey        → apply_tone(lower)          # may toggle/cancel; may become literal
      Modifier(w/d)  → apply_modifier(lower)      # circumflex/horn/đ; may become literal

  validate():                        # on RAW keys, segmented onset|nucleus|coda
      if not legal Vietnamese syllable shape:
          word_is_literal = true
      # NOTE: do NOT set literal merely because the word is incomplete/mid-typing.
      # Only set literal when the raw sequence cannot be a PREFIX of any legal syllable.

  render(); return out_buffer
```

**Critical correctness rules (port from OpenKey, verify against tests):**

1. **Validation is prefix-aware.** A mid-typed word like `ng` (typing `nghe`) or `to` (typing `toán`) must NOT be flagged literal. Flag literal only when no legal syllable can have this raw sequence as a prefix. OpenKey does this by attempting pattern match and only rejecting transforms, keeping literal chars (Engine.cpp:1144–1192). Practical rule: a word becomes literal when it contains a **vowel → consonant → vowel** pattern in RAW keys *that is not a legal nucleus/onset split*, OR when a tone/modifier key cannot attach to any legal nucleus. This is precisely what separates `telex` (t-e-l-e-x: nucleus `e`, coda `l`, then vowel `e` again = V-C-V across syllable → literal) from `tieengs` (t-ie-ng-s: single nucleus `iê` + coda `ng` + tone `s`).
   - **Implementer: the V-C-V test must run on RAW base keys, before circumflex resolution merges `ee`.** This is the whole point. Two adjacent same-vowel keys (`ee`,`aa`,`oo`) are a circumflex modifier, NOT two nucleus vowels — collapse them first when segmenting, then test V-C-V.

2. **Tone toggling / cancellation** (port from current `src/engine.rs` Phase 1 + OpenKey `insertMark`):
   - Same tone key twice cancels (`ss`→`s` literal). Test: `no_bubble_across_consonants` (`resset`→`reset`).
   - `z` removes tone. Tests: `z_key_removes_tone`.
   - First char is always literal even if it is a tone key. Test: `tone_only_input_produces_empty` (`s`→`s`).
   - Tone key inside a consonant cluster like `tr`, `pr` stays literal. Current rule at `src/engine.rs:209–217`.

3. **Modifier behavior:**
   - `aa→â, ee→ê, oo→ô, aw→ă, ow→ơ, uw→ư, dd→đ` (telex); VNI digits `6/7/8/9` (`src/modes.rs::resolve_vni`).
   - Triple repeat cancels the modifier and keeps literals: `aaa→aa`, `eee→ee` (test `toggling_triplet`), and **chars after the cancel are preserved**: `neeeb→neeb` (test `triple_cancel_with_trailing_chars`, just fixed — keep this behavior).
   - "Free-style" application: a modifier key applies to the most recent matching base vowel even across consonants, as long as the result is a legal syllable: `tieengs→tiếng`, `moiws→mới`, `memef→mềm`, `huow→hươ`. Tests: `free_style_modifier_bubbling`, `no_bubble_across_consonants`, `special_uow_combo`. **In the stateful model this is NOT bubbling** — it is: when modifier key arrives, find the target vowel entry in `buf[]` and set its flag; recompute. No reordering of the buffer.
   - `w` alone → `ư` (telex), `uow`→`ươ` special case (test `special_uow_combo`).

4. **Tone placement uses `nucleus_tone_target`** from tables, not positional heuristics. After determining the nucleus span in `buf[]`, the tone goes on `nucleus_start + nucleus_tone_target(nucleus)`. This replaces `apply_tone_in_place`.

5. **`qu` and `gi` prefixes:** the `u`/`i` is a glide and excluded from the nucleus for tone placement. Tests: `regression_qu_gi_placement`, `modern_orthography_quy_prefix`.

6. **Tone-coda restriction:** codas `c/ch/p/t` reject hỏi(3)/ngã(4) → fall back to literal/no-tone. Test: `tone_restriction_ch_t_coda`. Use `tables::tone_allowed_for_coda`.

**`backspace()`:** pop last `raw` key, pop/recompute `buf`, re-validate (literal flag may clear), re-render. Must produce the right diff for the FFI layer (see 3.4).

### 3.4 Diff output & `replay.rs`

The macOS app needs `(backspace_count, suffix_to_type)` per keystroke (`uvie_replay_feed`). Today `ReplayEngine` computes this by diffing full outputs (`diff_outputs` in `src/replay.rs:325`).

**Target:** the new engine renders the composing string; the diff is computed by comparing the previous render vs new render (longest common prefix), same `diff_outputs` logic. Keep `diff_outputs` — it is correct and dependency-free.

`ReplayEngine` becomes:

```rust
pub struct ReplayEngine {
    inner: UltraFastViEngine,
    prev_output: String,
}
impl ReplayEngine {
    pub fn feed(&mut self, ch: char) -> (usize, String) {
        let new = self.inner.feed(ch).to_string();   // or handle word-boundary commit
        let (bs, suffix) = diff_outputs(&self.prev_output, &new);
        self.prev_output = new;
        (bs, suffix)
    }
    // backspace/commit/reset analogous; committed_text() forwards to inner
}
```

> **Important behavioral note on auto-commit / V-C-V boundary.** The current `ReplayEngine` auto-commits a valid syllable when a new syllable starts (`neeboo` → commit `nê`, compose `bô`; tests `vcv_*`, `test_vcv_boundary_auto_commit`). The new stateful engine should handle multi-syllable input the same way: when a keystroke starts a new syllable (the running word would become two syllables), commit the completed first syllable to `committed` and start a fresh `buf` for the new one. Implement this inside `UltraFastViEngine` (it has the state to do it cleanly), and `ReplayEngine` just diffs. Preserve the exact outputs asserted in `tests/` and `src/tests.rs::replay_tests`.

### 3.5 `src/ffi.rs`

No signature changes. Rewire the bodies to call the new engine. `UvieEngine` wraps `UltraFastViEngine`; `UvieReplayEngine` wraps `ReplayEngine`. All 28 `extern "C"` functions keep their names/params (see `src/ffi.rs`). `build.rs` (cbindgen) regenerates `include/uvie.h` — diff it and confirm **no changes** (Phase 6 gate).

---

## 4. Phased Task Breakdown

Each phase is independently reviewable and compiles. Do them in order. **Run `cargo test` after every phase** (`rtk cargo test` per project rules). Do not proceed if tests regress beyond the documented expectations.

### Phase 0 — Safety net (no code change)
- Create branch `refactor/stateful-engine`.
- Snapshot current behavior: run `cargo test` and record the full pass/fail list. The two intentionally-documented cases (`test_telex_word_passthrough` asserting the WRONG `tễl`, and 4 `ignored` tests in `tests/replay_tests.rs`) are the targets we will flip.
- **Deliverable:** branch + baseline test log committed to PR description.

### Phase 1 — `syllable.rs` + `tables.rs` (pure data, no engine wiring)
- Implement `Syl` + flags (§3.1).
- Implement `tables.rs` with the 4 public fns (§3.2), ported from OpenKey `Vietnamese.cpp`.
- Write **unit tests for tables only** (new module `#[cfg(test)]`): assert legal onsets/nuclei/codas and tone-target indices for every nucleus appearing in `src/tests.rs` expectations. This pins the data before engine logic depends on it.
- **Acceptance:** `tables` unit tests pass; nothing else changed; crate compiles.
- **Risk:** getting tone-target indices wrong. Mitigation: derive each from a corresponding assertion in `src/tests.rs`.

### Phase 2 — new `UltraFastViEngine` core (telex, single syllable)
- Rewrite `engine.rs` with the new state + `feed`/`render`/`backspace`/`commit`/`clear` for **Telex, single-syllable words only**. Defer auto-commit/V-C-V (Phase 4) and VNI (Phase 3).
- Implement classify dispatch, modifier application (incl. triple-cancel + free-style via target-vowel lookup), tone application via `nucleus_tone_target`, tone toggling/cancel, and the **raw V-C-V literal rule** (§3.3 rule 1).
- Keep `src/phonetics.rs` and old `replay.rs` temporarily so the crate still builds; gate the new path or just replace `engine.rs` and fix `replay.rs` minimally to call new API.
- **Acceptance:** all single-syllable tests in `src/tests.rs` pass, INCLUDING the new targets `telex→telex`, `expect→expect`. Update `test_telex_word_passthrough` to assert `"telex"` (remove the KNOWN-limitation TODO). Multi-syllable/replay tests may still fail here.
- **Risk:** free-style modifier across consonants vs V-C-V literal rule can conflict (`memef→mềm` must work but `telex→telex` must passthrough). Mitigation: the discriminator is whether the resulting raw segmentation is a *legal single syllable*; `memef` = `m + ê + m` (legal), `telex` = `t + e + l + e` (two vowel groups, illegal). Test both explicitly.

### Phase 3 — VNI mode
- Wire VNI resolver (`src/modes.rs::resolve_vni`, digits as tone/modifier keys). Reuse the same engine; only classification/resolution tables differ.
- **Acceptance:** `vni_*` tests pass. Un-ignore `test_vni_quyet`, `test_vni_thuong` in `tests/replay_tests.rs` and make them pass.

### Phase 4 — multi-syllable auto-commit (V-C-V boundary) + diphthong tones
- Implement in-engine syllable boundary: when a key would create a second syllable, commit the first to `committed`, start fresh `buf`.
- **Acceptance:** `tests/replay_tests.rs::replay_tests::vcv_*` and `src/tests.rs::test_vcv_boundary_auto_commit` pass. Un-ignore `test_huyen`, `test_quyet` (diphthong tone placement) and make them pass.

### Phase 5 — slim `ReplayEngine` + diff
- Replace `ReplayEngine` internals with the thin adapter (§3.4). Keep `diff_outputs`. Delete `compact()`, `find_coda_split_slice`, `is_raw_passthrough_slice`, the old boundary detector.
- **Acceptance:** all of `tests/replay_tests.rs` + `src/tests.rs::replay_*` pass. `replay_compact_no_crash` still passes (the new engine bounds the buffer by construction — fixed 24-entry arrays + per-word reset; document why no `compact()` is needed).
- **Risk:** buffer overflow on pathological input (40 `e`s). Mitigation: fixed arrays with bounds; on overflow, force-commit current word and start fresh (safety valve, like current `raw_buf.is_full()` path).

### Phase 6 — FFI rewire + cleanup
- Point `src/ffi.rs` bodies at the new engine (no signature changes).
- Delete `src/phonetics.rs`; remove its `pub mod phonetics;` from `lib.rs`.
- Regenerate header: `rtk cargo build` and `git diff include/uvie.h` → **must be empty**.
- Build the macOS lib target and smoke-test (`cargo build --release`; the dylib/staticlib targets must compile).
- **Acceptance:** full `cargo test` green; `no_std` build green (`rtk cargo build --no-default-features --features heapless`); `include/uvie.h` unchanged.

### Phase 7 — manual QA on the app (human-in-loop)
- Build `UVieKey` against the new lib; manually type the bug-report cases: `neeboo`, `neeeb`, `telex`, `expect`, plus common Vietnamese words. Confirm no Swift changes needed.

---

## 5. Test Strategy

- **`src/tests.rs` and `tests/replay_tests.rs` are the spec.** Do not weaken an assertion to make it pass unless it encodes the *old buggy* behavior. The only assertion that should be *changed* is `test_telex_word_passthrough` (flip `tễl`→`telex`).
- **Un-ignore** the 4 ignored tests in `tests/replay_tests.rs` by end of Phase 4.
- **New tests to add** (Phase 2+):
  - `expect→expect`, `telex→telex`, `email→email`, `select→select`, `lorem→lorem` (English passthrough).
  - `nghieng→nghiêng`, `huyen→huyền`, `quyet→quyết`, `nguoi→người`, `thuong→thương` (diphthong + horn + tone).
  - Edge: `dd` at word start (`dong→dong` vs `ddong→đong`), `qu`/`gi` glide cases.
- **Differential test (optional, recommended):** add an `#[ignore]` integration test that feeds a word list through both the `vi` crate (already a dev-dep) and the new engine, flagging diffs for manual review. Not a hard gate (the `vi` crate has its own quirks) but a useful net.
- Run `rtk cargo test` (per `.windsurfrules`) to minimize token cost when iterating.

---

## 6. Potential Issues & Mitigations

| # | Issue | Likelihood | Mitigation |
|---|-------|-----------|------------|
| 1 | Free-style modifier vs V-C-V literal conflict (`memef` vs `telex`) | High | Discriminate via legal-single-syllable segmentation on RAW keys; collapse `aa/ee/oo` to circumflex BEFORE V-C-V test. Explicit tests both ways. |
| 2 | Tone-target table errors on diphthongs | High | Derive every entry from an existing test assertion; Phase 1 table unit tests pin them before engine depends on them. |
| 3 | Prefix-validation too eager → flags mid-typed valid words literal | Medium | Rule: literal only when no legal syllable can have this raw seq as prefix. Test incremental typing of `nghiêng`, `thương` char-by-char. |
| 4 | Multi-syllable auto-commit diff drift (wrong backspace count) | Medium | Keep `diff_outputs` (LCP) unchanged; add char-by-char screen-state assertions like existing `type_replay` helper. |
| 5 | FFI ABI accidentally changes | Medium | Phase 6 gate: `git diff include/uvie.h` must be empty; do not edit signatures. |
| 6 | `no_std`/`heapless` build breaks (new code uses `String`/`Vec`) | Medium | Engine core uses fixed `[Syl;24]`/`[u8;24]` arrays + `OutBuffer` from `buffers.rs`. `String` only in `ReplayEngine` (std/ffi path), as today. Gate accordingly. |
| 7 | Buffer overflow on pathological repeats | Low | Fixed arrays + safety-valve force-commit on full (mirror current `is_full()` path). |
| 8 | Caps/uppercase preservation regressions | Medium | `Syl::flags & F_CAPS`; reuse current case-reapply logic from `replay.rs:267–275`. Add tests for `Telex`, `Nghiêng`. |
| 9 | Old vs modern orthography divergence | Low | Tone-target table has two variants gated by `enable_modern_orthography`. Tests `modern_orthography_*` pin modern; add old-style if needed. |
| 10 | `vi` dev-dep benchmark (`benches/perf.rs`) breaks | Low | Keep `UltraFastViEngine`/`ReplayEngine` public API identical; benches call public API only. |

---

## 7. Definition of Done (final gate)

- [ ] `cargo test` fully green (lib + `tests/` + doctests), with the 4 previously-ignored tests un-ignored and passing.
- [ ] `test_telex_word_passthrough` asserts `"telex"`; new English-passthrough tests pass.
- [ ] `telex→telex`, `expect→expect`, `neeboo→nêbo…`, `neeeb→neeb` all correct.
- [ ] Free-style (`tieengs→tiếng`, `memef→mềm`, `huow→hươ`) still correct.
- [ ] `git diff include/uvie.h` empty after `cargo build`.
- [ ] `no_std` build: `cargo build --no-default-features --features heapless` green.
- [ ] `src/phonetics.rs` deleted; no `bubbling`/`compact`/V-C-V-detector workaround code remains.
- [ ] `UVieKey` app builds against new lib with zero Swift changes (manual QA, Phase 7).
- [ ] PR description documents the architecture change and links `docs/engine-architecture.md` + this plan.

---

## 8. Appendix — quick reference for implementers

- Tone numbering (telex keys → id): `s=1 sắc, f=2 huyền, r=3 hỏi, x=4 ngã, j=5 nặng, z=0 remove`. Source: `src/modes.rs::TONE_TELEX`.
- Unicode tone mapping: `src/tone.rs::map_vowel_with_tone` + `TONE_VOWELS` (12 vowel rows × 6 tones). Reuse directly.
- Classification tables: `src/modes.rs::CLASSIFY_TELEX/CLASSIFY_VNI` (`IS_VOWEL/IS_MODIFIER/IS_TONE_KEY`). Reuse.
- OpenKey source of truth for patterns: `/Users/thupx/Documents/Workspace/OpenKey/Sources/OpenKey/engine/Vietnamese.cpp` (`_vowel`, `_vowelCombine`, `_consonantTable`, `_endConsonantTable`) and `Engine.cpp` (`checkCorrectVowel` 468–499, `handleModernMark`/`handleOldMark` 629–751, `insertW` 871–984, `insertMark` 753–806, restore 1204–1221).
- Existing diff logic to keep: `src/replay.rs::diff_outputs` (longest-common-prefix → backspaces+suffix).
- Shell commands: prefix with `rtk` (per `.windsurfrules`), e.g. `rtk cargo test`.
