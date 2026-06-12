use arrayvec::ArrayVec;

use crate::engine::UltraFastViEngine;
use crate::modes::InputMethod;

/// Pure-functional replay wrapper around `UltraFastViEngine`.
///
/// On every keystroke that is part of a Vietnamese word, replays the entire
/// raw keystroke buffer through a fresh engine instance. This guarantees
/// 100% accuracy for tone placement on complex syllables (diphthongs,
/// multi-character codas) at the cost of O(n) per keystroke where n ≤ 10.
///
/// The wrapper is **not** `no_std` / `no_alloc` safe — it uses `String` for
/// the public API. The internal buffer is stack-only (`ArrayVec`).
pub struct ReplayEngine {
    inner: UltraFastViEngine,
    method: InputMethod,
    quick_start: bool,
    quick_telex: bool,
    modern_ortho: bool,
    /// Raw keystrokes of the current word (max ~10 for Vietnamese).
    raw_buf: ArrayVec<char, 16>,
    /// Full rendered output from the *previous* keystroke (for diffing).
    prev_output: String,
}

impl ReplayEngine {
    pub fn new() -> Self {
        let method = InputMethod::Telex;
        let mut inner = UltraFastViEngine::new();
        inner.set_input_method(method);
        Self {
            inner,
            method,
            quick_start: false,
            quick_telex: false,
            modern_ortho: false,
            raw_buf: ArrayVec::new(),
            prev_output: String::new(),
        }
    }

    pub fn set_input_method(&mut self, method: InputMethod) {
        self.method = method;
        self.inner.set_input_method(method);
    }

    pub fn set_quick_start(&mut self, enabled: bool) {
        self.quick_start = enabled;
    }

    pub fn set_quick_telex(&mut self, enabled: bool) {
        self.quick_telex = enabled;
    }

    pub fn set_modern_orthography(&mut self, enabled: bool) {
        self.modern_ortho = enabled;
    }

    // ------------------------------------------------------------------
    // Core API
    // ------------------------------------------------------------------

    /// Feed a single character.
    ///
    /// Returns `(backspace_count, suffix_to_type)`:
    /// - `backspace_count`: how many trailing characters the caller must delete.
    /// - `suffix_to_type`: the new suffix to type after deleting.
    ///
    /// Only the changed suffix is returned, minimising screen flicker.
    /// On a word boundary the internal buffer is cleared and the character is
    /// returned as-is with zero backspaces.
    pub fn feed(&mut self, ch: char) -> (usize, String) {
        if is_word_boundary(ch) {
            self.raw_buf.clear();
            self.prev_output.clear();
            self.inner.clear();
            return (0, ch.to_string());
        }

        let prev = self.replay_current();
        self.raw_buf.push(ch);
        let new_output = self.replay_current();
        let (bs, suffix) = diff_outputs(&prev, &new_output);
        self.prev_output.clear();
        self.prev_output.push_str(&new_output);
        (bs, suffix)
    }

    /// Call when the user presses Backspace.
    ///
    /// Returns `(backspace_count, suffix_to_type)`.
    /// If the buffer is empty, returns `(0, "")` so the OS handles the backspace.
    pub fn backspace(&mut self) -> (usize, String) {
        if self.raw_buf.is_empty() {
            return (0, String::new());
        }
        let prev = self.replay_current();
        self.raw_buf.pop();
        let new_output = self.replay_current();
        let (bs, suffix) = diff_outputs(&prev, &new_output);
        self.prev_output.clear();
        self.prev_output.push_str(&new_output);
        (bs, suffix)
    }

    /// Call on word boundary (space, punctuation, Enter) to finalize the word.
    ///
    /// Just clears state; the caller passes the boundary char through normally.
    pub fn commit(&mut self) -> (usize, String) {
        self.raw_buf.clear();
        self.prev_output.clear();
        self.inner.clear();
        (0, String::new())
    }

    /// Hard reset (e.g. when user clicks elsewhere).
    pub fn reset(&mut self) {
        self.raw_buf.clear();
        self.prev_output.clear();
        self.inner.clear();
    }

    // ------------------------------------------------------------------
    // Query
    // ------------------------------------------------------------------

    pub fn is_composing(&self) -> bool {
        !self.raw_buf.is_empty()
    }

    pub fn current_composing(&self) -> String {
        self.replay_current()
    }

    // ------------------------------------------------------------------
    // Internal
    // ------------------------------------------------------------------

    /// Replay `raw_buf` through a fresh engine and return the rendered output.
    fn replay_current(&self) -> String {
        let mut e = UltraFastViEngine::new();
        e.set_input_method(self.method);
        e.set_quick_start(self.quick_start);
        e.set_quick_telex(self.quick_telex);
        e.set_modern_orthography(self.modern_ortho);

        let mut out = String::new();
        for &ch in &self.raw_buf {
            out = e.feed(ch).to_string();
        }
        out
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn is_word_boundary(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t' | '\n' | '\r' |
        '.' | ',' | '!' | '?' | ';' | ':' |
        '(' | ')' | '[' | ']' | '{' | '}' |
        '"' | '\'' | '/' | '\\' | '-' | '_'
    )
}

/// Compute the minimal diff between two strings.
/// Returns `(backspaces, suffix)`:
/// - `backspaces`: number of trailing chars in `old` that must be deleted.
/// - `suffix`: the new trailing chars from `new` that must be typed.
fn diff_outputs(old: &str, new: &str) -> (usize, String) {
    if old.is_empty() {
        return (0, new.to_string());
    }
    if new.is_empty() {
        return (old.chars().count(), String::new());
    }

    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();

    // Find longest common prefix
    let mut common = 0;
    let min_len = old_chars.len().min(new_chars.len());
    while common < min_len && old_chars[common] == new_chars[common] {
        common += 1;
    }

    let backspaces = old_chars.len() - common;
    let suffix: String = new_chars[common..].iter().collect();
    (backspaces, suffix)
}
