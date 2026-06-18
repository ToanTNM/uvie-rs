# FFI Cleanup + Swift Rewire Plan — Remove ReplayEngine, Fix Feature Gaps

**Audience:** coding agents. **Goal:** eliminate the `ReplayEngine` concept everywhere (Rust module + FFI naming), consolidate to a single clean FFI surface backed by `UltraFastViEngine`'s diff API, update Swift, and **fix the feature gaps found during review**.

---

## STATUS (updated 2026-06-16, after commits `e0cf36b`, `6d3ea26`)

Re-verified against the working tree. Progress so far:

| Item | State | Notes |
|------|-------|-------|
| `src/replay.rs` deleted, merged into `UltraFastViEngine` | ✅ DONE | commit `e0cf36b`; `lib.rs` has no `mod replay` / `ReplayEngine` re-export |
| Diff API on engine (`feed_diff`/`backspace_diff`/`commit_diff`/`reset_diff`/`is_composing_diff`/`committed_text_diff`) | ✅ EXISTS | confirmed in `src/ffi.rs` call sites + `src/engine.rs` |
| Typed syllable slots (`OnsetKind`/`NucleusKind`/`SylStructure`) | ✅ DONE | commit `6d3ea26`; exported from `lib.rs` (separate from this plan, noted for context) |
| **Phase 1 — FFI consolidation** | ❌ NOT STARTED | `src/ffi.rs` STILL has BOTH surfaces: `uvie_engine_*` (non-diff + `feed_utf8` + `pending_utf8`) AND `uvie_replay_*` / `UvieReplayEngine` (diff). Must collapse to one. |
| **Phase 2 — delete replay.rs** | ✅ DONE | covered by `e0cf36b` (replaces this phase) |
| **Phase 3 — Swift rewire** | ❌ NOT STARTED | `EngineBridge.swift` still binds `uvie_replay_*` (38 refs); `TextDiff.swift` still present; engines not shared |
| **Phase 4 — feature gaps F1–F4** | ❌ NOT STARTED | no caller of `setModernOrthography`; no runtime propagation; AX engine still separate/unconfigured |
| **Phase 5 — tests/QA** | ⏳ pending | `cargo build` currently green |

**Cosmetic leftover (optional, for Definition-of-Done §6):** the word `replay` still appears as internal names/comments in `src/engine.rs` (`replay_chars`, "V-C-V replay" comments) and `src/tests.rs` (`mod replay_tests`, `replay_compact_no_crash`, `replay_triple_cancel_preserves_trailing_chars`). These are harmless (no `ReplayEngine` type/module remains) but rename them if you want `rg -i replay` to return zero.

**Remaining work = Phase 1 + Phase 3 + Phase 4 + Phase 5.** Phase 2 is already complete; skip it.

---

## 0. Current State (verified against code)

### Rust
- `UltraFastViEngine` (`src/engine.rs`) already exposes the diff API: `feed_diff`, `backspace_diff`, `commit_diff`, `reset_diff`, `is_composing_diff`, `committed_text_diff`. (Verify these six exist before starting.)
- `src/ffi.rs` has **two** C surfaces:
  - `uvie_engine_*` — non-diff: `feed_utf8` returns the full composing string; multibyte UTF-8 decode via `pending_utf8`. **Not used by Swift.**
  - `uvie_replay_*` — diff API: `feed`/`backspace`/`commit` return `(backspaces, suffix)`. **This is what Swift uses.** Its struct `UvieReplayEngine` already wraps `UltraFastViEngine` directly (no `replay::ReplayEngine`).
- `src/replay.rs` (`ReplayEngine`) is still compiled and exported (`lib.rs`: `pub mod replay;` + `pub use ...ReplayEngine`). FFI no longer uses it; only tests may. **Likely dead for production.**

