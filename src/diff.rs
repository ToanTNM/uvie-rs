//! Diff-based input API and V-C-V syllable splitting.
//!
//! The diff engine wraps the core composing engine and computes minimal
//! (backspace_count, suffix_to_type) instructions for each keystroke.

use crate::buffers::{CharVec, OutBuffer, new_out_buffer};
use crate::engine::UltraFastViEngine;
use crate::modes::{IS_TONE_KEY, Mode};
use crate::composing::Composable;

/// Diff-mode state: tracks what's on screen vs what the engine produced.
pub struct DiffState {
    /// Raw keystroke buffer for feed_diff (chars, not bytes; needed for V-C-V split).
    pub raw_chars: CharVec<24>,
    /// Composing text currently visible on screen (for diffing).
    pub prev_rendered: OutBuffer,
    /// The inner engine's true last render (diff baseline).
    pub prev_inner_render: OutBuffer,
    /// Raw char count at the last valid (non-passthrough) Vietnamese render.
    pub last_valid_raw_len: usize,
    /// Coda start index at the last valid Vietnamese render (used to avoid optimistic display when the syllable already has a coda).
    pub last_valid_coda_start: usize,
    /// Output at the last valid Vietnamese render, used for V-C-V split.
    pub last_valid_out: OutBuffer,
    /// Accumulated auto-committed text from V-C-V splits (diff mode only).
    pub diff_committed: OutBuffer,
    /// Scratch buffer backing the &str returned by feed_diff/backspace_diff.
    pub diff_suffix: OutBuffer,
}

impl Default for DiffState {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffState {
    pub fn new() -> Self {
        Self {
            raw_chars: CharVec::new(),
            prev_rendered: new_out_buffer(),
            prev_inner_render: new_out_buffer(),
            last_valid_raw_len: 0,
            last_valid_coda_start: 0,
            last_valid_out: new_out_buffer(),
            diff_committed: new_out_buffer(),
            diff_suffix: new_out_buffer(),
        }
    }

