//! Diff-based IME wrapper around [`UltraFastViEngine`] with V-C-V syllable
//! boundary detection.
//!
//! # Purpose
//!
//! An IME (Input Method Editor) typically needs to know **what changed** on
//! screen, not the full current text. `ReplayEngine` wraps the stateful
//! `UltraFastViEngine` and turns its composing-text snapshots into
//! `(backspace_count, new_suffix)` diffs that a text-injection layer
//! (e.g. the macOS Accessibility `AXUIElement`) can apply directly.
//!
//! # V-C-V boundary detection
//!
//! Vietnamese is a mono-syllabic language. When the user types the start of a
//! second syllable (e.g. `neebo` = `nê` + `bo`), the engine detects the
//! vowel-consonant-vowel (V-C-V) boundary and auto-commits the first syllable:
//!
//! ```text
//! n  →  composing: "n"
//! e  →  composing: "ne"
//! e  →  composing: "nê"   (double-e modifier)
//! b  →  composing: "nêb"  (coda consonant, still valid)
//! o  →  BOUNDARY: commit "nê", start new composing "bo"
//! o  →  composing: "bô"   (double-o modifier on new syllable)
//! ```
//!
//! # Diff output
//!
//! ```text
//! prev_output = "nêb"  →  new screen = "nêbo"  →  diff = (0, "o")
//! prev_output = "nêbo" →  new screen = "nêbô"  →  diff = (1, "ô")
//! ```
//!
//! # Compatibility
//!
//! All `uvie_replay_*` FFI functions depend on `ReplayEngine`; its public API
//! is unchanged.

use crate::engine::UltraFastViEngine;
use crate::modes::InputMethod;

// ---------------------------------------------------------------------------
// ReplayEngine struct
// ---------------------------------------------------------------------------

/// Diff-based IME wrapper with V-C-V syllable auto-commit.
pub struct ReplayEngine {
    inner: UltraFastViEngine,
    /// Raw keystroke buffer for the current composing word.
    /// Used to replay the second syllable after a V-C-V split.
    raw_buf: arrayvec::ArrayVec<char, 24>,
    /// Exact string currently visible on screen (composing portion only).
    /// May differ from inner.out_buf by at most one optimistic consonant suffix.
    prev_output: String,
    /// What the inner engine last produced. Used as the true baseline for diffs
    /// when the optimistic display is active (so we don't drift over multiple keys).
    prev_inner_composed: String,
    /// Accumulated auto-committed text (syllables committed via V-C-V split).
    committed: String,
    /// Length of raw_buf at the point of the last valid (non-raw) Vietnamese output.
    /// Used for VCV detection: when the inner engine goes to raw passthrough after
    /// a vowel, we can replay from this checkpoint.
    last_valid_raw_len: usize,
    /// The last valid Vietnamese composing output (before it became raw passthrough).
    last_valid_out: String,
}

impl ReplayEngine {
    pub fn new() -> Self {
        Self {
            inner: UltraFastViEngine::new(),
            raw_buf: arrayvec::ArrayVec::new(),
            prev_output: String::new(),
            prev_inner_composed: String::new(),
            committed: String::new(),
            last_valid_raw_len: 0,
            last_valid_out: String::new(),
        }
    }

    // ------------------------------------------------------------------
    // Configuration
    // ------------------------------------------------------------------

    pub fn set_input_method(&mut self, method: InputMethod) {
        self.inner.set_input_method(method);
    }

    pub fn set_quick_start(&mut self, enabled: bool) {
        self.inner.set_quick_start(enabled);
    }

    pub fn set_quick_telex(&mut self, enabled: bool) {
        self.inner.set_quick_telex(enabled);
    }

    pub fn set_modern_orthography(&mut self, enabled: bool) {
        self.inner.set_modern_orthography(enabled);
    }

    // ------------------------------------------------------------------
    // Core API
    // ------------------------------------------------------------------

