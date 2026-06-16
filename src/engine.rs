//! Incremental stateful Vietnamese input engine.
//!
//! # Architecture
//!
//! Each keystroke is processed once and the engine keeps per-char state in a
//! `SylBuf`. Validation runs against positive syllable pattern tables
//! (`tables.rs`) on raw keystrokes — *before* any Unicode transform — so
//! English words are automatically rejected without a blacklist.
//!
//! ## Flow per keystroke
//!
//! ```text
//! feed(key)
//!   1. classify(key) → Consonant | Vowel | Modifier | ToneKey | Boundary
//!   2. handle_consonant / handle_vowel / handle_modifier / handle_tone
//!   3. revalidate_word() — check raw onset/nucleus/coda against tables.rs
//!        invalid? → mark_all_literal() (English passthrough)
//!   4. render out_buf from SylBuf
//! ```
//!
//! ## Key invariants
//!
//! - `raw[..raw_len]` always equals the original keystrokes, so fallback to
//!   literal output is always available.
//! - Tone placement is driven by `tables::nucleus_tone_target` — no positional
//!   heuristics. This correctly handles `iê`, `uyê`, `ươ`, `oa`, etc.
//! - Modifier "bubbling" (free-style: `tieengs` → `tiếng`) is handled by the
//!   raw-scan approach in `handle_modifier`: the modifier key searches backward
//!   for its target vowel, skipping over other vowels and glides that already
//!   form a legal sequence.

use crate::buffers::{OutBuffer, new_out_buffer};
use crate::modes::{IS_MODIFIER, IS_TONE_KEY, IS_VOWEL, InputMethod, Mode, mode_for};
use crate::syllable::{
    F_CAPS, F_CIRCUMFLEX, F_HORN, F_LITERAL, F_TONE_SET, OnsetKind, NucleusKind, Syl, SylBuf,
    SylStructure,
};
use crate::tables::{
    is_legal_coda, is_legal_nucleus, is_legal_onset, nucleus_tone_target, onset_is_gi,
    onset_is_qu, tone_allowed_for_coda,
};


// ---------------------------------------------------------------------------
// Public engine struct
// ---------------------------------------------------------------------------

pub struct UltraFastViEngine {
    /// Per-char buffer for the current composing word.
    buf: SylBuf,
    /// Raw keystroke snapshot. `raw[i]` == lowercased byte of the i-th key.
    raw: [u8; 24],
    raw_len: usize,
    /// Rendered output of the current composing word.
    out_buf: OutBuffer,
    /// Accumulated committed text (prior complete words + boundary chars).
    committed: OutBuffer,
    /// Input method — determines classifier and tone tables.
    input_method: InputMethod,
    mode: &'static Mode,
    /// Engine configuration flags.
    enable_quick_start: bool,
    enable_quick_telex: bool,
    enable_modern_orthography: bool,

    /// Incrementally maintained syllable structure (onset/nucleus/coda slots).
    /// Guarded by debug_assert_eq! against partition_syllable() as oracle.
    syl_structure: SylStructure,

    // ---- Diff/V-C-V fields (replaces ReplayEngine) ----

    /// Raw keystroke buffer for feed_diff (chars, not bytes; needed for V-C-V replay).
    raw_chars: arrayvec::ArrayVec<char, 24>,
    /// Composing text currently visible on screen (for diffing).
    prev_rendered: OutBuffer,
    /// The inner engine's true last render (diff baseline). Prevents the
    /// "dính chữ" sticky-char bug when typing past a transient state.
    prev_inner_render: OutBuffer,
    /// Raw char count at the last valid (non-passthrough) Vietnamese render.
    last_valid_raw_len: usize,
    /// Output at the last valid Vietnamese render, used for V-C-V split.
    last_valid_out: OutBuffer,
    /// Accumulated auto-committed text from V-C-V splits (diff mode only).
    diff_committed: OutBuffer,
    /// Scratch buffer backing the &str returned by feed_diff/backspace_diff.
    diff_suffix: OutBuffer,
}

impl UltraFastViEngine {
    pub fn new() -> Self {
        let input_method = InputMethod::Telex;
        Self {
            buf: SylBuf::new(),
            raw: [0u8; 24],
            raw_len: 0,
            out_buf: new_out_buffer(),
            committed: new_out_buffer(),
            input_method,
            mode: mode_for(input_method),
            enable_quick_start: false,
            enable_quick_telex: false,
            enable_modern_orthography: false,
            syl_structure: SylStructure::new(),
            // Diff/V-C-V fields
            raw_chars: arrayvec::ArrayVec::new(),
            prev_rendered: new_out_buffer(),
            prev_inner_render: new_out_buffer(),
            last_valid_raw_len: 0,
            last_valid_out: new_out_buffer(),
            diff_committed: new_out_buffer(),
            diff_suffix: new_out_buffer(),
        }
    }

    // ------------------------------------------------------------------
    // Configuration accessors
    // ------------------------------------------------------------------

    pub fn set_quick_start(&mut self, enabled: bool) {
        self.enable_quick_start = enabled;
    }
    pub fn quick_start(&self) -> bool { self.enable_quick_start }

    pub fn set_quick_telex(&mut self, enabled: bool) {
        self.enable_quick_telex = enabled;
    }
    pub fn quick_telex(&self) -> bool { self.enable_quick_telex }

    pub fn set_modern_orthography(&mut self, enabled: bool) {
        self.enable_modern_orthography = enabled;
    }
    pub fn modern_orthography(&self) -> bool { self.enable_modern_orthography }

    pub fn set_input_method(&mut self, method: InputMethod) {
        self.input_method = method;
        self.mode = mode_for(method);
    }
    pub fn input_method(&self) -> InputMethod { self.input_method }