### Swift (`UVieKey/`)
- Binds Rust symbols via `@_silgen_name` in `EngineBridge.swift` → all `uvie_replay_*`. **Swift binds by symbol name, not via `include/uvie.h`** — any Rust rename requires editing the `@_silgen_name` strings manually.
- `EventTap` owns its own `EngineBridge` and drives `feed`/`backspace`/`commit`.
- `AXTextInjector` (Spotlight path) owns a **second, separate** `EngineBridge`.
- `TextDiff.swift` — **dead code**: no references (the engine returns diffs now). grep confirms only its own definition site.

### `cbindgen`
- `build.rs` regenerates `include/uvie.h` from the `extern "C"` fns. Renames auto-propagate to the header, but **NOT** to Swift (`@_silgen_name` is manual).

---

## 1. Feature Gaps Found (the "check features" ask) — fix these

| # | Gap | Evidence | Impact | Fix |
|---|-----|----------|--------|-----|
| F1 | **`modernOrthography` toggle does nothing** | `SettingsWindow.swift:353/362` binds `@AppStorage(modernOrthography)`; `EngineBridge.setModernOrthography` exists; **no caller anywhere** | User toggles modern/old orthography → engine never told | Wire it: apply on load + on change to the shared engine |
| F2 | **Quick Start / Quick Telex / Modern Orthography not propagated at runtime** | Only `EventTap.loadSettings()` sets them, once at init. Settings UI changes don't reach the engine until app restart | Toggling in Settings has no effect until restart | Add a `reloadSettings()` / observe `UserDefaults` and re-apply to engine(s) |
| F3 | **AXTextInjector engine never configured** | `AXTextInjector` has its own `EngineBridge` + setters, but **no one calls** `setInputMethod/QuickStart/QuickTelex` on it; `EventTap` never forwards | Spotlight always Telex, no quick modes, ignores VNI / runtime changes | Share ONE engine, or forward all config to the AX engine too |
| F4 | **Input method runtime change only partially wired** | `MenuBarController:20` calls `eventTap.engine.setInputMethod`; AX engine + quick flags not updated | Inconsistent state between EventTap and AX engines | Centralize config application (see Task 3) |
| F5 | **`feed` drops non-ASCII** (`CChar(char.asciiValue ?? 0)`) | `EngineBridge.swift:89` | Pasted/precomposed non-ASCII keystroke → byte 0 | Acceptable for IME (keystrokes are ASCII); document. Optional: guard & pass-through non-ASCII unchanged |

> F1–F4 are real bugs worth fixing in this same pass since we are touching the FFI/Swift boundary anyway. F5 is acceptable; just document.

---

## 2. Target Design (decisions)

**One engine type, one FFI namespace, diff-based.**

- **FFI namespace:** rename `uvie_replay_*` → `uvie_engine_*` (single clean prefix), and **remove the old non-diff `uvie_engine_*` set** that Swift doesn't use. Net result: one opaque `UvieEngine` whose `feed`/`backspace`/`commit` are the diff API.
  - This is an intentional ABI break. It's an internal project (Swift is the only consumer); acceptable. If you must keep C source-compat for external users, instead keep `uvie_engine_*` as the diff API and just delete `uvie_replay_*` — but do not keep both.
- **Keep** introspection: `is_composing`, `committed_text`, `current_composing` (handy for tests/UI), plus config setters and `reset`/`clear`/`free`/`new`.
- **Decide on `feed_utf8` multibyte path:** keystrokes are ASCII, so the diff `feed(c_char)` is sufficient. Drop `pending_utf8` + `feed_utf8` unless a non-Swift consumer needs full-output mode. (Recommend drop; simpler.)
- **Delete `src/replay.rs`** and its exports once nothing references `ReplayEngine`.
- **Swift:** one shared engine instance used by both `EventTap` and `AXTextInjector` (fixes F3/F4). Delete `TextDiff.swift`.

---

## 3. Task Breakdown (ordered, each compiles + tests green)