    /// Feed one character and return `(backspace_count, suffix_to_type)`.
    ///
    /// - `backspace_count`: trailing characters to delete from screen.
    /// - `suffix_to_type`: new text to append after deletion.
    ///
    /// On a word boundary the composing buffer is cleared and the character is
    /// returned as-is with zero backspaces.
    pub fn feed(&mut self, ch: char) -> (usize, String) {
        if is_word_boundary(ch) {
            self.inner.clear();
            self.raw_buf.clear();
            self.prev_output.clear();
            self.prev_inner_composed.clear();
            self.committed.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            return (0, ch.to_string());
        }

        // Push key.
        if self.raw_buf.is_full() {
            // Safety valve: buffer full — commit current word, start fresh.
            let screen_before = core::mem::take(&mut self.prev_output);
            let prev_composed = self.inner.current_composing().to_string();
            self.committed.push_str(&prev_composed);
            self.inner.clear();
            self.raw_buf.clear();
            self.prev_inner_composed.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            let _ = self.raw_buf.try_push(ch);
            let new_composed = self.inner.feed(ch).to_string();
            let (bs, suffix) = diff_outputs(&screen_before, &new_composed);
            self.prev_output = new_composed.clone();
            self.prev_inner_composed = new_composed;
            return (bs, suffix);
        }

        let raw_len_before = self.inner.raw_len();
        let _ = self.raw_buf.try_push(ch);
        let new_composed = self.inner.feed(ch).to_string();
        let raw_len_after = self.inner.raw_len();

        // Double-tone-cancel: inside the engine, push_raw_key adds 1 then
        // double-cancel subtracts 1, so net raw_len change = 0 instead of +1.
        // raw_buf always pushed the new char, so it grew by 1 more than inner.
        // Detect: if raw_len_after == raw_len_before (no net growth), one extra
        // byte sits in raw_buf that inner doesn't know about. Remove it.
        // We only handle net=0 (double-cancel eats exactly one byte). Larger drops
        // (modifier-cancel replays from scratch) are left alone — inner's backspace
        // replay handles them correctly via its own raw array.
        let double_cancel_fired = raw_len_after == raw_len_before && !self.raw_buf.is_empty();
        if double_cancel_fired {
            // Net zero growth: inner consumed the char + one previous byte.
            // raw_buf has one extra byte at position len-2 (the previous key).
            // Remove it: swap the just-pushed char to len-2, then truncate.
            let last_idx = self.raw_buf.len() - 1;
            if last_idx >= 1 {
                self.raw_buf.swap(last_idx - 1, last_idx);
                self.raw_buf.truncate(last_idx);
            }
            // The previously-valid state (last_valid_raw_len / last_valid_out) is
            // now stale because the syllable has changed. Reset it so that the
            // next consonant is NOT treated as an optimistic extension of the
            // old valid word — which would produce ghost diacritics ("nêb" bug).
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
        }

        let is_now_raw = is_raw_passthrough(&self.raw_buf, &new_composed);

        // Track the last valid (non-raw) composing state for VCV detection.
        // Every time the inner engine produces a Vietnamese-rendered output,
        // we record the raw length and output at that point.
        if !is_now_raw {
            self.last_valid_raw_len = self.raw_buf.len();
            self.last_valid_out = new_composed.clone();
        }

        // Optimistic display: if the inner engine just went to raw passthrough
        // after adding a SINGLE consonant (non-tone-key) to an otherwise valid
        // Vietnamese word, show the composed form with the consonant appended.
        // This lets "nêb" display while composing, giving better UX before the
        // next vowel triggers VCV detection.
        //
        // IMPORTANT: diff is computed against `prev_inner_composed` (the actual
        // inner engine output from last step), not `prev_output` (the possibly
        // optimistic screen display). This prevents drift when the user keeps
        // typing after the optimistic consonant.
        let is_optimistic = is_now_raw
            && !self.last_valid_out.is_empty()
            && !ch_is_consonant_tone_key(ch, &self.inner)
            && is_single_consonant_appended(&self.raw_buf, self.last_valid_raw_len);

        let display_composed = if is_optimistic {
            format!("{}{}", self.last_valid_out, ch)
        } else {
            new_composed.clone()
        };

        // Choose the correct baseline for diff:
        // - If the previous step showed an OPTIMISTIC display (e.g. "thạtt") but
        //   the inner engine had a different string ("thajtt"), and this step is NOT
        //   optimistic, diff from the inner engine's previous output, not the screen.
        //   This prevents garbled output when the user keeps typing after an
        //   optimistic consonant (the "dính chữ" / sticky-char bug).
        // - Otherwise diff from prev_output as normal.
        let prev_was_optimistic = self.prev_output != self.prev_inner_composed;
        let diff_baseline = if !is_optimistic && prev_was_optimistic {
            self.prev_inner_composed.clone()
        } else {
            self.prev_output.clone()
        };
        self.prev_inner_composed = new_composed.clone();

        // V-C-V boundary detection:
        // Triggered when the current char is a vowel AND the inner engine now
        // produces raw passthrough AND we have a previous valid Vietnamese state.
        let ch_is_vowel = is_ascii_vowel(ch as u8);
        if is_now_raw && ch_is_vowel && !self.last_valid_out.is_empty()
            && self.last_valid_raw_len < self.raw_buf.len()
        {
            // Find the split point using find_split_point with the last valid state.
            let actual_split = find_split_point(self.raw_buf.as_slice(), &self.last_valid_out, &self.inner);

            if actual_split > 0 {
                let split = actual_split;
                let committed_raw = &self.raw_buf[..split];
                let committed_out = replay_raw_slice(committed_raw);
                let new_syl_raw: arrayvec::ArrayVec<char, 24> =
                    self.raw_buf[split..].iter().copied().collect();

                // Commit the first syllable.
                self.committed.push_str(&committed_out);

                // Restart inner engine with the new syllable.
                self.inner.clear();
                let mut new_composed2 = String::new();
                for &c in new_syl_raw.iter() {
                    new_composed2 = self.inner.feed(c).to_string();
                }

                // Diff from the screen truth (prev_output, which may be optimistic).
                let full_screen = format!("{}{}", committed_out, new_composed2);
                let (bs, suffix) = diff_outputs(&self.prev_output, &full_screen);

                // Reset tracking — new syllable starts fresh.
                self.raw_buf = new_syl_raw;
                self.prev_output = new_composed2.clone();
                self.prev_inner_composed = new_composed2.clone();
                let is_new_raw = is_raw_passthrough(&self.raw_buf, &new_composed2);
                self.last_valid_raw_len = if is_new_raw { 0 } else { self.raw_buf.len() };
                self.last_valid_out = if is_new_raw { String::new() } else { new_composed2 };
                return (bs, suffix);
            }
        }

        // Normal path: diff from the correct baseline.
        let (bs, suffix) = diff_outputs(&diff_baseline, &display_composed);
        self.prev_output = display_composed;
        (bs, suffix)
    }