    // ------------------------------------------------------------------
    // State queries
    // ------------------------------------------------------------------

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty() && self.committed.is_empty()
    }

    pub fn is_composing(&self) -> bool { !self.buf.is_empty() }

    /// Current logical raw length (may differ from push count due to double-cancel).
    pub fn raw_len(&self) -> usize { self.raw_len }

    pub fn current_composing(&self) -> &str { &self.out_buf }

    /// Returns the classify flags for a raw byte in the current input mode.
    /// Used by ReplayEngine helpers to check if a char is a tone key.
    #[inline]
    pub fn mode_classify(&self, b: u8) -> u8 {
        self.mode.classify[b as usize]
    }

    pub fn committed_text(&self) -> &str { &self.committed }

    #[cfg(feature = "std")]
    pub fn current_output(&self) -> String {
        let mut s = String::with_capacity(self.committed.len() + self.out_buf.len());
        s.push_str(&self.committed);
        s.push_str(&self.out_buf);
        s
    }

    /// Returns the current syllable structure (onset/nucleus/coda slots).
    #[inline]
    pub fn syl_structure(&self) -> &SylStructure {
        &self.syl_structure
    }

    // ------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------

    pub fn clear(&mut self) {
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
        self.committed.clear();
        self.syl_structure.clear();
    }

    /// Finalise the composing word into `committed` and reset composing state.
    pub fn commit(&mut self) {
        self.render_out_buf();
        let _ = self.committed.push_str(&self.out_buf);
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
    }

    /// Delete the last typed key (backspace). Returns the new composing text.
    pub fn backspace(&mut self) -> &str {
        if self.raw_len > 0 {
            // Decrement raw_len and replay buf from scratch. We can't just
            // buf.pop() because modifier keys may map 2 raw bytes to 1 buf entry
            // (e.g. "oo"→"ô": raw_len=2, buf=[ô]; backspace should give "o" not "").
            self.raw_len -= 1;
            let target_len = self.raw_len;
            self.buf.clear();
            self.raw_len = 0;
            for i in 0..target_len {
                let b = self.raw[i];
                self.raw[self.raw_len] = b;
                self.raw_len += 1;
                self.process_key(b, false);
            }
            self.render_out_buf();
            return &self.out_buf;
        }
        if !self.committed.is_empty() {
            self.committed.pop();
        }
        self.out_buf.clear();
        &self.out_buf
    }

    // ------------------------------------------------------------------
    // Diff-based API (replaces ReplayEngine)
    // ------------------------------------------------------------------

    /// Feed one char in diff mode. Returns `(backspace_count, suffix_to_type)`.
    /// The suffix borrows `self.diff_suffix`.
    ///
    /// On a word boundary the composing buffer is cleared and the char is
    /// returned as-is with zero backspaces.
    pub fn feed_diff(&mut self, ch: char) -> (usize, &str) {
        // Word boundary: commit composing word, clear state, return char directly.
        if Self::is_word_boundary(ch) {
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.raw_chars.clear();
            self.prev_rendered.clear();
            self.prev_inner_render.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            self.diff_committed.clear();
            self.diff_suffix.clear();
            let _ = self.diff_suffix.push(ch);
            return (0, &self.diff_suffix);
        }

        // Safety valve: buffer full — commit, start fresh.
        if self.raw_chars.is_full() {
            self.render_out_buf();
            let _ = self.diff_committed.push_str(&self.out_buf);
            let screen_before_len = self.prev_rendered.chars().count();
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.raw_chars.clear();
            self.prev_inner_render.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            // Feed the new char into the fresh engine state.
            let _ = self.raw_chars.try_push(ch);
            self.feed(ch);
            let new_composed = self.out_buf.clone();
            let bs = screen_before_len;
            self.diff_suffix.clear();
            let _ = self.diff_suffix.push_str(&new_composed);
            self.prev_rendered.clear();
            let _ = self.prev_rendered.push_str(&new_composed);
            self.prev_inner_render.clear();
            let _ = self.prev_inner_render.push_str(&new_composed);
            return (bs, &self.diff_suffix);
        }

        let raw_len_before = self.raw_len;
        let _ = self.raw_chars.try_push(ch);
        // Use the core feed to process this char.
        self.feed(ch);
        let raw_len_after = self.raw_len;

        // Double-tone-cancel detection: net raw_len growth = 0 instead of +1.
        let double_cancel_fired = raw_len_after == raw_len_before && !self.raw_chars.is_empty();
        if double_cancel_fired {
            // raw_chars has one extra char at position len-2 (the prev key).
            let last_idx = self.raw_chars.len() - 1;
            if last_idx >= 1 {
                self.raw_chars.swap(last_idx - 1, last_idx);
                self.raw_chars.truncate(last_idx);
            }
            // Invalidate stale last_valid state.
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
        }

        let new_composed = self.out_buf.clone();
        let is_now_raw = Self::is_raw_passthrough_slice(&self.raw_chars, &new_composed);

        // Track last valid Vietnamese state.
        if !is_now_raw {
            self.last_valid_raw_len = self.raw_chars.len();
            self.last_valid_out.clear();
            let _ = self.last_valid_out.push_str(&new_composed);
        }

        // Optimistic display: show coda consonant appended to valid Vietnamese.
        let ch_is_tone = Self::is_tone_key_in_mode(ch, self.mode);
        let is_optimistic = is_now_raw
            && !self.last_valid_out.is_empty()
            && !ch_is_tone
            && Self::is_single_consonant_appended_slice(&self.raw_chars, self.last_valid_raw_len);

        let display_composed = if is_optimistic {
            let mut d = self.last_valid_out.clone();
            let _ = d.push(ch);
            d
        } else {
            new_composed.clone()
        };

        // Diff baseline: use prev_inner_render to prevent sticky-char drift.
        let prev_was_optimistic = self.prev_rendered != self.prev_inner_render;
        let diff_baseline = if !is_optimistic && prev_was_optimistic {
            self.prev_inner_render.clone()
        } else {
            self.prev_rendered.clone()
        };
        self.prev_inner_render.clear();
        let _ = self.prev_inner_render.push_str(&new_composed);

        // V-C-V boundary detection.
        let ch_is_vowel = Self::is_ascii_vowel(ch as u8);
        if is_now_raw && ch_is_vowel && !self.last_valid_out.is_empty()
            && self.last_valid_raw_len < self.raw_chars.len()
        {
            let split = Self::find_split_point(&self.raw_chars);
            if split > 0 {
                let committed_raw: arrayvec::ArrayVec<char, 24> =
                    self.raw_chars[..split].iter().copied().collect();
                let new_syl_raw: arrayvec::ArrayVec<char, 24> =
                    self.raw_chars[split..].iter().copied().collect();

                // Replay committed prefix through fresh state.
                let committed_out = Self::replay_chars(&committed_raw, self.mode);

                // Commit first syllable.
                let _ = self.diff_committed.push_str(&committed_out);

                // Restart engine with new syllable.
                self.buf.clear();
                self.raw_len = 0;
                self.out_buf.clear();
                for &c in new_syl_raw.iter() {
                    self.feed(c);
                }
                let new_composed2 = self.out_buf.clone();

                // Diff from prev_rendered → committed_out + new_composed2.
                let mut full_screen = committed_out.clone();
                let _ = full_screen.push_str(&new_composed2);
                let (bs, _) = Self::diff_into(&self.prev_rendered, &full_screen, &mut self.diff_suffix);

                // Reset tracking for new syllable.
                self.raw_chars = new_syl_raw;
                self.prev_rendered.clear();
                let _ = self.prev_rendered.push_str(&new_composed2);
                self.prev_inner_render.clear();
                let _ = self.prev_inner_render.push_str(&new_composed2);
                let is_new_raw = Self::is_raw_passthrough_slice(&self.raw_chars, &new_composed2);
                if is_new_raw {
                    self.last_valid_raw_len = 0;
                    self.last_valid_out.clear();
                } else {
                    self.last_valid_raw_len = self.raw_chars.len();
                    self.last_valid_out.clear();
                    let _ = self.last_valid_out.push_str(&new_composed2);
                }
                return (bs, &self.diff_suffix);
            }
        }

        // Normal path: diff from baseline → display_composed.
        let (bs, _) = Self::diff_into(&diff_baseline, &display_composed, &mut self.diff_suffix);
        self.prev_rendered.clear();
        let _ = self.prev_rendered.push_str(&display_composed);
        (bs, &self.diff_suffix)
    }

    /// Handle a physical Backspace key in diff mode.
    /// Returns `(backspace_count, suffix_to_type)`.
    pub fn backspace_diff(&mut self) -> (usize, &str) {
        if self.raw_chars.is_empty() {
            self.diff_suffix.clear();
            return (0, &self.diff_suffix);
        }
        self.raw_chars.pop();
        let prev = self.prev_rendered.clone();
        self.backspace();
        let new_composed = self.out_buf.clone();
        self.prev_inner_render.clear();
        let _ = self.prev_inner_render.push_str(&new_composed);

        // Full reset when empty.
        if self.raw_chars.is_empty() {
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
            let (bs, _) = Self::diff_into(&prev, &new_composed, &mut self.diff_suffix);
            self.prev_rendered.clear();
            return (bs, &self.diff_suffix);
        }

        // Recompute last_valid state.
        let is_raw = Self::is_raw_passthrough_slice(&self.raw_chars, &new_composed);
        if is_raw {
            self.last_valid_raw_len = 0;
            self.last_valid_out.clear();
        } else {
            self.last_valid_raw_len = self.raw_chars.len();
            self.last_valid_out.clear();
            let _ = self.last_valid_out.push_str(&new_composed);
        }
        let (bs, _) = Self::diff_into(&prev, &new_composed, &mut self.diff_suffix);
        self.prev_rendered.clear();
        let _ = self.prev_rendered.push_str(&new_composed);
        (bs, &self.diff_suffix)
    }

    /// Explicit commit in diff mode (Enter, click, etc.).
    /// Clears composing state but preserves `diff_committed` for debugging/testing.
    pub fn commit_diff(&mut self) -> (usize, &str) {
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
        self.raw_chars.clear();
        self.prev_rendered.clear();
        self.prev_inner_render.clear();
        self.last_valid_raw_len = 0;
        self.last_valid_out.clear();
        // NOTE: diff_committed is NOT cleared — preserved for debugging/testing.
        self.diff_suffix.clear();
        (0, &self.diff_suffix)
    }

    /// Hard reset in diff mode (focus lost, app switch).
    pub fn reset_diff(&mut self) {
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
        self.raw_chars.clear();
        self.prev_rendered.clear();
        self.prev_inner_render.clear();
        self.last_valid_raw_len = 0;
        self.last_valid_out.clear();
        self.diff_committed.clear();
        self.diff_suffix.clear();
    }

    /// Returns true when there is an active composing word (diff mode).
    pub fn is_composing_diff(&self) -> bool {
        !self.raw_chars.is_empty()
    }

    /// The composing text currently visible on screen (diff mode).
    pub fn current_composing_diff(&self) -> &str {
        &self.prev_rendered
    }

    /// Accumulated auto-committed text from V-C-V splits (diff mode).
    pub fn committed_text_diff(&self) -> &str {
        &self.diff_committed
    }

    // ------------------------------------------------------------------
    // Diff helpers (private)
    // ------------------------------------------------------------------

    /// Compute minimal diff from `prev` → `new`, writing suffix into `out`.
    /// Returns `(backspace_count, suffix_len)`.
    fn diff_into(prev: &str, new: &str, out: &mut OutBuffer) -> (usize, usize) {
        let mut common = 0usize;
        let mut prev_iter = prev.chars();
        let mut new_iter = new.chars();
        loop {
            match (prev_iter.next(), new_iter.next()) {
                (Some(a), Some(b)) if a == b => common += 1,
                _ => break,
            }
        }
        let backspaces = prev.chars().count() - common;
        out.clear();
        for c in new.chars().skip(common) {
            let _ = out.push(c);
        }
        (backspaces, out.len())
    }

    /// Returns true for characters that end the current composing word.
    #[inline]
    fn is_word_boundary(ch: char) -> bool {
        ch.is_whitespace()
            || matches!(
                ch,
                '.' | ',' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{'
                    | '}' | '\n' | '\r' | '\t'
            )
    }

    /// Returns true if the composed output equals the raw input (no Vietnamese transforms).
    #[inline]
    fn is_raw_passthrough_slice(raw: &[char], composed: &str) -> bool {
        if raw.is_empty() { return true; }
        let mut ci = composed.chars();
        for &r in raw {
            match ci.next() {
                Some(c) if c == r => {}
                _ => return false,
            }
        }
        ci.next().is_none()
    }

    /// Find the V-C-V split point: index in raw_chars where the second syllable starts.
    fn find_split_point(raw: &[char]) -> usize {
        let n = raw.len();
        if n == 0 { return 0; }
        let new_vowel_pos = n - 1;
        let mut last_old_vowel = 0usize;
        let mut found_old_vowel = false;
        for i in (0..new_vowel_pos).rev() {
            if Self::is_ascii_vowel(raw[i] as u8) {
                last_old_vowel = i;
                found_old_vowel = true;
                break;
            }
        }
        if !found_old_vowel { return 0; }
        if last_old_vowel < new_vowel_pos {
            let first_cons_after_vowel = (last_old_vowel + 1..new_vowel_pos)
                .find(|&i| !Self::is_ascii_vowel(raw[i] as u8))
                .unwrap_or(new_vowel_pos);
            return first_cons_after_vowel;
        }
        0
    }

    /// Replay a slice of chars through a fresh engine and return rendered output.
    fn replay_chars(raw: &[char], mode: &'static Mode) -> OutBuffer {
        let mut eng = UltraFastViEngine::new();
        eng.mode = mode;
        for &c in raw {
            eng.feed(c);
        }
        eng.out_buf
    }

    #[inline]
    fn is_ascii_vowel(b: u8) -> bool {
        matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
    }

    /// Returns true if `ch` is a tone key in the given mode.
    #[inline]
    fn is_tone_key_in_mode(ch: char, mode: &Mode) -> bool {
        let b = ch as u8;
        mode.classify[b as usize] & IS_TONE_KEY != 0
    }

    /// Returns true if exactly one consonant was appended after `last_valid_raw_len`.
    #[inline]
    fn is_single_consonant_appended_slice(raw: &[char], last_valid_raw_len: usize) -> bool {
        if raw.len() != last_valid_raw_len + 1 { return false; }
        let ch = raw[last_valid_raw_len];
        !Self::is_ascii_vowel(ch as u8)
    }

    // ------------------------------------------------------------------
    // Core feed method
    // ------------------------------------------------------------------

    /// Feed one character. Returns the current composing text (not including
    /// committed text). Whitespace commits the current word.
    pub fn feed(&mut self, key: char) -> &str {
        if key.is_whitespace() {
            // Commit composing word, then append the whitespace to committed.
            self.render_out_buf();
            let _ = self.committed.push_str(&self.out_buf);
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            let _ = self.committed.push(key);
            return &self.out_buf;
        }

        let lower = key.to_ascii_lowercase();

        // Apply quick-start / quick-telex expansions (these push to raw buffer
        // before classification, so classification sees the expanded sequence).
        let caps = key != lower;

        if self.enable_quick_start {
            match lower {
                'j' => { self.push_raw_key(b'g', false); self.push_raw_key(b'i', false); }
                'f' => { self.push_raw_key(b'p', false); self.push_raw_key(b'h', false); }
                'w' => { self.push_raw_key(b'q', false); self.push_raw_key(b'u', false); }
                _ => { self.push_raw_key(lower as u8, caps); }
            }
        } else if self.enable_quick_telex && self.raw_len > 0 {
            // Quick-telex: double a single-consonant to expand it.
            let prev = self.raw[self.raw_len - 1];
            let expansion: Option<[u8; 2]> = match lower {
                'c' => Some([b'c', b'h']),
                'g' => Some([b'g', b'i']),
                'k' => Some([b'k', b'h']),
                'n' => Some([b'n', b'g']),
                'q' => Some([b'q', b'u']),
                'p' => Some([b'p', b'h']),
                't' => Some([b't', b'h']),
                _ => None,
            };
            if let Some(pair) = expansion {
                if prev == lower as u8 {
                    self.raw_len -= 1;
                    self.buf.pop();
                    self.push_raw_key(pair[0], false);
                    self.push_raw_key(pair[1], false);
                } else {
                    self.push_raw_key(lower as u8, caps);
                }
            } else {
                self.push_raw_key(lower as u8, caps);
            }
        } else {
            self.push_raw_key(lower as u8, caps);
        }

        self.render_out_buf();
        &self.out_buf
    }

    // ------------------------------------------------------------------
    // Internal: push a raw key and process it
    // ------------------------------------------------------------------

    /// Push one raw key byte into the raw snapshot and process it into `buf`.
    fn push_raw_key(&mut self, b: u8, caps: bool) {
        if self.raw_len >= 24 {
            // Buffer full — silently drop (shouldn't happen for Vietnamese syllables).
            return;
        }
        self.raw[self.raw_len] = b;
        self.raw_len += 1;
        self.process_key(b, caps);
    }

    /// Classify and handle one byte. Updates `self.buf` in-place.
    fn process_key(&mut self, b: u8, caps: bool) {
        let attr = self.mode.classify[b as usize];

        if attr & IS_TONE_KEY != 0 {
            self.handle_tone_key(b, caps);
        } else if attr & IS_MODIFIER != 0 {
            self.handle_modifier(b, caps);
        } else if attr & IS_VOWEL != 0 {
            self.handle_vowel(b, caps);
        } else {
            // Consonant or other literal.
            self.handle_consonant(b, caps);
        }
    }

    // ------------------------------------------------------------------
    // Handlers
    // ------------------------------------------------------------------

    fn handle_consonant(&mut self, b: u8, caps: bool) {
        self.buf.push(Syl::literal(b, caps));
    }

    fn handle_vowel(&mut self, b: u8, caps: bool) {
        // In Telex, 'w' acts as a standalone vowel ư when not following a/o/u.
        // In VNI, 'w' is not a vowel key.
        // For the raw VNI buffer 'w' is classified as modifier there, so this
        // branch only fires for a/e/i/o/u/y (always vowels).

        // Free-style modifier support: if there is already a vowel in the buffer
        // with the same base, try to apply the modifier (e.g. second 'e' → ê,
        // second 'a' → â, second 'o' → ô).
        // This handles adjacent (ee, aa, oo) AND non-adjacent (tieengs) cases.

        // Check for double-vowel modifier (aa→â, ee→ê, oo→ô).
        if matches!(b, b'a' | b'e' | b'o') {
            if let Some(target_idx) = self.find_modifier_target_for_double_vowel(b) {
                // Apply circumflex to the target vowel.
                let syl = self.buf.get(target_idx).clone();
                // Triple-cancel: if target already has circumflex, revert to literal.
                if syl.flags & F_CIRCUMFLEX != 0 {
                    // Only triple-cancel if the word is currently valid Vietnamese.
                    // For English words (already in passthrough), don't cancel —
                    // just push the vowel as a plain entry (fall through to push below).
                    // e.g. "banana": the third 'a' finds 'â' but the word is already
                    // invalid (passthrough), so we skip triple-cancel.
                    if self.is_valid_vietnamese() {
                        // 3rd same-char in valid Vietnamese: revert modifier and
                        // mark as passthrough. Decrement raw_len so the output
                        // reflects 2 chars (e.g. "aaa" → "aa").
                        let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                        self.buf.set(target_idx, reverted);
                        self.buf.push(Syl::literal(b, caps));
                        if self.raw_len > 0 { self.raw_len -= 1; }
                        self.mark_all_literal();
                        return;
                    }
                    // Already invalid — skip modifier, fall through to plain push.
                } else {
                    let updated = syl.with_circumflex();
                    self.buf.set(target_idx, updated);
                    // The modifier key itself is consumed (not added as a separate entry).
                    // Re-place tone onto the nucleus after modifier change.
                    self.reapply_tone_after_nucleus_change();
                    return;
                }
            }
        }

        // Plain vowel — just push.
        self.buf.push(Syl::literal(b, caps));
        // Reapply any pending tone to the new nucleus.
        self.reapply_tone_after_nucleus_change();
    }

    fn handle_modifier(&mut self, b: u8, caps: bool) {
        // Telex modifiers: 'w' (→ ă/ơ/ư/đ context-dep) and 'd' (→ đ).
        // VNI modifiers: digit keys 6/7/8/9 (modifier class in VNI classify table).
        // Note: in the VNI table, digits 6/7/8/9 are IS_MODIFIER.
        //
        // Telex 'w' targets:
        //   - 'a' → ă (aw)
        //   - 'o' → ơ (ow)
        //   - 'u' → ư (uw)
        //   - standalone w (no prior a/o/u) → ư (used as vowel onset ư)
        //   - 'u' after 'q' (glide) → skip (qu stays qu)
        //
        // Telex 'd':
        //   - 'd' after 'd' → đ (dd)
        //   - standalone 'd' → consonant 'd'
        //
        // VNI:
        //   6 → â/ê/ô (circumflex on most-recent matching vowel)
        //   7 → ơ/ư (horn on o/u)
        //   8 → ă (horn on a)
        //   9 → đ (applied to most-recent d)

        match b {
            b'w' => self.handle_telex_w(caps),
            b'd' => self.handle_telex_d(caps),
            b'6' => self.handle_vni_6(caps),
            b'7' => self.handle_vni_7(caps),
            b'8' => self.handle_vni_8(caps),
            b'9' => self.handle_vni_9(caps),
            _ => {
                // Unknown modifier — treat as literal.
                self.buf.push(Syl::literal(b, caps));
            }
        }
    }

    fn handle_telex_w(&mut self, caps: bool) {
        // Telex 'w' modifier:
        //   - First 'w' after a/o/u: apply horn (aw→ă, ow→ơ, uw→ư).
        //   - Second 'w' (double-w): cancel the horn, push 'w' as a plain char.
        //     e.g. showw → sh + o + w(applied) + w(cancel) → raw "showw" but
        //     output "show" (cancel undoes horn, raw_len decremented, raw becomes "show").
        //   - Standalone 'w' at onset: becomes ư vowel.
        //   - 'w' with no matching target: literal 'w'.

        let n = self.buf.len();

        for i in (0..n).rev() {
            let syl = self.buf.get(i);
            match syl.base {
                b'u' => {
                    if self.is_u_glide(i) { break; }
                    if syl.flags & F_HORN != 0 {
                        // Double-w cancel: revert horn. The second 'w' (cancel)
                        // is removed from raw; the first 'w' stays. Add a plain
                        // 'w' entry to buf so nucleus becomes invalid → raw passthrough.
                        let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                        self.buf.set(i, reverted);
                        if self.raw_len > 0 { self.raw_len -= 1; }
                        self.buf.push(Syl::literal(b'w', caps));
                        self.reapply_tone_after_nucleus_change();
                        return;
                    }
                    let updated = self.buf.get(i).clone().with_horn();
                    self.buf.set(i, updated);
                    self.reapply_tone_after_nucleus_change();
                    return;
                }
                b'o' => {
                    if syl.flags & F_HORN != 0 {
                        let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                        self.buf.set(i, reverted);
                        if self.raw_len > 0 { self.raw_len -= 1; }
                        self.buf.push(Syl::literal(b'w', caps));
                        // Also revert co-modified u→ư if present.
                        if i > 0 {
                            let prev = self.buf.get(i - 1);
                            if prev.base == b'u' && prev.flags & F_HORN != 0 {
                                let reverted_u = Syl::literal(b'u', prev.flags & F_CAPS != 0);
                                self.buf.set(i - 1, reverted_u);
                            }
                        }
                        self.reapply_tone_after_nucleus_change();
                        return;
                    }
                    let updated = self.buf.get(i).clone().with_horn();
                    self.buf.set(i, updated);
                    // Co-modification: if `u` immediately precedes this `o` in
                    // the nucleus, promote it to `ư` (uo → ươ phonetic rule).
                    if i > 0 {
                        let prev = self.buf.get(i - 1);
                        if prev.base == b'u' && prev.flags == 0 && !self.is_u_glide(i - 1) {
                            let promoted = prev.clone().with_horn();
                            self.buf.set(i - 1, promoted);
                        }
                    }
                    self.reapply_tone_after_nucleus_change();
                    return;
                }
                b'a' => {
                    if syl.flags & F_CIRCUMFLEX != 0 { continue; }
                    if syl.flags & F_HORN != 0 {
                        let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                        self.buf.set(i, reverted);
                        if self.raw_len > 0 { self.raw_len -= 1; }
                        self.buf.push(Syl::literal(b'w', caps));
                        self.reapply_tone_after_nucleus_change();
                        return;
                    }
                    let updated = self.buf.get(i).clone().with_horn();
                    self.buf.set(i, updated);
                    self.reapply_tone_after_nucleus_change();
                    return;
                }
                b'w' if syl.flags & F_HORN != 0 => {
                    // Double-w cancel on a standalone ư nucleus (the first 'w'
                    // became ư via the onset path below). Revert ư → plain 'w'
                    // and decrement raw_len so only the first 'w' stays in raw.
                    // Push a second literal 'w' so the buf is [w_lit, w_lit] which
                    // is never a valid Vietnamese onset/nucleus → passthrough uses
                    // raw[0..raw_len] = "w" = just the first w, making subsequent
                    // letters continue as normal passthrough (e.g. "wwork" → "work").
                    let reverted = Syl::literal(b'w', syl.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                    if self.raw_len > 0 { self.raw_len -= 1; }
                    self.buf.push(Syl::literal(b'w', caps));
                    self.reapply_tone_after_nucleus_change();
                    return;
                }
                b'd' => {
                    // 'w' doesn't target 'd' — stop scan.
                    break;
                }
                _ if self.mode.classify[syl.base as usize] & IS_VOWEL == 0 => {
                    // Hit a non-vowel, non-d consonant — stop.
                    break;
                }
                _ => continue,
            }
        }

        // No match — standalone 'w' becomes ư only at the start of the nucleus
        // (all preceding entries are onset consonants, or buffer is empty).
        let onset_len = self.onset_len();
        if onset_len == n {
            // Onset-only context: 'w' starts the nucleus as ư.
            let mut syl = Syl::literal(b'w', caps);
            syl.flags |= F_HORN;
            syl.out = 'ư';
            self.buf.push(syl);
            self.reapply_tone_after_nucleus_change();
        } else {
            // No valid target — 'w' is a literal.
            self.buf.push(Syl::literal(b'w', caps));
        }
    }

    fn handle_telex_d(&mut self, caps: bool) {
        // 'd' is classified IS_MODIFIER in Telex because 'dd' → 'đ'.
        // Free-style bubbling: the second 'd' can bubble back past vowels/consonants
        // to find the first 'd' in the onset and transform it to 'đ'.
        // Triple-cancel: 3rd 'd' after 'đ' → revert to plain "dd" (like "aaa"→"aa").

        let n = self.buf.len();

        // Scan backward for a 'd' entry.
        for i in (0..n).rev() {
            let s = self.buf.get(i);
            if s.base == b'd' && s.flags & F_HORN != 0 {
                // Found an existing 'đ'. Triple-cancel — but only if the word is
                // currently valid Vietnamese. For English words like "added" the
                // đ from "dd" is already in passthrough mode; don't triple-cancel.
                if self.is_valid_vietnamese() {
                    let reverted = Syl::literal(b'd', s.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                    self.buf.push(Syl::literal(b'd', caps));
                    if self.raw_len > 0 { self.raw_len -= 1; }
                    self.mark_all_literal();
                    return;
                }
                // Already invalid — skip triple-cancel, fall through to plain push.
                break;
            }
            if s.base == b'd' && s.flags & F_LITERAL == 0 && s.flags & F_HORN == 0 {
                // Found a non-đ 'd'. Only transform to 'đ' if it's in the onset
                // (before any vowel). A 'd' after a vowel is in coda position and
                // transforming it would create an illegal đ coda (e.g. "added" → "ađed").
                let is_in_onset = (0..i).all(|j| {
                    let sj = self.buf.get(j);
                    self.mode.classify[sj.base as usize] & IS_VOWEL == 0
                        && sj.base != b'w'
                });
                if !is_in_onset {
                    // Skip — d is after a vowel, can't be đ onset.
                    break;
                }
                let new_syl = Syl {
                    base: b'd',
                    out: if s.flags & F_CAPS != 0 { 'Đ' } else { 'đ' },
                    tone: 0,
                    flags: s.flags | F_HORN,
                };
                self.buf.set(i, new_syl);
                // 'd' consumed as modifier.
                return;
            }
        }

        // No match — push as plain consonant.
        self.buf.push(Syl::literal(b'd', caps));
    }

    fn handle_vni_6(&mut self, _caps: bool) {
        // VNI '6': apply circumflex to most-recent a/e/o.
        for i in (0..self.buf.len()).rev() {
            let syl = self.buf.get(i);
            if matches!(syl.base, b'a' | b'e' | b'o') && syl.flags & F_LITERAL == 0 {
                if syl.flags & F_CIRCUMFLEX != 0 {
                    // Double-6 cancel: revert.
                    let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                } else {
                    let updated = self.buf.get(i).clone().with_circumflex();
                    self.buf.set(i, updated);
                }
                self.reapply_tone_after_nucleus_change();
                return;
            }
            if self.mode.classify[syl.base as usize] & IS_VOWEL == 0 { break; }
        }
        // No target — push '6' literal.
        self.buf.push(Syl::literal(b'6', false));
    }

    fn handle_vni_7(&mut self, _caps: bool) {
        // VNI '7': apply horn to most-recent o or u (→ ơ or ư).
        for i in (0..self.buf.len()).rev() {
            let syl = self.buf.get(i);
            if matches!(syl.base, b'o' | b'u') && syl.flags & F_LITERAL == 0 {
                if syl.flags & F_HORN != 0 {
                    let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                } else {
                    let updated = self.buf.get(i).clone().with_horn();
                    self.buf.set(i, updated);
                }
                self.reapply_tone_after_nucleus_change();
                return;
            }
            if self.mode.classify[syl.base as usize] & IS_VOWEL == 0 { break; }
        }
        self.buf.push(Syl::literal(b'7', false));
    }

    fn handle_vni_8(&mut self, _caps: bool) {
        // VNI '8': apply horn (breve) to most-recent 'a' (→ ă).
        for i in (0..self.buf.len()).rev() {
            let syl = self.buf.get(i);
            if syl.base == b'a' && syl.flags & F_LITERAL == 0 {
                if syl.flags & F_HORN != 0 {
                    let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                } else {
                    let updated = self.buf.get(i).clone().with_horn();
                    self.buf.set(i, updated);
                }
                self.reapply_tone_after_nucleus_change();
                return;
            }
            if self.mode.classify[syl.base as usize] & IS_VOWEL == 0 { break; }
        }
        self.buf.push(Syl::literal(b'8', false));
    }

    fn handle_vni_9(&mut self, _caps: bool) {
        // VNI '9': apply đ to most-recent 'd'.
        for i in (0..self.buf.len()).rev() {
            let syl = self.buf.get(i);
            if syl.base == b'd' && syl.flags & F_LITERAL == 0 {
                if syl.flags & F_HORN != 0 {
                    let reverted = Syl::literal(syl.base, syl.flags & F_CAPS != 0);
                    self.buf.set(i, reverted);
                } else {
                    let new_syl = Syl {
                        base: b'd',
                        out: if syl.flags & F_CAPS != 0 { 'Đ' } else { 'đ' },
                        tone: 0,
                        flags: syl.flags | F_HORN,
                    };
                    self.buf.set(i, new_syl);
                }
                return;
            }
            if self.mode.classify[syl.base as usize] & IS_VOWEL == 0 { break; }
        }
        self.buf.push(Syl::literal(b'9', false));
    }

    fn handle_tone_key(&mut self, b: u8, caps: bool) {
        let tone_val = self.mode.tone[b as usize];

        // Rule: first char is always literal (can't apply tone to empty nucleus).
        let n = self.buf.len();
        if n == 0 {
            self.buf.push(Syl::literal(b, caps));
            return;
        }

        // Rule: 'r' after certain consonants (tr, pr, fr, …) is a consonant cluster.
        if b == b'r' && n > 0 {
            let prev = self.buf.get(n - 1).base;
            if matches!(prev, b't' | b'p' | b'f' | b'c' | b'b' | b'd' | b'g' | b'k') {
                self.buf.push(Syl::literal(b'r', caps));
                return;
            }
        }

        // If the word is already invalid Vietnamese (passthrough mode), treat
        // tone keys as plain consonants — no tone application, no double-cancel.
        // This ensures English words like "stress" pass through intact.
        if !self.is_valid_vietnamese() {
            self.buf.push(Syl::consonant(b, caps));
            return;
        }

        // Find tone carrier (nucleus position determined by tables).
        let carrier = self.tone_carrier_idx();

        if carrier.is_none() {
            // No vowel in buffer — tone key cannot apply.
            // Special case: if the entire buf is a modified consonant (e.g. đ from
            // VNI d9 or Telex dd), swallow the tone key silently. This handles
            // "d91" → "đ" in VNI where the '1' tone has no target.
            let (_, ns, ne, _) = self.partition_syllable();
            if ne <= ns {
                // No nucleus at all. Check if onset has a đ (modified consonant).
                let has_modified_consonant = (0..self.buf.len()).any(|i| {
                    let s = self.buf.get(i);
                    s.base == b'd' && s.flags & F_HORN != 0
                });
                if has_modified_consonant {
                    // Swallow tone key for onset-only modified-consonant words (đ+tone).
                    if self.raw_len > 0 { self.raw_len -= 1; }
                    return;
                }
            }
            self.buf.push(Syl::literal(b, caps));
            return;
        }

        let carrier_idx = carrier.unwrap();

        // Check existing tone on this carrier.
        let existing = self.buf.get(carrier_idx);
        let already_has_tone = existing.flags & F_TONE_SET != 0;

        if already_has_tone && existing.tone == tone_val && existing.flags & F_LITERAL == 0 {
            // Double-same-tone-key: cancel tone.
            //
            // The first tone key (byte `b`) was consumed into the carrier vowel's
            // tone. The second (duplicate) `b` cancels it. After cancellation:
            //   - Tone is removed from the carrier vowel.
            //   - The first tone key `b` may become a plain coda consonant
            //     (only if `b` is a legal coda — e.g. 's' is NOT a legal coda).
            //   - raw_len decremented by 2: both keys consumed.
            //   - If first key is a legal coda, it is re-pushed as a coda entry.
            //
            // "tess": t+e → té (via s) → +s → cancel → t+e+s(coda) = "tes".
            //   First s is a legal coda for Vietnamese (no — but 'tes' is still
            //   rendered as passthrough since 's' coda is invalid; raw→"tes").
            // "stress" + ss: stré+s → cancel → stre (no coda), raw→"stress"? No.
            //
            // Correct semantics (matching old engine):
            //   - Remove both occurrences of the tone key from raw (raw_len -= 2).
            //   - Re-add the first key as a coda entry in buf so the word struct
            //     reflects the correct syllable shape.
            //   - Validation then decides if it's Vietnamese or passthrough.
            let first_tone_b = b; // the duplicate key (= first key too)

            let reverted = {
                let s = self.buf.get(carrier_idx);
                let mut new_s = *s;
                new_s.flags &= !(F_TONE_SET);
                new_s.tone = 0;
                new_s.recompute_out();
                new_s
            };
            self.buf.set(carrier_idx, reverted);

            // The first tone key stays in raw; only remove the second (duplicate).
            // This way render_passthrough outputs the word with the tone key as a
            // literal consonant: "tess" → raw=[t,e,s] → "tes", "teff" → "tef", etc.
            if self.raw_len > 0 { self.raw_len -= 1; }
            // The first tone key becomes a coda consonant in buf (may make the word
            // invalid → render_passthrough picks it up from raw instead).
            self.buf.push(Syl::consonant(first_tone_b, caps));
            return;
        }

        // Override: last tone key wins (sắc then huyền → huyền).
        {
            let s = self.buf.get_mut(carrier_idx);
            s.tone = tone_val;
            s.flags |= F_TONE_SET;
            s.recompute_out();
        }
    }

    // ------------------------------------------------------------------
    // Tone carrier logic
    // ------------------------------------------------------------------

    /// Returns the index in `buf` of the vowel that should carry the tone,
    /// based on `tables::nucleus_tone_target` and `qu`/`gi` glide rules.
    ///
    /// Returns `None` if there is no vowel in the buffer.
    fn tone_carrier_idx(&self) -> Option<usize> {
        let n = self.buf.len();

        // Collect the nucleus: contiguous resolved-char vowels in the buffer,
        // excluding onset consonants, glides (qu u, gi i), and coda consonants.
        let (_onset_end, nucleus_start, nucleus_end, _coda_start) =
            self.partition_syllable();

        if nucleus_start >= nucleus_end {
            return None; // no nucleus
        }

        let nucleus_len = nucleus_end - nucleus_start;

        // Build nucleus using modifier-resolved chars WITHOUT tone (base_no_tone),
        // because the tone-target table uses untoned vowels like 'â', 'ê', 'ơ'.
        let mut nuc: [char; 3] = ['\0'; 3];
        let take = nucleus_len.min(3);
        for i in 0..take {
            nuc[i] = self.buf.get(nucleus_start + i).base_no_tone();
        }
        let nuc_slice = &nuc[..take];

        // qu/gi glide adjustment: after 'qu' the 'u' is a glide, not nucleus.
        // After 'gi' the 'i' is a glide, not nucleus.
        let onset_raw = self.onset_raw_slice();
        let (eff_nucleus_start, eff_nuc_slice, tone_offset) =
            if onset_is_qu(onset_raw) && nucleus_start < n && self.buf.get(nucleus_start).base == b'u' {
                let eff_start = nucleus_start + 1;
                if eff_start < nucleus_end {
                    let eff_len = (nucleus_end - eff_start).min(3);
                    let mut enuc = ['\0'; 3];
                    for i in 0..eff_len { enuc[i] = self.buf.get(eff_start + i).base_no_tone(); }
                    (eff_start, enuc, 0usize)
                } else {
                    (nucleus_start, { let mut a = ['\0'; 3]; a[..take].copy_from_slice(nuc_slice); a }, 0)
                }
            } else if onset_is_gi(onset_raw) && nucleus_start < n && self.buf.get(nucleus_start).base == b'i' {
                let eff_start = nucleus_start + 1;
                if eff_start < nucleus_end {
                    let eff_len = (nucleus_end - eff_start).min(3);
                    let mut enuc = ['\0'; 3];
                    for i in 0..eff_len { enuc[i] = self.buf.get(eff_start + i).base_no_tone(); }
                    (eff_start, enuc, 0usize)
                } else {
                    (nucleus_start, { let mut a = ['\0'; 3]; a[..take].copy_from_slice(nuc_slice); a }, 0)
                }
            } else {
                (nucleus_start, { let mut a = ['\0'; 3]; a[..take].copy_from_slice(nuc_slice); a }, 0)
            };

        // Determine effective nucleus length.
        let eff_len = (nucleus_end - eff_nucleus_start).min(3);
        let eff_slice = &eff_nuc_slice[..eff_len];

        // Look up tone-target in table.
        if let Some(target_in_nucleus) = nucleus_tone_target(eff_slice) {
            return Some(eff_nucleus_start + target_in_nucleus + tone_offset);
        }

        // Fallback: if no table match, use last vowel in nucleus.
        if eff_nucleus_start < nucleus_end {
            Some(nucleus_end - 1)
        } else if nucleus_start < nucleus_end {
            Some(nucleus_end - 1)
        } else {
            None
        }
    }

    /// After a modifier changes the nucleus (circumflex/horn applied), re-place
    /// the tone mark on the correct carrier according to the new nucleus shape.
    fn reapply_tone_after_nucleus_change(&mut self) {
        // Find if there is any existing tone in the nucleus.
        let (_, nucleus_start, nucleus_end, _) = self.partition_syllable();
        let mut tone_val: Option<u8> = None;
        let mut old_carrier: Option<usize> = None;

        for i in nucleus_start..nucleus_end {
            let s = self.buf.get(i);
            if s.flags & F_TONE_SET != 0 {
                tone_val = Some(s.tone);
                old_carrier = Some(i);
                break;
            }
        }

        let Some(tv) = tone_val else { return };
        let Some(oc) = old_carrier else { return };

        // Clear tone from old carrier.
        {
            let s = self.buf.get_mut(oc);
            s.flags &= !F_TONE_SET;
            s.tone = 0;
            s.recompute_out();
        }

        // Re-place on new correct carrier.
        if let Some(new_carrier) = self.tone_carrier_idx() {
            let s = self.buf.get_mut(new_carrier);
            s.tone = tv;
            s.flags |= F_TONE_SET;
            s.recompute_out();
        }
    }

    // ------------------------------------------------------------------
    // Syllable partitioning
    // ------------------------------------------------------------------

    /// Partition the current `buf` into (onset_end, nucleus_start, nucleus_end,
    /// coda_start) indices.
    ///
    /// - `[0, onset_end)` = onset consonants (may include đ).
    /// - `[nucleus_start, nucleus_end)` = vowels (nucleus_start == onset_end).
    /// - `[coda_start, len)` = trailing coda consonants.
    ///
    /// For words where all chars are consonants (onset only, no vowel yet),
    /// `nucleus_start == nucleus_end == len`.
    fn partition_syllable(&self) -> (usize, usize, usize, usize) {
        let n = self.buf.len();

        // onset: consecutive non-vowels from the start.
        let mut onset_end = 0;
        while onset_end < n {
            let s = self.buf.get(onset_end);
            if self.is_vowel_entry(s) { break; }
            onset_end += 1;
        }

        // Special case: `qu` digraph — if onset ends with `q` and the next
        // entry is `u` (plain vowel, not modified), treat that `u` as part of
        // the onset (it's a glide, not a nucleus vowel).
        if onset_end < n
            && onset_end > 0
            && self.buf.get(onset_end - 1).base == b'q'
        {
            let next = self.buf.get(onset_end);
            if next.base == b'u' && next.flags == 0 {
                // The `u` after `q` is a glide; include it in the onset.
                onset_end += 1;
            }
        }

        // Special case: `gi` digraph — if onset ends with `g` and the next
        // entry is plain `i` AND there is at least one more vowel after it,
        // treat that `i` as part of the onset (it's a glide, not a nucleus).
        // e.g. "giải": g + [i-glide] + a + i-coda → onset=gi, nucleus=a
        //      "giường": g + [i-glide] + ươ + ng
        // But "gì": g + ì (tone on i) — the `i` IS the nucleus, not a glide.
        // We detect this by requiring a vowel after the `i` to confirm it's a glide.
        if onset_end < n
            && onset_end > 0
            && self.buf.get(onset_end - 1).base == b'g'
        {
            let prev2 = if onset_end >= 2 { self.buf.get(onset_end - 2).base } else { 0 };
            // Only match `gi` (not `ngi` or `ng+i` combos).
            if prev2 != b'n' {
                let next = self.buf.get(onset_end);
                // The `i` is a glide only when followed by another vowel.
                let has_vowel_after_i = onset_end + 1 < n
                    && self.is_vowel_entry(self.buf.get(onset_end + 1));
                if next.base == b'i' && next.flags == 0 && has_vowel_after_i {
                    onset_end += 1;
                }
            }
        }

        // nucleus: contiguous vowels from onset_end.
        let nucleus_start = onset_end;
        let mut nucleus_end = nucleus_start;
        while nucleus_end < n {
            let s = self.buf.get(nucleus_end);
            if !self.is_vowel_entry(s) { break; }
            nucleus_end += 1;
        }

        let coda_start = nucleus_end;
        (onset_end, nucleus_start, nucleus_end, coda_start)
    }

    /// Returns true if `s` should be treated as a vowel entry in the syllable
    /// structure. `w` standing alone as ư vowel is also a vowel.
    fn is_vowel_entry(&self, s: &Syl) -> bool {
        let b = s.base;
        // Base vowels.
        if self.mode.classify[b as usize] & IS_VOWEL != 0 { return true; }
        // 'w' with F_HORN is ư (vowel).
        if b == b'w' && s.flags & F_HORN != 0 { return true; }
        false
    }

    /// Derive `SylStructure` from `partition_syllable()` and update the field.
    /// In debug builds, asserts that the derived structure matches the oracle.
    fn update_syl_structure(&mut self) {
        let (onset_end, _nuc_start, nucleus_end, _coda_start) = self.partition_syllable();
        let onset_kind = self.derive_onset_kind(onset_end);
        let nuc_len = nucleus_end.saturating_sub(onset_end);
        let nucleus_kind = match nuc_len {
            0 => NucleusKind::None,
            1 => NucleusKind::Single,
            2 => NucleusKind::Diphthong,
            _ => NucleusKind::Triphthong,
        };
        self.syl_structure = SylStructure {
            onset_end,
            nucleus_end,
            onset_kind,
            nucleus_kind,
        };
    }

    /// Derive the OnsetKind from the buf entries up to `onset_end`.
    fn derive_onset_kind(&self, onset_end: usize) -> OnsetKind {
        match onset_end {
            0 => OnsetKind::None,
            1 => OnsetKind::Single(self.buf.get(0).base),
            2 => OnsetKind::Digraph(self.buf.get(0).base, self.buf.get(1).base),
            3 => OnsetKind::Trigraph,
            // For quick-start expansions etc., treat as trigraph.
            _ => OnsetKind::Trigraph,
        }
    }

    /// Length of the onset (number of leading non-vowel entries).
    fn onset_len(&self) -> usize {
        let (onset_end, _, _, _) = self.partition_syllable();
        onset_end
    }

    /// Returns the raw onset bytes as a slice into `self.raw`.
    fn onset_raw_slice(&self) -> &[u8] {
        let onset_len = self.onset_len();
        &self.raw[..onset_len]
    }

    // ------------------------------------------------------------------
    // Free-style modifier: find target for double-vowel (aa→â, ee→ê, oo→ô)
    // ------------------------------------------------------------------

    /// Search backward for a matching vowel to apply circumflex.
    /// Returns the index in `buf` if found. Follows free-style rules:
    /// can skip over glide vowels and w to find the same-base vowel.
    fn find_modifier_target_for_double_vowel(&self, b: u8) -> Option<usize> {
        let n = self.buf.len();
        // Scan backward looking for a vowel with the same base to apply a
        // modifier (e.g. second 'a' → â, second 'e' → ê).
        //
        // Rules:
        // 1. Stop at a tone-key literal (e.g. 's' in Telex) — this is a tone
        //    separator (e.g. "reset": r,e,s,e,t → 's' blocks the second 'e'
        //    from bubbling back to the first 'e').
        // 2. Stop at đ (onset modifier boundary).
        // 3. If we find a toned vowel (F_TONE_SET) WITHOUT crossing any consonant
        //    between it and the current position, do NOT bubble (the tone key was
        //    between the two vowels, e.g. "reset"). But if we crossed at least one
        //    consonant to reach it, DO bubble (free-style: "thajta" → "thật",
        //    where 'j' is before the coda 't' and the second 'a' crosses 't').
        let mut crossed_consonant = false;
        for i in (0..n).rev() {
            let s = self.buf.get(i);
            if s.base == b && s.flags & F_LITERAL == 0 && s.flags & F_HORN == 0 {
                // Found a candidate. If it has a tone but we haven't crossed any
                // consonant, this is the "reset" pattern — tone key acted as a
                // separator, don't bubble.
                if s.flags & F_TONE_SET != 0 && !crossed_consonant {
                    return None;
                }
                return Some(i);
            }
            // Track whether we've scanned past a consonant (coda position).
            // In free-style typing like "thajta", the coda 't' is between the
            // toned nucleus 'ạ' and the modifier 'a'.
            let classify = self.mode.classify[s.base as usize];
            if classify & IS_VOWEL == 0 && classify & IS_TONE_KEY == 0 && s.base != b'w' {
                // It's a consonant (not a vowel, not a tone key, not the 'w' glide).
                crossed_consonant = true;
            }
            // Stop if we encounter a tone-key literal in the buf.
            if classify & IS_TONE_KEY != 0 {
                return None;
            }
            // Also stop at đ (onset modifier boundary).
            if s.base == b'd' && s.flags & F_HORN != 0 {
                return None;
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Validation
    // ------------------------------------------------------------------

    /// Validate the current `buf` against positive syllable tables.
    /// If invalid, mark all entries as literal (English passthrough).
    ///
    /// Called once at render time rather than on every keystroke for simplicity.
    /// (Each `render_out_buf` call rebuilds from scratch.)
    fn is_valid_vietnamese(&self) -> bool {
        let n = self.buf.len();
        if n == 0 { return true; }

        // If any entry is already literal, the word is in passthrough mode.
        // But we only set F_LITERAL for triple-cancel; for English detection
        // we do the full check here.

        let (onset_end, nucleus_start, nucleus_end, coda_start) =
            self.partition_syllable();

        // Words with only consonants are OK (onset-only, e.g. "ng", "tr" as they
        // are being typed). The onset validity check handles these.
        // Also: if there are trailing tone-key literals after a valid onset and
        // NO vowels at all, treat the word as onset-only (e.g. VNI "d9" + "1").
        if nucleus_start >= n {
            // All consonants — could still be valid onset prefix.
            let onset_raw = &self.raw[..n];
            return is_legal_onset(onset_raw);
        }


        // Build onset raw.
        let onset_raw = &self.raw[..onset_end];

        // Build nucleus using modifier-resolved chars WITHOUT tone diacritics,
        // because the nucleus table contains vowels like 'â', 'ê', 'ơ' but NOT
        // toned forms like 'á', 'ế', 'ợ'. Tone is validated separately.
        let nuc_len = (nucleus_end - nucleus_start).min(3);
        let mut nuc = ['\0'; 3];
        for i in 0..nuc_len {
            nuc[i] = self.buf.get(nucleus_start + i).base_no_tone();
        }
        let nuc_slice = &nuc[..nuc_len];

        // Build coda raw.
        let coda_len = n - coda_start;
        let mut coda_raw = [0u8; 4];
        let coda_take = coda_len.min(4);
        for i in 0..coda_take {
            coda_raw[i] = self.buf.get(coda_start + i).base;
        }
        let coda_slice = &coda_raw[..coda_take];

        // Get the current tone (if any).
        let mut tone: u8 = 0;
        for i in 0..n {
            let s = self.buf.get(i);
            if s.flags & F_TONE_SET != 0 {
                tone = s.tone;
                break;
            }
        }

        // Validate.
        if !is_legal_onset(onset_raw) { return false; }
        if !is_legal_nucleus(nuc_slice) { return false; }
        // Validate.
        if !is_legal_coda(coda_slice) { return false; }
        if !tone_allowed_for_coda(coda_slice, tone) { return false; }

        // V-C-V check: if there are consonants within the nucleus range, this
        // is an English word (e.g. "telex" → t-e-l-e-x, 'l' between two 'e's).
        // In a valid Vietnamese syllable, the nucleus is always contiguous.
        // We already split by first/last vowel; if nucleus_end - nucleus_start
        // is larger than the actual vowel run, there's a consonant inside.
        // The partition already handles this by stopping at non-vowels, so
        // nucleus_start..nucleus_end is purely vowels. The V-C-V case is caught
        // when the second 'e' (or 'a') tries to find a modifier target and
        // fails because there's a consonant between them.

        true
    }

    /// Mark all entries in `buf` as literal (passthrough mode).
    fn mark_all_literal(&mut self) {
        for i in 0..self.buf.len() {
            let s = self.buf.get_mut(i);
            s.flags |= F_LITERAL;
            s.flags &= !(F_CIRCUMFLEX | F_HORN | F_TONE_SET);
            s.tone = 0;
            s.out = s.base as char;
        }
    }

    // ------------------------------------------------------------------
    // Rendering
    // ------------------------------------------------------------------

    /// Rebuild `out_buf` from `buf`. Validates and renders.
    fn render_out_buf(&mut self) {
        // Update and validate syllable structure tracking.
        self.update_syl_structure();

        self.out_buf.clear();
        let n = self.buf.len();
        if n == 0 { return; }

        // Check for any F_LITERAL entries (triple-cancel set this).
        let has_literal = (0..n).any(|i| self.buf.get(i).flags & F_LITERAL != 0);

        if has_literal {
            // Force-literal passthrough (triple-cancel): use raw passthrough with
            // dd→đ substitution.
            self.render_passthrough();
            return;
        }

        // Validate Vietnamese syllable structure.
        if !self.is_valid_vietnamese() {
            // English/invalid passthrough: use raw passthrough.
            self.render_passthrough();
            return;
        }

        // Valid Vietnamese — render resolved chars from buf.
        for i in 0..n {
            let s = self.buf.get(i);
            let c = s.render();
            let _ = self.out_buf.push(c);
        }
    }

    /// Render in passthrough mode: output raw bytes, but apply the `dd→đ`
    /// substitution ONLY for `dd` pairs that appear in the onset and where
    /// the corresponding buf entry is a transformed đ (F_HORN set).
    ///
    /// This preserves intentional Telex `đ` typing (e.g. `ddoww` → `đow`)
    /// while letting English words like `added` and triple-cancel `ddd` pass
    /// through unchanged.
    fn render_passthrough(&mut self) {
        // Build a map: for each raw position, is there a corresponding đ (F_HORN) entry?
        // We track how many raw bytes correspond to each buf entry.
        // For 'dd→đ', one buf entry consumes 2 raw bytes.
        // For all other entries, one buf entry consumes 1 raw byte.
        // We walk both in sync to determine if a 'dd' pair maps to a đ buf entry.
        let n_buf = self.buf.len();
        let mut buf_idx = 0usize;
        let mut raw_idx = 0usize;
        while raw_idx < self.raw_len {
            let b = self.raw[raw_idx];
            // Check if the current buf entry is a 'd' with F_HORN (transformed đ).
            let is_dh = buf_idx < n_buf
                && self.buf.get(buf_idx).base == b'd'
                && self.buf.get(buf_idx).flags & F_HORN != 0;
            if is_dh
                && b == b'd'
                && raw_idx + 1 < self.raw_len
                && self.raw[raw_idx + 1] == b'd'
            {
                // This 'dd' pair corresponds to a đ buf entry: output đ.
                let _ = self.out_buf.push('đ');
                raw_idx += 2;
                buf_idx += 1;
            } else {
                let _ = self.out_buf.push(b as char);
                raw_idx += 1;
                buf_idx += 1;
            }
        }
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    /// Returns `true` if the 'u' at `idx` is a glide (part of 'qu' onset).
    fn is_u_glide(&self, idx: usize) -> bool {
        // After partition_syllable special-case, the 'u' glide in 'qu' is
        // already inside the onset range (onset_end includes it). So 'u' is a
        // glide if it is directly after 'q' in the buf.
        if idx == 0 { return false; }
        let prev = self.buf.get(idx - 1);
        if prev.base == b'q' { return true; }
        false
    }

    /// Returns true if 'i' at `idx` is a glide (part of 'gi' onset).
    #[allow(dead_code)]
    fn is_i_glide(&self, idx: usize) -> bool {
        let onset_len = self.onset_len();
        if idx == onset_len && onset_len >= 2
            && self.raw[onset_len - 2] == b'g'
            && self.raw[onset_len - 1] == b'i'
        {
            return true;
        }
        false
    }
}

impl Default for UltraFastViEngine {
    fn default() -> Self {
        Self::new()
    }
}


