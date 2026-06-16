# Refactor Prompt — Reviewed & Corrected

This is a corrected version of the "Remove ReplayEngine & Accuracy Improvements" prompt.
Read **"Review Findings"** first — it explains what changed and why. The corrected prompt follows.

---

## Review Findings (read before implementing)

I verified every claim against the current code (`src/engine.rs` 1217L, `src/replay.rs` 490L, `src/ffi.rs`, `src/buffers.rs`). Severity: 🔴 blocker, 🟠 correctness/UX, 🟡 minor.

### 🔴 R1 — Task 3 (1-char lookahead) is the wrong mechanism. Drop it.

- **Breaks the IME streaming contract.** `feed_diff` returns `(backspaces, suffix)`. Holding a key returns `(0, "")`, so the just-typed char is **invisible until the next keystroke**. The hold set is `n, t, c, ch, ng` — the codas that end the **majority** of Vietnamese syllables (`an, tin, công, một, sách…`). Result: visible 1-key input lag on most words. This is a serious UX regression, not an accuracy win.
- **The prompt's own test proves it's broken.** `lookahead_coda_ambiguity_on` asserts `current_composing() == "on"` after feeding `"on"`. But with lookahead, `'n'` is still in `pending_key` (never flushed — only 2 chars typed), so `current_composing()` is `"o"`. The test contradicts the design.
- **It's redundant.** The engine already **re-renders the whole syllable from `raw[]` on every key** (see `engine.rs` `feed` → `partition_syllable` → render; `backspace` replays from `raw`). Any transiently-wrong display (e.g. tone position before the coda arrives) is corrected on the very next key by the diff. V-C-V detection already resolves "coda vs next-onset" (case A) without holding. Cases B/C (`giansf`, `uong`) are handled by full re-render + V-C-V too.
- **Decision: remove Task 3.** If a specific sequence still mis-renders after Tasks 1–2, fix it with a targeted rule + test, not a global hold buffer. (Document the failing sequence first.)

### 🟠 R2 — "Drop optimistic display" — keep the claim honest; lock behavior with a test.