### Phase 1 — Rust FFI consolidation
1. In `src/ffi.rs`, rename the `uvie_replay_*` functions → `uvie_engine_*` and rename `UvieReplayEngine` → `UvieEngine`. Keep bodies (they already call `feed_diff`/`backspace_diff`/`commit_diff`/`reset_diff`/`is_composing_diff`/`committed_text_diff`).
2. Delete the **old** non-diff `uvie_engine_*` functions and the `pending_utf8`/`decode_utf8_char`/`feed_utf8` machinery (unless keeping a non-diff consumer — see §2). Resolve the name collision from step 1 by removing the old set first.
3. Keep `write_output` / `utf8_prefix_len` helpers.
4. `cargo build` → regenerate `include/uvie.h`. Confirm it now contains a single `uvie_engine_*` set (diff API). Commit the regenerated header.
5. **Acceptance:** `cargo build` + `cargo test` green (Rust unit/integration tests not depending on `uvie_replay_*`).

### Phase 2 — Delete `replay.rs` ✅ ALREADY DONE (commit `e0cf36b`)
`src/replay.rs` is deleted and `lib.rs` is clean. Skip this phase. (Optional cleanup: rename internal `replay`-named helpers/tests per Status note above.)

### Phase 3 — Swift: single shared engine + new symbol names
1. `EngineBridge.swift`: update every `@_silgen_name("uvie_replay_*")` → `"uvie_engine_*"` to match Phase 1. Update the wrapper class doc (drop "ReplayEngine"). API of `EngineBridge` (Swift methods) can stay the same.
2. **Share one engine:** make `EngineBridge` injectable. `EventTap` creates the single `EngineBridge`; pass the same instance into `AXTextInjector` (constructor param) instead of `AXTextInjector` creating its own. Remove `AXTextInjector`'s private `_engine = EngineBridge()`.
   - Rationale: fixes F3/F4 — one engine, one config, consistent state across CGEvent and AX paths.
3. Delete `TextDiff.swift` (dead). Confirm no references remain.
4. **Acceptance:** app builds; typing works in normal apps and Spotlight with the same input method.