    pub fn clear(&mut self) {
        self.raw_chars.clear();
        self.prev_rendered.clear();
        self.prev_inner_render.clear();
        self.last_valid_raw_len = 0;
        self.last_valid_coda_start = 0;
        self.last_valid_out.clear();
        self.diff_committed.clear();
        self.diff_suffix.clear();
    }
}

/// Diff-mode input API: minimal-edit instructions for each keystroke.
pub trait Diffable {
    fn feed_diff(&mut self, ch: char) -> (usize, &str);
    fn backspace_diff(&mut self) -> (usize, &str);
    fn commit_diff(&mut self) -> (usize, &str);
    fn reset_diff(&mut self);
    fn is_composing_diff(&self) -> bool;
    fn current_composing_diff(&self) -> &str;
    fn committed_text_diff(&self) -> &str;
    fn prev_inner_render_debug(&self) -> &str;
    fn prev_rendered_debug(&self) -> &str;
}

impl Diffable for UltraFastViEngine {
    fn feed_diff(&mut self, ch: char) -> (usize, &str) {
        // Word boundary: commit composing word, clear state, return char directly.
        if Self::is_word_boundary(ch) {
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.diff.clear();
            let _ = self.diff.diff_suffix.push(ch);
            return (0, &self.diff.diff_suffix);
        }

        // Safety valve: buffer full — commit, start fresh.
        if self.diff.raw_chars.is_full() {
            self.render_out_buf();
            let _ = self.diff.diff_committed.push_str(&self.out_buf);
            let screen_before_len = self.diff.prev_rendered.chars().count();
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.diff.raw_chars.clear();
            self.diff.prev_inner_render.clear();
            self.diff.last_valid_raw_len = 0;
            self.diff.last_valid_coda_start = 0;
            self.diff.last_valid_out.clear();
            let _ = self.diff.raw_chars.try_push(ch);
            self.feed(ch);
            let new_composed = self.out_buf.clone();
            let bs = screen_before_len;
            self.diff.diff_suffix.clear();
            let _ = self.diff.diff_suffix.push_str(&new_composed);
            self.diff.prev_rendered.clear();
            let _ = self.diff.prev_rendered.push_str(&new_composed);
            self.diff.prev_inner_render.clear();
            let _ = self.diff.prev_inner_render.push_str(&new_composed);
            return (bs, &self.diff.diff_suffix);
        }

        let raw_len_before = self.raw_len;
        let _ = self.diff.raw_chars.try_push(ch);
        self.feed(ch);
        let raw_len_after = self.raw_len;

        // Double-tone-cancel detection.
        let double_cancel_fired = raw_len_after == raw_len_before && !self.diff.raw_chars.is_empty();
        if double_cancel_fired {
            let last_idx = self.diff.raw_chars.len() - 1;
            if last_idx >= 1 {
                self.diff.raw_chars.swap(last_idx - 1, last_idx);
                self.diff.raw_chars.truncate(last_idx);
                // Update engine's raw_len to match the modified diff.raw_chars
                self.raw_len = self.diff.raw_chars.len();
            }
            self.diff.last_valid_raw_len = 0;
            self.diff.last_valid_coda_start = 0;
            self.diff.last_valid_out.clear();
        }

        let new_composed = self.out_buf.clone();
        let is_now_raw = Self::is_raw_passthrough_slice(&self.diff.raw_chars, &new_composed);

        if !is_now_raw {
            self.diff.last_valid_raw_len = self.diff.raw_chars.len();
            self.diff.last_valid_coda_start = Self::raw_coda_start(&self.diff.raw_chars);
            self.diff.last_valid_out.clear();
            let _ = self.diff.last_valid_out.push_str(&new_composed);
        }

        // Optimistic display: show coda consonant appended to valid Vietnamese.
        // Only use it when the valid Vietnamese syllable had no coda yet;
        // otherwise the screen and the engine's true state diverge, causing ghost characters.
        let ch_is_tone = Self::is_tone_key_in_mode(ch, self.mode);
        let mut optimistic_candidate = self.diff.last_valid_out.clone();
        let _ = optimistic_candidate.push(ch);
        let is_optimistic = is_now_raw
            && !self.diff.last_valid_out.is_empty()
            && !ch_is_tone
            && Self::is_single_consonant_appended_slice(&self.diff.raw_chars, self.diff.last_valid_raw_len)
            && self.diff.last_valid_coda_start == self.diff.last_valid_raw_len;

        let display_composed = if is_optimistic {
            optimistic_candidate
        } else {
            new_composed.clone()
        };

        // Diff baseline.
        let prev_was_optimistic = self.diff.prev_rendered != self.diff.prev_inner_render;
        let diff_baseline = if !is_optimistic && prev_was_optimistic {
            // When optimistic display is cancelled, diff from the optimistic display
            // (prev_rendered) to the new output, since that's what the user sees.
            self.diff.prev_rendered.clone()
        } else {
            self.diff.prev_rendered.clone()
        };
        self.diff.prev_inner_render.clear();
        let _ = self.diff.prev_inner_render.push_str(&new_composed);

        // V-C-V boundary detection.
        let ch_is_vowel = Self::is_ascii_vowel(ch as u8);
        if is_now_raw && ch_is_vowel && !self.diff.last_valid_out.is_empty()
            && self.diff.last_valid_raw_len < self.diff.raw_chars.len()
        {
            let split = Self::find_split_point(&self.diff.raw_chars);
            if split > 0 {
                let committed_raw: CharVec<24> =
                    self.diff.raw_chars[..split].iter().copied().collect();
                let new_syl_raw: CharVec<24> =
                    self.diff.raw_chars[split..].iter().copied().collect();

                let committed_out = Self::rerender_chars(&committed_raw, self.mode);

                let _ = self.diff.diff_committed.push_str(&committed_out);

                // Restart engine with new syllable.
                self.buf.clear();
                self.raw_len = 0;
                self.out_buf.clear();
                for &c in new_syl_raw.iter() {
                    self.feed(c);
                }
                let new_composed2 = self.out_buf.clone();

                let mut full_screen = committed_out.clone();
                let _ = full_screen.push_str(&new_composed2);
                let (bs, _) = Self::diff_into(&self.diff.prev_rendered, &full_screen, &mut self.diff.diff_suffix);

                self.diff.raw_chars = new_syl_raw;
                // CRITICAL FIX: Sync raw_len with diff.raw_chars after V-C-V split
                // Without this, backspace() will use wrong indices when replaying keystrokes
                self.raw_len = self.diff.raw_chars.len();
                self.diff.prev_rendered.clear();
                let _ = self.diff.prev_rendered.push_str(&new_composed2);
                self.diff.prev_inner_render.clear();
                let _ = self.diff.prev_inner_render.push_str(&new_composed2);
                let is_new_raw = Self::is_raw_passthrough_slice(&self.diff.raw_chars, &new_composed2);
                if is_new_raw {
                    self.diff.last_valid_raw_len = 0;
                    self.diff.last_valid_coda_start = 0;
                    self.diff.last_valid_out.clear();
                } else {
                    self.diff.last_valid_raw_len = self.diff.raw_chars.len();
                    self.diff.last_valid_coda_start = Self::raw_coda_start(&self.diff.raw_chars);
                    self.diff.last_valid_out.clear();
                    let _ = self.diff.last_valid_out.push_str(&new_composed2);
                }
                return (bs, &self.diff.diff_suffix);
            }
        }

        // Normal path: diff from baseline → display_composed.
        let (bs, _) = Self::diff_into(&diff_baseline, &display_composed, &mut self.diff.diff_suffix);
        self.diff.prev_rendered.clear();
        let _ = self.diff.prev_rendered.push_str(&display_composed);
        (bs, &self.diff.diff_suffix)
    }