- The justification *"this matches OpenKey's behavior"* is **false**. OpenKey shows coda consonants immediately; it never hides a typed char. (OpenKey's codas are *valid* codas; uvie's "optimistic" only ever fires for *invalid* codas like `b` that signal a coming syllable break.)
- Dropping it is *acceptable in practice* because Vietnamese is space-separated, so the only artifact is a **one-keystroke transient** where a diacritic reverts to raw before the next vowel triggers V-C-V (e.g. `…nê → "neeb"(raw) → "nêbo"`). That transient is corrected immediately.
- BUT optimistic display was removed/fixed across several commits (`dính chữ`, double-cancel). The real fix that made it stable was the **`prev_inner_composed` baseline** (diff against the inner engine's true output, not the optimistic screen string). **Whatever you decide, you MUST preserve the `prev_inner_composed` baseline logic** or the sticky-char bugs return.
- **Decision:** dropping optimistic is allowed, but (a) remove the false OpenKey justification, (b) keep `prev_inner_composed` baseline, (c) add a test that pins the chosen transient behavior so it can't silently regress.

### 🔴 R3 — Dual `feed()` / `feed_diff()` paths = production code is barely tested.

- The 104 tests use `feed()` (no V-C-V, no commit-on-boundary). Production (FFI/Swift) will use `feed_diff()` (V-C-V + auto-commit). So **"all 104 tests pass" does NOT validate the production path.** You'd ship the risky path on ~3 new tests.
- **Decision:** V-C-V / auto-commit / diff logic must be covered directly. Either (preferred) make `feed()` and `feed_diff()` share one internal `process_key` + one V-C-V path and add a `feed_diff_seq` test for every behavior class, or port the existing `replay_tests.rs` cases to drive `feed_diff`. Target: parity with the current `ReplayEngine` test set, then add new cases. Do not delete `replay.rs` (Task 6) until `feed_diff` reaches that parity.

### 🟠 R4 — `feed_diff(&mut self) -> (usize, &str)` needs an owned backing buffer.

- The V-C-V path produces `committed_out + new_composing` — a concatenation that must live somewhere to be returned as `&str`. Returning a borrow of a temporary won't compile.
- **Decision:** add a scratch field `diff_suffix: OutBuffer` (reuse `buffers.rs` type), write the suffix into it, return `&self.diff_suffix`. Keep return type `(usize, &str)`. (Or return owned `(usize, String)` like today — simpler, std-only; but engine is also `no_std`, so prefer the scratch buffer.)

### 🟠 R5 — New string fields are mis-sized and partly redundant.

- `prev_rendered: ArrayString<64>` / `last_valid_out: ArrayString<64>` — **64 is bytes, not chars.** A 24-key syllable of multibyte vowels (`ư`,`ơ` = 3 bytes) can exceed 64 bytes → truncation/panic. Size for worst case: `ArrayString<96>` (24×3 + margin), or use the crate's `OutBuffer` (already `heapless::String<128>` in no_std) for consistency.
- `raw_chars: ArrayVec<char,24>` **duplicates** the engine's existing `raw: [u8;24]` + `raw_len`. Keystrokes are ASCII — `u8` is sufficient. **Reuse `raw[..raw_len]`; do not add `raw_chars`.** This also avoids keeping two raw buffers in sync (a known bug source — see the `double_cancel_fired` raw_buf/​inner desync hack in `replay.rs:156`).
- Use ONE string type. The engine already uses `OutBuffer` (`buffers.rs`: `String` in std, `heapless::String<128>` in no_std) for `out_buf`/`committed`. Prefer `OutBuffer` over `arrayvec::ArrayString` for the new fields so std/no_std behave identically and there's a single string abstraction.

### 🟠 R6 — V-C-V replay must NOT spawn a fresh engine.

- `replay.rs::replay_raw_slice` does `UltraFastViEngine::new()` per split (heap alloc in std; wasteful). The merged engine can replay a `raw[]` slice **using its own existing replay path** (the same one `backspace()` already uses: reset `raw_len`, re-feed bytes). Reuse that; don't allocate a second engine.

### 🟡 R7 — `committed` is unbounded; don't shrink it.

- `ReplayEngine.committed` is `String` (grows with each auto-committed syllable). The engine already has `committed: OutBuffer`. In no_std that's `heapless::String<128>` — pre-existing cap, leave as-is. **Do not** put `committed` in a small `ArrayString`.

### 🔴 R8 — `pending_key` interaction with backspace/commit/reset is undefined.

- (Only relevant if R1 is ignored.) The prompt only handles flush-on-boundary. `backspace_diff`, `commit`, `reset` with a live `pending_key` are undefined → desync. Since R1 removes lookahead, this disappears. Listed so no one re-introduces it half-baked.

### 🟡 R9 — Task 2 (`SylStructure`) is high-risk; gate it.

- `raw_len` is mutated in ~15 places (triple-cancel, modifier-cancel, w-cancel, tone-cancel, backspace replay). Keeping an incremental `SylStructure` in sync across all of them is error-prone, and `partition_syllable()` is the current single source of truth for tone placement + validation.
- **Decision:** do Task 2 **last**, behind a `debug_assert_eq!(self.syl_structure_derived(), partition_syllable())` cross-check so any drift fails tests immediately. Keep `partition_syllable()` until the assert holds across the full suite, then switch reads over.

### 🟡 R10 — Example bugs in the prompt's tests.

- `backspace_diff_correctness`: `"viet"` has **no tone key**, so it renders `"viêt"`, not `"việt"` (that needs `"vietj"`). The `(bs=1, suffix="")` is right but the word/comment is wrong — fix to avoid confusing implementers.
- `vcv_boundary_neebo`: assert the full diff sequence, not just final state (the diff per key is the actual contract). Use the `feed_diff_seq` helper and assert the last `(bs, suffix)`.

### ✅ Sound parts (keep as-is)

- Task 1 merge (diff + V-C-V into the engine), Task 4 FFI rewire, Task 6 deletion order, the "no new FFI ABI / keep `feed`+`backspace` signatures" constraints — all correct.
- `diff_outputs` (LCP, char-safe) is correct and dependency-free — reuse verbatim.
- Implementation order is good, with the fix that Task 6 (delete `replay.rs`) is gated on R3 parity.

---

## Corrected Prompt

> Everything below supersedes the original. Where the original conflicts, this wins.

### Context

Working on **uvie-rs**, a Vietnamese IME engine in Rust. Branch `fix/core-engine-intergration-bugs`.
Two layers today:
- `UltraFastViEngine` (`src/engine.rs`) — incremental stateful core, re-renders the whole syllable from `raw[..raw_len]` on every key. Stack-friendly; uses `OutBuffer` (`buffers.rs`) for strings.
- `ReplayEngine` (`src/replay.rs`) — diff + V-C-V wrapper used by FFI.

**Goal:** fold `ReplayEngine`'s responsibilities into `UltraFastViEngine`, delete `replay.rs`, keep the FFI ABI identical, and improve accuracy via typed syllable slots — **without** introducing a lookahead/hold buffer.

### FFI contract (must stay byte-identical)

All `uvie_replay_*` symbols, arg order, and return types unchanged (`src/ffi.rs`). `uvie_replay_feed`/`_backspace`/`_commit` return `backspace_count` and write `suffix` into `out_buf`. `git diff include/uvie.h` MUST be empty after `cargo build`.

### Task 1 — Merge diff + V-C-V into `UltraFastViEngine`

Add fields (reuse existing `raw[..raw_len]`; do NOT add `raw_chars`):

```rust
pub struct UltraFastViEngine {
    // ... existing fields ...
    /// Composing text currently visible on screen (for diffing). Reuse the
    /// crate string type so std/no_std match and capacity is adequate.
    prev_rendered: OutBuffer,
    /// The inner engine's true last render (diff baseline). MUST be kept —
    /// prevents the "dính chữ" sticky-char bug when typing past a transient state.
    prev_inner_render: OutBuffer,
    /// Raw length / output at the last valid (non-passthrough) Vietnamese render,
    /// used for V-C-V split.
    last_valid_raw_len: usize,
    last_valid_out: OutBuffer,
    /// Scratch buffer that backs the &str returned by feed_diff/backspace_diff.
    diff_suffix: OutBuffer,
}
```

New methods (keep existing `feed`/`backspace` for tests + internal use):

```rust
/// Feed one char, return (backspace_count, suffix_to_type). Suffix borrows self.diff_suffix.
pub fn feed_diff(&mut self, ch: char) -> (usize, &str);
/// Backspace, return (backspace_count, suffix_to_type).
pub fn backspace_diff(&mut self) -> (usize, &str);
```

Diff helper (port `diff_outputs` from `replay.rs:450`, char-safe LCP):

```rust
fn diff_into(prev: &str, new: &str, out: &mut OutBuffer) -> usize {
    let common = prev.chars().zip(new.chars()).take_while(|(a,b)| a==b).count();
    let backspaces = prev.chars().count() - common;
    out.clear();
    for c in new.chars().skip(common) { let _ = out.push(c); }
    backspaces
}
```

V-C-V logic (port from `replay.rs::feed` + `find_split_point`), with two corrections vs the original code:
1. **Reuse the engine's own raw-replay** to render the committed prefix and the new syllable — do NOT spawn a fresh `UltraFastViEngine`. Factor the existing `backspace()` "reset raw_len, re-feed `raw[]`" loop into a private `render_raw_slice(&mut self, &[u8])` and call it for both segments.
2. **Diff baseline = `prev_inner_render`** (not the optimistic screen string) for the non-optimistic step, exactly as the current `replay.rs` `diff_baseline` logic. Keep this.

**Drop optimistic display.** Coda consonants that aren't legal codas render as raw until the next vowel triggers V-C-V. Remove the `is_optimistic`/`is_single_consonant_appended` path. Remove the false "matches OpenKey" comment. Add a test (Task 5) pinning the resulting transient so it can't silently regress.

Word boundary (`is_word_boundary`, port from `replay.rs:362`): on boundary, commit composing → `committed`, return `(0, ch)`.

### Task 2 — FFI rewire

```rust
pub struct UvieReplayEngine { inner: Mutex<UltraFastViEngine> }
```

All `uvie_replay_*` call `feed_diff`/`backspace_diff`/`commit`/`reset`/`committed_text`. Signatures unchanged. Verify `include/uvie.h` diff is empty.

### Task 3 — Tests (parity first, then new)

- All 104 tests in `src/tests.rs` (which use `feed`) keep passing.
- **Port `tests/replay_tests.rs` + the `replay_*` cases to drive `feed_diff`** so the production path has parity coverage with today's `ReplayEngine`. Add helper:

```rust
fn feed_diff_seq(e: &mut UltraFastViEngine, s: &str) -> Vec<(usize, String)> {
    s.chars().map(|c| { let (bs, suf) = e.feed_diff(c); (bs, suf.to_string()) }).collect()
}
```

- New tests: V-C-V (`neebo` → committed `nê`, composing `bo`; assert the per-key diffs), backspace diffs, the dropped-optimistic transient (e.g. assert what `neeb`'s 4th-key diff is), double-tone-cancel, `dd`/`ww` cancels, `gi`/`qu` glide. **Fix the example word bug: use `vietj` for `việt`; `viet` → `viêt`.**

### Task 4 — Delete `replay.rs` (GATED)

Only after Task 3 shows `feed_diff` parity with the old `ReplayEngine` behavior:
1. Delete `src/replay.rs`, remove `pub mod replay;` (`lib.rs`) and the `pub use ...ReplayEngine` re-export, remove `use crate::replay::ReplayEngine;` from `ffi.rs`.
2. `cargo clippy -- -D warnings` clean; `cargo test` green; `no_std` build green (`cargo build --no-default-features --features heapless`).

### Task 5 — Typed Syllable Slots (LAST, gated)

Implement `OnsetKind`/`NucleusKind`/`SylStructure` as in the original, maintained incrementally in `handle_consonant/vowel/modifier/tone`. **Guard with `debug_assert_eq!(self.derived_structure(), self.partition_syllable())`** and keep `partition_syllable()` as the oracle until the assert holds across the entire suite; only then switch reads to the field. `gi`/`qu` glide set at onset-construction time.

### Removed from the original

- **Task 3 (1-char lookahead) is removed entirely** (see R1). No `pending_key`, no `should_hold_for_lookahead`. If a sequence still mis-renders, file it with a repro and fix with a targeted rule + test.

### Constraints / invariants

- No new FFI ABI. `feed`/`backspace` signatures unchanged. All 104 tests pass + new diff tests.
- One string abstraction (`OutBuffer`); sized for worst-case multibyte syllables. No `arrayvec::ArrayString`; reuse `raw[..raw_len]` instead of a second raw buffer.
- `cargo clippy -- -D warnings` clean; std + `no_std/heapless` both build.
- Preserve the `prev_inner_render` diff baseline (anti sticky-char).
- V-C-V replay reuses the engine's own raw-replay; no fresh-engine allocation.

### Recommended order

1. Task 1 (merge; keep `replay.rs` for now) → 2. Task 2 (FFI) → 3. Task 3 (parity + new tests) → 4. Task 4 (delete `replay.rs`) → 5. Task 5 (`SylStructure`, gated).

### Key files

| File | Role |
|---|---|
| `src/engine.rs` | core — primary target |
| `src/replay.rs` | delete (Task 4, gated) |
| `src/ffi.rs` | rewire `UvieReplayEngine` |
| `src/buffers.rs` | `OutBuffer` type (std String / no_std heapless<128>) |
| `src/tables.rs` | syllable tables — read-only here |
| `src/tests.rs` | 104 tests — must pass |
| `src/syllable.rs` | `Syl`/flags — new fields for Task 5 |

---

## Optional Task 6 — Exhaustive syllable set (PHF): oracle now, final-gate later

A proposed improvement: generate the full set of valid Vietnamese syllables (~6.5–7k incl. toned forms) at build time and use a perfect-hash lookup instead of runtime onset/nucleus/coda logic.

**Verdict: adopt as a COMPLEMENT, not a replacement.** A flat syllable PHF cannot replace decomposition because:

1. **Incremental IME needs PREFIX validation, not complete-syllable validation.** Mid-typing `tiến`, the state `tiê` is not a complete syllable but is a valid prefix → must keep composing. A complete-syllable PHF returns "invalid" for `tiê` → wrongly passthroughs mid-word. Prefix validation needs a trie/DAWG, not a flat hash.
2. **A boolean "valid syllable" gives no tone-target.** Tone placement (`oa→1`, `uyê→2`, `qu`/`gi` glide) still needs nucleus decomposition, and tones are applied *incrementally* (tone key pressed before the syllable is complete), so a lookup of the finished form can't drive it.
3. **Multi-syllable / V-C-V** composing (`neebo`) spans two syllables; a single-syllable PHF over the whole composing run fails. Segmentation still required.
4. **"Zero edge cases" is misleading.** The ~7k set must be *generated from* onset×nucleus×coda×tone enumeration + phonotactic constraints — i.e. the same decomposition logic. Wrong rules → wrong PHF. It front-loads edge cases into the generator, not eliminates them. (Upside: the generator can be validated offline against a reference word list.)
5. **Perf is not the motivation.** Runtime match is already ~O(1) on small tables; PHF is O(1) too. Per-keystroke cost is negligible either way. The real value is accuracy/completeness.
6. **Deps/size.** Codegen needs a build-dep (e.g. `phf_codegen` in `build.rs`); runtime stays zero-dep (generated `const` arrays). ~40KB of string data — fine for desktop, note it for embedded/no_std flash budget.

### How to integrate (phased, both gated and low-risk)

**6A — PHF as test oracle (do this; risk-free, high ROI).**
- `build.rs`: enumerate all valid syllables from the decomposition rules (or embed a reference list), emit a `const`/PHF in `OUT_DIR/syllable_phf.rs`.
- Add a differential test: for every syllable in the set, feed its Telex/VNI keystrokes through the engine and assert it round-trips to that exact syllable (correct tone position included). This surfaces accuracy holes in bulk **without touching runtime architecture**.
- No runtime behavior change. Keep `cargo clippy -D warnings` clean; keep build-dep behind `#[cfg(feature)]` if no_std flash is a concern.

**6B — PHF as final-gate (optional, later).**
- On syllable commit / word boundary (when the composing syllable is complete), look it up in the PHF. If the resolved syllable is NOT in the set, fall back to raw passthrough. This kills false-positive resolved outputs (the `tễl` class) with zero phonotactic guesswork.
- **Keep decomposition** for prefix validation (during typing), tone-target placement, and multi-syllable segmentation. PHF is only the completion cross-check.

**Do NOT** delete `is_legal_onset`/`is_legal_coda`/`NUCLEUS_TABLE`. They serve prefix + tone-target + segmentation, which the PHF does not cover.
| `tests/replay_tests.rs` | port to drive `feed_diff` |