### Phase 4 — Wire the feature gaps (F1, F2, F4)
1. Add a single `applyEngineSettings()` on `EventTap` (or a small `EngineConfig` helper) that reads all of: `inputMethod`, `quickStart`, `quickTelex`, `modernOrthography` from `UserDefaults` and pushes them to the **shared** engine. Call it in `init` and whenever settings change.
2. **Propagate runtime changes:** observe the relevant `@AppStorage`/`UserDefaults` keys (`DefaultsKey.quickStart`, `.quickTelex`, `.modernOrthography`, `.inputMethod`). On change → `applyEngineSettings()`. (SwiftUI `@AppStorage` writes to `UserDefaults`; observe via `NotificationCenter`/`UserDefaults.didChangeNotification` or a Combine publisher, mirroring `InputMethodManager`'s pattern.)
3. Ensure `EngineBridge.setModernOrthography` is actually called (fixes F1). Add `modernOrthography` to `EventTap.loadSettings` too.
4. Keep `MenuBarController`'s input-method change working; route it through `applyEngineSettings()` so the AX engine (now shared) is also updated.
5. **Acceptance:** toggling Quick Telex / Modern Orthography / input method in Settings takes effect **without restart**, in both normal and Spotlight apps.

### Phase 5 — Tests + QA
1. Rust: `cargo test`, `cargo clippy -- -D warnings`, `no_std` build — all green. `git diff include/uvie.h` reflects the single `uvie_engine_*` diff API (intentional change, committed).
2. Swift: build the app; manual QA matrix:
   - Telex + VNI typing in a normal app (TextEdit) and a compound app (Safari/Chrome) and Spotlight.
   - Toggle Quick Telex / Quick Start / Modern Orthography at runtime → verify behavior changes immediately.
   - Switch input method from menu bar → verify both normal + Spotlight paths follow.
   - Backspace, space-commit, break-key-commit, V-C-V (`neebo`) behavior unchanged.

---

## 4. Constraints / Invariants

- Swift binds by **symbol name** (`@_silgen_name`) — renames must be applied in both Rust and Swift in the same change, or the app fails to link/launch.
- After consolidation there must be exactly **one** FFI engine type and **one** feed/backspace/commit semantics (diff). No `replay` anywhere (symbols, types, module, comments).
- `cargo clippy -- -D warnings` clean; std + `no_std/heapless` both build.
- Engine remains heap-light; FFI string output via existing `write_output` (128-byte caller buffers — keep; a composing syllable + committed prefix fits, but verify `out_len` callers use ≥128).
- Don't regress the compound-app / Chromium backspace handling in `EventTap` (the `sendEmptyCharacter` + `+1` backspace logic) — untouched by this refactor.

---

## 5. Risks

| # | Risk | Mitigation |
|---|------|-----------|
| 1 | Symbol rename desync (Rust renamed, Swift not) → app won't launch | Do Phase 1 + Phase 3 symbol edits together; grep both sides for `uvie_replay_` = 0 hits before building the app |
| 2 | Sharing one engine across CGEvent + AX paths introduces state bleed if both run concurrently | They don't run concurrently (one focused app at a time); `Mutex` in FFI already serializes. Verify `reset()` on focus change |
| 3 | Deleting `feed_utf8`/`pending_utf8` breaks a non-Swift consumer | Confirm no other consumer; it's internal. If unsure, keep `feed_utf8` under `uvie_engine_feed_full` |
| 4 | `replay.rs` deletion breaks tests | Phase 2 migrates/deletes tests first; `tests/feed_diff_tests.rs` should already cover the diff path |
| 5 | Runtime settings observer fires on background thread → engine mutated mid-keystroke | Apply settings on the same queue that drives the tap, or rely on FFI `Mutex`; keep changes idempotent |
| 6 | `committed_text` semantics differ between old `ReplayEngine` and `feed_diff` engine | Covered by `tests/feed_diff_tests.rs`; assert `committed_text` in QA |

---

## 6. Definition of Done

- [ ] No `replay` anywhere: `rg -i 'replay' src/ UVieKey/` returns only historical doc mentions (or zero).
- [ ] Single `uvie_engine_*` diff FFI; `include/uvie.h` regenerated + committed.
- [ ] `src/replay.rs` deleted; `lib.rs` cleaned; `cargo test` + `clippy -D warnings` + `no_std` build green.
- [ ] `TextDiff.swift` deleted; one shared `EngineBridge` used by `EventTap` + `AXTextInjector`.
- [ ] F1–F4 fixed: Quick Start/Telex, Modern Orthography, input method all apply at runtime in both normal and Spotlight paths. F5 documented.
- [ ] Manual QA matrix (§3 Phase 5) passes.

---

## 7. Quick reference

| File | Action |
|------|--------|
| `src/ffi.rs` | rename `uvie_replay_*`→`uvie_engine_*`, drop old non-diff set + `pending_utf8`/`feed_utf8` |
| `src/replay.rs` | delete (Phase 2) |
| `src/lib.rs` | remove `pub mod replay;` + `pub use ...ReplayEngine` |
| `include/uvie.h` | regenerated by cbindgen; commit |
| `EngineBridge.swift` | update `@_silgen_name` strings; injectable shared instance |
| `EventTap.swift` | create shared engine; `applyEngineSettings()` + runtime observers; pass engine to AX |
| `AXTextInjector.swift` | take shared engine via init; drop private engine |
| `TextDiff.swift` | delete (dead) |
| `MenuBarController.swift` | route input-method change through `applyEngineSettings()` |
| `tests/feed_diff_tests.rs` | ensure parity coverage before deleting replay tests |

Shell: prefix with `rtk` per `.windsurfrules` (e.g. `rtk cargo test`, `rg -i replay src/`).