    fn backspace_diff(&mut self) -> (usize, &str) {
        if self.diff.raw_chars.is_empty() {
            if !self.diff.diff_committed.is_empty() {
                self.diff.diff_committed.pop();
                self.diff.diff_suffix.clear();
                return (1, &self.diff.diff_suffix);
            }
            self.diff.diff_suffix.clear();
            return (0, &self.diff.diff_suffix);
        }
        self.diff.raw_chars.pop();
        let prev = self.diff.prev_rendered.clone();
        self.backspace();
        // Sync raw_len with diff.raw_chars after backspace
        self.raw_len = self.diff.raw_chars.len();
        let new_composed = self.out_buf.clone();
        self.diff.prev_inner_render.clear();
        let _ = self.diff.prev_inner_render.push_str(&new_composed);

        if self.diff.raw_chars.is_empty() {
            self.buf.clear();
            self.raw_len = 0;
            self.out_buf.clear();
            self.diff.last_valid_raw_len = 0;
            self.diff.last_valid_coda_start = 0;
            self.diff.last_valid_out.clear();
            let (bs, _) = Self::diff_into(&prev, &new_composed, &mut self.diff.diff_suffix);
            self.diff.prev_rendered.clear();
            self.diff.prev_inner_render.clear();
            return (bs, &self.diff.diff_suffix);
        }

        let is_raw = Self::is_raw_passthrough_slice(&self.diff.raw_chars, &new_composed);
        if is_raw {
            self.diff.last_valid_raw_len = 0;
            self.diff.last_valid_coda_start = 0;
            self.diff.last_valid_out.clear();
        } else {
            self.diff.last_valid_raw_len = self.diff.raw_chars.len();
            self.diff.last_valid_coda_start = Self::raw_coda_start(&self.diff.raw_chars);
            self.diff.last_valid_out.clear();
            let _ = self.diff.last_valid_out.push_str(&new_composed);
        }
        let (bs, _) = Self::diff_into(&prev, &new_composed, &mut self.diff.diff_suffix);
        self.diff.prev_rendered.clear();
        let _ = self.diff.prev_rendered.push_str(&new_composed);

        // Debug validation: ensure raw_len stays in sync with diff.raw_chars
        #[cfg(debug_assertions)]
        {
            if self.raw_len != self.diff.raw_chars.len() {
                panic!(
                    "raw_len ({}) != diff.raw_chars.len() ({}) after backspace_diff",
                    self.raw_len, self.diff.raw_chars.len()
                );
            }
        }

        (bs, &self.diff.diff_suffix)
    }