    /// Handle a physical Backspace key.
    pub fn backspace(&mut self) -> (usize, String) {
        if self.raw_buf.is_empty() {
            return (0, String::new());
        }
        self.raw_buf.pop();
        // Use prev_output as the screen truth (may be optimistic from prev step).
        let prev = core::mem::take(&mut self.prev_output);
        let new_composed = self.inner.backspace().to_string();
        self.prev_inner_composed = new_composed.clone();

        // When raw_buf is now empty, do a full reset so the engine state is
        // perfectly clean. This avoids any residual state causing sync issues
        // when the user types a new word after clearing with backspace.
        if self.raw_buf.is_empty() {
            self.inner.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            // prev_output stays "" (new_composed should also be "")
            let (bs, suffix) = diff_outputs(&prev, &new_composed);
            return (bs, suffix);
        }

        // Recompute last_valid state from the post-backspace output.
        let is_raw = is_raw_passthrough(&self.raw_buf, &new_composed);
        if is_raw {
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
        } else {
            self.last_valid_raw_len = self.raw_buf.len();
            self.last_valid_out = new_composed.clone();
        }
        let (bs, suffix) = diff_outputs(&prev, &new_composed);
        self.prev_output = new_composed;
        (bs, suffix)
    }

    /// Explicit commit (Enter, click, …). Clears composing state.
    pub fn commit(&mut self) -> (usize, String) {
        self.inner.clear();
        self.raw_buf.clear();
        self.prev_output.clear();
        self.prev_inner_composed.clear();
        self.last_valid_raw_len = 0;
        self.last_valid_out.clear();
        // NOTE: `committed` is NOT cleared — preserved for debugging/testing.
        (0, String::new())
    }

    /// Hard reset (focus lost, app switch, …). Clears everything.
    pub fn reset(&mut self) {
        self.inner.clear();
        self.raw_buf.clear();
        self.prev_output.clear();
        self.prev_inner_composed.clear();
        self.committed.clear();
        self.last_valid_raw_len = 0;
        self.last_valid_out.clear();
    }

    // ------------------------------------------------------------------
    // Introspection
    // ------------------------------------------------------------------

    /// Returns `true` when there is an active composing word.
    pub fn is_composing(&self) -> bool {
        self.inner.is_composing()
    }

    pub fn raw_len(&self) -> usize { self.raw_buf.len() }

    /// The composing text currently visible on screen.
    pub fn current_composing(&self) -> &str {
        &self.prev_output
    }