    fn commit_diff(&mut self) -> (usize, &str) {
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
        self.diff.raw_chars.clear();
        self.diff.prev_rendered.clear();
        self.diff.prev_inner_render.clear();
        self.diff.last_valid_raw_len = 0;
        self.diff.last_valid_coda_start = 0;
        self.diff.last_valid_out.clear();
        // NOTE: diff_committed is NOT cleared here - it accumulates across commits
        // It only gets cleared on reset_diff() or word boundary (via diff.clear())
        self.diff.diff_suffix.clear();
        (0, &self.diff.diff_suffix)
    }

    fn reset_diff(&mut self) {
        self.buf.clear();
        self.raw_len = 0;
        self.out_buf.clear();
        self.diff.clear();
    }

    fn is_composing_diff(&self) -> bool {
        !self.diff.raw_chars.is_empty()
    }

    fn current_composing_diff(&self) -> &str {
        &self.diff.prev_rendered
    }

    fn committed_text_diff(&self) -> &str {
        &self.diff.diff_committed
    }

    fn prev_inner_render_debug(&self) -> &str {
        &self.diff.prev_inner_render
    }

    fn prev_rendered_debug(&self) -> &str {
        &self.diff.prev_rendered
    }
}

// Static helper methods on UltraFastViEngine used by the diff module.
impl UltraFastViEngine {
    /// Compute minimal diff from `prev` → `new`, writing suffix into `out`.
    pub(crate) fn diff_into(prev: &str, new: &str, out: &mut OutBuffer) -> (usize, usize) {
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
    pub(crate) fn is_word_boundary(ch: char) -> bool {
        ch.is_whitespace()
            || matches!(
                ch,
                '.' | ',' | '!' | '?' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{'
                    | '}' | '\n' | '\r' | '\t'
            )
    }

    /// Returns true if the composed output equals the raw input (no Vietnamese transforms).
    #[inline]
    pub(crate) fn is_raw_passthrough_slice(raw: &[char], composed: &str) -> bool {
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
    pub(crate) fn find_split_point(raw: &[char]) -> usize {
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

    /// Re-render a slice of chars through a fresh engine and return rendered output.
    pub(crate) fn rerender_chars(raw: &[char], mode: &'static Mode) -> OutBuffer {
        let mut eng = UltraFastViEngine::new();
        eng.mode = mode;
        for &c in raw {
            eng.feed(c);
        }
        eng.out_buf
    }

    #[inline]
    pub(crate) fn is_ascii_vowel(b: u8) -> bool {
        matches!(b, b'a' | b'e' | b'i' | b'o' | b'u' | b'y')
    }

    #[inline]
    pub(crate) fn is_tone_key_in_mode(ch: char, mode: &Mode) -> bool {
        let b = ch as u8;
        mode.classify[b as usize] & IS_TONE_KEY != 0
    }

    #[inline]
    pub(crate) fn is_single_consonant_appended_slice(raw: &[char], last_valid_raw_len: usize) -> bool {
        if raw.len() != last_valid_raw_len + 1 { return false; }
        let ch = raw[last_valid_raw_len];
        !Self::is_ascii_vowel(ch as u8)
    }

    /// Find the raw index where the coda starts (index past the last vowel).
    /// If there is no vowel, the whole slice is treated as onset/coda.
    #[inline]
    pub(crate) fn raw_coda_start(raw: &[char]) -> usize {
        let mut last_vowel = None;
        for (i, &c) in raw.iter().enumerate() {
            if Self::is_ascii_vowel(c as u8) {
                last_vowel = Some(i);
            }
        }
        last_vowel.map(|i| i + 1).unwrap_or(0)
    }
}