    /// Accumulated auto-committed text (from V-C-V splits). Useful for tests.
    pub fn committed_text(&self) -> &str {
        &self.committed
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` for characters that end the current composing word.
#[inline]
fn is_word_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '.' | ',' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{'
                | '}' | '\n' | '\r' | '\t'
        )
}

/// Returns `true` if the composed output is the same as the raw input
/// (i.e. no Vietnamese transforms were applied).
fn is_raw_passthrough(raw: &[char], composed: &str) -> bool {
    if raw.is_empty() {
        return true;
    }
    let raw_str: String = raw.iter().copied().collect();
    composed == raw_str
}

/// Find the index in `raw_buf` where the second syllable starts.
/// We binary-search for the largest prefix that still produces non-raw output.
fn find_split_point(
    raw: &[char],
    _prev_composed: &str,
    _engine: &UltraFastViEngine,
) -> usize {
    // The previous composed output was valid Vietnamese. Find the longest
    // prefix of raw that maps to a composed (non-raw) output.
    // Heuristic: scan from the end of prev_composed backward to find the
    // coda consonants that start the new syllable.
    // No need to replay — use the simpler heuristic below.

    // Count trailing consonants in raw that form the coda of the old syllable
    // or the onset of the new one. The split is right before the first
    // consonant that follows the last vowel group.
    //
    // Strategy: find last vowel in raw (before the new ch), the consonants
    // after it are the coda, and the new vowel ch starts the new syllable.
    // The split = position of the new vowel.
    let n = raw.len();
    if n == 0 { return 0; }

    // Find last vowel position (before the final char, which is the new vowel).
    let new_vowel_pos = n - 1; // last raw char is the new vowel that triggered V-C-V
    let mut last_old_vowel = 0usize;
    let mut found_old_vowel = false;
    for i in (0..new_vowel_pos).rev() {
        if is_ascii_vowel(raw[i] as u8) {
            last_old_vowel = i;
            found_old_vowel = true;
            break;
        }
    }
    if !found_old_vowel { return 0; }

    // The split is at the first consonant after last_old_vowel, OR at the
    // position of the new vowel if there are no intervening consonants.
    //
    // e.g. "neebo" = [n,e,e,b,o]:
    //   last_old_vowel = 2 ('e'), new_vowel_pos = 4 ('o')
    //   first consonant after position 2 = position 3 ('b')
    //   → split = 3  (committed = "nee", new syllable = "bo")
    //
    // e.g. "naabo" = [n,a,a,b,o]:
    //   last_old_vowel = 2 ('a'), new_vowel_pos = 4 ('o')
    //   first consonant after position 2 = position 3 ('b')
    //   → split = 3  (committed = "naa", new syllable = "bo")
    if last_old_vowel < new_vowel_pos {
        // Find the first consonant position after last_old_vowel.
        let first_cons_after_vowel = (last_old_vowel + 1..new_vowel_pos)
            .find(|&i| !is_ascii_vowel(raw[i] as u8))
            .unwrap_or(new_vowel_pos);
        return first_cons_after_vowel;
    }
    0
}

/// Replay a raw slice through a fresh engine and return the rendered output.
fn replay_raw_slice(raw: &[char]) -> String {
    let mut engine = UltraFastViEngine::new();
    let mut out = String::new();
    for &c in raw {
        out = engine.feed(c).to_string();
    }
    out
}

/// Compute the minimal diff from `old` → `new` as `(backspaces, suffix)`.
fn diff_outputs(old: &str, new: &str) -> (usize, String) {
    let mut common = 0usize;
    let mut old_iter = old.chars();
    let mut new_iter = new.chars();
    loop {
        match (old_iter.next(), new_iter.next()) {
            (Some(a), Some(b)) if a == b => common += 1,
            _ => break,
        }
    }
    let old_tail = old.chars().skip(common).count();
    let new_tail: String = new.chars().skip(common).collect();
    (old_tail, new_tail)
}

#[inline]
fn is_ascii_vowel(b: u8) -> bool {
    matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
}

/// Returns true if `ch` is a tone key in Telex (s, f, r, x, j, z).
/// These are never onset consonants, so if one appears at the end of raw_buf,
/// it cannot be the start of a new syllable.
#[inline]
fn ch_is_consonant_tone_key(ch: char, engine: &crate::engine::UltraFastViEngine) -> bool {
    use crate::modes::IS_TONE_KEY;
    let b = ch as u8;
    engine.mode_classify(b) & IS_TONE_KEY != 0
}

/// Returns true if exactly one consonant was appended after `last_valid_raw_len`.
/// Used for optimistic display: show "nêb" while composing even though 'b' is
/// not a legal Vietnamese coda.
#[inline]
fn is_single_consonant_appended(raw_buf: &[char], last_valid_raw_len: usize) -> bool {
    if raw_buf.len() != last_valid_raw_len + 1 {
        return false;
    }
    let ch = raw_buf[last_valid_raw_len];
    !is_ascii_vowel(ch as u8)
}
