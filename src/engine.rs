use crate::buffers::{OutBuffer, RawBuffer, new_out_buffer, new_raw_buffer};
use crate::modes::{IS_TONE_KEY, IS_VOWEL, InputMethod, Mode, ModeTrait, ResolverKind, mode_for, TelexMode, VniMode};
use crate::tone::{is_vowel_unicode, map_vowel_with_tone};

/// Bitmask lookup table for invalid Vietnamese consonant pairs.
/// Index = (c1 - b'a') * 26 + (c2 - b'a'), value = true if pair is invalid.
static INVALID_PAIR_TABLE: [bool; 676] = {
    let mut t = [false; 676];
    // Helper: encode pair as index
    macro_rules! mark {
        ($a:expr, $b:expr) => {
            t[($a - b'a') as usize * 26 + ($b - b'a') as usize] = true;
        };
    }
    mark!(b'c', b'l'); mark!(b'f', b'l'); mark!(b'b', b'l'); mark!(b'g', b'l');
    mark!(b's', b'l'); mark!(b'p', b'l');
    mark!(b'b', b'r'); mark!(b'p', b'r'); mark!(b'd', b'r'); mark!(b'f', b'r');
    mark!(b'g', b'r'); mark!(b'k', b'r');
    mark!(b's', b't'); mark!(b's', b'p'); mark!(b's', b'k');
    mark!(b'p', b't'); mark!(b'p', b'c'); mark!(b'p', b'g'); mark!(b'p', b'q');
    mark!(b'p', b's'); mark!(b'p', b'k'); mark!(b'p', b'd'); mark!(b'p', b'f');
    mark!(b'p', b'b');
    t
};

pub struct UltraFastViEngine {
    raw_buffer: RawBuffer,
    out_buffer: OutBuffer,
    input_method: InputMethod,
    mode: &'static Mode,
}

impl UltraFastViEngine {
    pub fn new() -> Self {
        let input_method = InputMethod::Telex;
        Self {
            raw_buffer: new_raw_buffer(),
            out_buffer: new_out_buffer(),
            input_method,
            mode: mode_for(input_method),
        }
    }

    pub fn clear(&mut self) {
        self.raw_buffer.clear();
        self.out_buffer.clear();
    }

    pub fn set_input_method(&mut self, method: InputMethod) {
        self.input_method = method;
        self.mode = mode_for(method);
    }

    pub fn input_method(&self) -> InputMethod {
        self.input_method
    }

    pub fn feed(&mut self, key: char) -> &str {
        if key.is_whitespace() {
            self.render_str();
            self.raw_buffer.clear();
            let _ = self.out_buffer.push(key);
            return &self.out_buffer;
        }
        let _ = self.raw_buffer.push(key.to_ascii_lowercase());
        self.render_str()
    }

    #[inline(always)]
    fn render_str(&mut self) -> &str {
        if self.raw_buffer.is_empty() {
            self.out_buffer.clear();
            return &self.out_buffer;
        }

        let bytes_all = self.raw_buffer.as_bytes();
        let len = bytes_all.len().min(32);

        // Filter tone + Toggling in one pass.
        // Table lookups use get_unchecked (index is u8, tables are [u8; 256]).
        // Lookahead bounds checks only run for tone keys (rare and predictable branch).
        let mut toggled = [0u8; 40];
        let mut t_len = 0usize;
        let mut last_tone_char = 0u8;
        let mut tone_cancelled = false;
        let mut run_char: u8 = 0;
        let mut run_count: u8 = 0;
        let mut seen_mod: u8 = 0;
        let mut need_mod_bubble = false;
        let mut has_w = false;

        let mut idx = 0usize;
        while idx < len {
            let b = unsafe { *bytes_all.get_unchecked(idx) };
            let attr = unsafe { *self.mode.classify.get_unchecked(b as usize) };
            let is_tone = (attr & IS_TONE_KEY) != 0;

            if is_tone {
                if idx == 0 {
                    // Rule 1: First char is always literal
                    unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                    t_len += 1;
                    idx += 1;
                    continue;
                }

                // Rule 2: 'r' after certain consonants forms a cluster
                if b == b'r' {
                    let prev = unsafe { *bytes_all.get_unchecked(idx - 1) };
                    if matches!(prev, b't' | b'p' | b'f' | b'c' | b'b' | b'd' | b'g' | b'k') {
                        unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                        t_len += 1;
                        idx += 1;
                        continue;
                    }
                }

                // Double tone key cancellation
                if b == last_tone_char {
                    unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                    t_len += 1;
                    last_tone_char = 0;
                    tone_cancelled = true;
                    idx += 1;
                    continue;
                }

                // If tone was previously cancelled, subsequent tone keys are literals
                if tone_cancelled {
                    unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                    t_len += 1;
                    idx += 1;
                    continue;
                }

                // Rule 3: tone key between vowels with trailing consonant → literal.
                // Single branch covers both lookaheads; tone keys are rare so branch is predictable.
                if idx + 2 < len {
                    let next = unsafe { *bytes_all.get_unchecked(idx + 1) };
                    let next_attr = unsafe { *self.mode.classify.get_unchecked(next as usize) };
                    if (next_attr & IS_VOWEL) != 0 {
                        let after_next = unsafe { *bytes_all.get_unchecked(idx + 2) };
                        let after_next_attr = unsafe { *self.mode.classify.get_unchecked(after_next as usize) };
                        if (after_next_attr & IS_VOWEL) == 0 {
                            unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                            t_len += 1;
                            idx += 1;
                            continue;
                        }
                    }
                }

                last_tone_char = b;
            } else {
                // Fused toggling: detect triple-repeat
                if b == run_char {
                    run_count += 1;
                    if run_count == 3 && matches!(b, b'a' | b'e' | b'o' | b'd') {
                        t_len -= 1;
                        run_count = 1;
                        idx += 1;
                        continue;
                    }
                } else {
                    run_char = b;
                    run_count = 1;
                }

                let is_adjacent = b == run_char && run_count == 2;
                match b {
                    b'a' => { let bit = 1u8 << 0; if seen_mod & bit != 0 && !is_adjacent { need_mod_bubble = true; } seen_mod |= bit; }
                    b'e' => { let bit = 1u8 << 1; if seen_mod & bit != 0 && !is_adjacent { need_mod_bubble = true; } seen_mod |= bit; }
                    b'o' => { let bit = 1u8 << 2; if seen_mod & bit != 0 && !is_adjacent { need_mod_bubble = true; } seen_mod |= bit; }
                    b'd' => { let bit = 1u8 << 3; if seen_mod & bit != 0 && !is_adjacent { need_mod_bubble = true; } seen_mod |= bit; }
                    b'w' => { has_w = true; }
                    _ => {}
                }

                unsafe { *toggled.get_unchecked_mut(t_len) = b; }
                t_len += 1;
            }
            idx += 1;
        }

        // Fused modifier + w bubbling pass
        const W_LITERAL: u8 = 0x01;
        let need_w_pass = has_w && self.mode.enable_w_bubbling;
        {
            if need_mod_bubble || need_w_pass {
                let mut buf = [0u8; 40];
                let mut b_len = 0usize;

                // Phase 1: modifier bubbling + double-w collapse
                let mut last_pos: [u8; 4] = [0xFF; 4];
                let mut wi = 0usize;
                while wi < t_len {
                    let c = unsafe { *toggled.get_unchecked(wi) };

                    // Double-w cancellation
                    if c == b'w' && self.mode.enable_w_bubbling {
                        if wi + 1 < t_len {
                            let next_c = unsafe { *toggled.get_unchecked(wi + 1) };
                            if next_c == b'w' {
                                unsafe { *buf.get_unchecked_mut(b_len) = W_LITERAL; }
                                b_len += 1;
                                wi += 2;
                                continue;
                            }
                        }
                        unsafe { *buf.get_unchecked_mut(b_len) = c; }
                        b_len += 1;
                        wi += 1;
                        continue;
                    }

                    // Modifier bubbling for a,e,o,d
                    let slot = match c {
                        b'a' => Some(0),
                        b'e' => Some(1),
                        b'o' => Some(2),
                        b'd' => Some(3),
                        _ => None,
                    };

                    if let Some(s) = slot {
                        if last_pos[s] != 0xFF {
                            let insert_at = last_pos[s] as usize + 1;
                            buf.copy_within(insert_at..b_len, insert_at + 1);
                            unsafe { *buf.get_unchecked_mut(insert_at) = c; }
                            b_len += 1;
                            last_pos[s] = 0xFF;
                            for p in last_pos.iter_mut() {
                                if *p != 0xFF && *p as usize >= insert_at {
                                    *p += 1;
                                }
                            }
                        } else {
                            last_pos[s] = b_len as u8;
                            unsafe { *buf.get_unchecked_mut(b_len) = c; }
                            b_len += 1;
                        }
                    } else {
                        unsafe { *buf.get_unchecked_mut(b_len) = c; }
                        b_len += 1;
                    }
                    wi += 1;
                }

                // Phase 2: w-bubbling in-place
                if need_w_pass {
                    let mut out = [0u8; 40];
                    let mut o_len = 0usize;
                    let mut last_target_pos: Option<usize> = None;

                    for k in 0..b_len {
                        let c = unsafe { *buf.get_unchecked(k) };
                        if c == b'w' {
                            if let Some(tp) = last_target_pos {
                                let insert_at = tp + 1;
                                if insert_at < o_len {
                                    out.copy_within(insert_at..o_len, insert_at + 1);
                                }
                                unsafe { *out.get_unchecked_mut(insert_at) = b'w'; }
                                o_len += 1;
                                last_target_pos = Some(insert_at);
                            } else {
                                unsafe { *out.get_unchecked_mut(o_len) = b'w'; }
                                o_len += 1;
                            }
                        } else {
                            unsafe { *out.get_unchecked_mut(o_len) = c; }
                            o_len += 1;
                            if unsafe { *self.mode.w_target.get_unchecked(c as usize) } {
                                last_target_pos = Some(o_len - 1);
                            }
                        }
                    }
                    toggled = out;
                    t_len = o_len;
                } else {
                    toggled = buf;
                    t_len = b_len;
                }
            }
        }

        // Resolve mode rules & Build Char Buffer
        // Pad toggled with sentinel so resolver loop needs no Option/branching.
        unsafe { *toggled.get_unchecked_mut(t_len) = 0; }

        let mut char_buf = ['\0'; 32];
        let mut c_len = 0usize;
        let mut vowel_mask = 0u16;

        let mut i = 0usize;
        while i < t_len {
            let curr = unsafe { *toggled.get_unchecked(i) };

            // W_LITERAL sentinel: output literal 'w', skip resolver
            if curr == W_LITERAL {
                char_buf[c_len] = 'w';
                c_len += 1;
                i += 1;
                continue;
            }

            // SAFETY: toggled is padded with sentinel at t_len, so toggled[i+1] is always valid.
            let next = unsafe { *toggled.get_unchecked(i + 1) };

            // Static dispatch: compiler inlines the specific resolver per enum arm.
            let (mut c, consumed) = match self.mode.resolver {
                ResolverKind::Telex => TelexMode::resolve(curr, next),
                ResolverKind::Vni => VniMode::resolve(curr, next),
            };

            // uow -> ươ
            if curr == b'u' && !consumed {
                if next == b'o' {
                    if i + 2 < t_len && unsafe { *toggled.get_unchecked(i + 2) } == b'w' {
                        let is_qu = if i > 0 {
                            let prev = unsafe { *toggled.get_unchecked(i - 1) };
                            prev == b'q' || prev == b'Q'
                        } else {
                            false
                        };
                        if !is_qu {
                            c = 'ư';
                        }
                    }
                }
            }

            if is_vowel_unicode(c) {
                vowel_mask |= 1 << c_len;
            }

            char_buf[c_len] = c;
            c_len += 1;
            i += if consumed { 2 } else { 1 };
        }

        // If no vowels in resolved output and tone keys were stripped, fall back to raw
        if vowel_mask == 0 && last_tone_char != 0 && !tone_cancelled {
            let has_modified = char_buf[..c_len].iter().any(|&c| !c.is_ascii());
            if !has_modified {
                self.out_buffer.clear();
                let _ = self.out_buffer.push_str(&self.raw_buffer);
                return &self.out_buffer;
            }
        }

        // Validation
        if self.is_invalid_vietnamese_chars(&char_buf[..c_len], vowel_mask) {
            self.out_buffer.clear();
            let _ = self.out_buffer.push_str(&self.raw_buffer);
            return &self.out_buffer;
        }

        // Tone Placement
        if last_tone_char > 0 {
            let tone_id = unsafe { *self.mode.tone.get_unchecked(last_tone_char as usize) };
            self.apply_tone_in_place(&mut char_buf[..c_len], vowel_mask, tone_id);
        }

        self.out_buffer.clear();
        for &c in &char_buf[..c_len] {
            let _ = self.out_buffer.push(c);
        }

        &self.out_buffer
    }

    fn is_invalid_vietnamese_chars(&self, chars: &[char], vowel_mask: u16) -> bool {
        if vowel_mask == 0 {
            return chars.len() > 1;
        }

        let len = chars.len();

        // Check "ou" adjacency
        let mut mask_o: u32 = 0;
        let mut mask_u: u32 = 0;
        let mut idx: u32 = 0;
        for &c in chars.iter() {
            if idx >= 32 {
                break;
            }
            if c == 'o' {
                mask_o |= 1u32 << idx;
            } else if c == 'u' {
                mask_u |= 1u32 << idx;
            }
            idx += 1;
        }
        if (mask_o & (mask_u >> 1)) != 0 {
            return true;
        }

        // Check leading consonant cluster (onset)
        let first_vowel_pos = vowel_mask.trailing_zeros() as usize;

        if first_vowel_pos >= 3 {
            if first_vowel_pos == 3 {
                if len >= 3 && chars[0] == 'n' && chars[1] == 'g' && chars[2] == 'h' {
                    // ngh- is valid
                } else {
                    return true;
                }
            } else {
                return true;
            }
        }

        if first_vowel_pos == 2 {
            let c1 = chars[0];
            let c2 = chars[1];
            if c1.is_ascii_lowercase() && c2.is_ascii_lowercase() {
                let b1 = c1 as u32 - b'a' as u32;
                let b2 = c2 as u32 - b'a' as u32;
                let table_idx = b1 as usize * 26 + b2 as usize;
                if INVALID_PAIR_TABLE[table_idx] {
                    return true;
                }
            }
        }

        // Check mid-word consonant clusters: between any two vowels, at most 2
        // consecutive consonants are allowed (coda of prev syllable + onset of next).
        // e.g. "êlctronic" has 4 consonants between ê and o → invalid.
        {
            let mut consec_consonants = 0u8;
            let mut seen_vowel = false;
            for i in 0..len {
                let is_v = (vowel_mask >> i) & 1 == 1;
                if is_v {
                    consec_consonants = 0;
                    seen_vowel = true;
                } else if seen_vowel {
                    consec_consonants += 1;
                    if consec_consonants > 2 {
                        return true;
                    }
                }
            }
        }

        // Check trailing consonant cluster (coda)
        // Vietnamese allows at most: c, ch, m, n, ng, nh, p, t after the vowel cluster
        let last_vowel_pos = 15 - (vowel_mask.reverse_bits().trailing_zeros() as usize);
        let trailing_len = len - 1 - last_vowel_pos;

        if trailing_len == 2 {
            let c1 = chars[last_vowel_pos + 1];
            let c2 = chars[last_vowel_pos + 2];
            // Valid 2-char codas: ch, ng, nh
            if !((c1 == 'c' && c2 == 'h')
                || (c1 == 'n' && c2 == 'g')
                || (c1 == 'n' && c2 == 'h'))
            {
                return true;
            }
        }

        false
    }

    fn apply_tone_in_place(&self, chars: &mut [char], mask: u16, tone: u8) {
        let count = mask.count_ones();
        if count == 0 {
            return;
        }

        let target_pos = match count {
            1 => mask.trailing_zeros() as usize,
            2 => {
                let first = mask.trailing_zeros() as usize;
                let second = (mask & !(1 << first)).trailing_zeros() as usize;

                let f = chars.get(first).copied().unwrap_or('\0');
                let sc = chars.get(second).copied().unwrap_or('\0');

                // Special case: ui/ưi (e.g. "túi", "gửi") place tone on the first vowel.
                // Exception: in "qu" prefix, 'u' is a glide, so tone belongs to the following vowel.
                let mut prefer_first = (f == 'u' || f == 'ư') && sc == 'i';

                // Modified/circumflex vowels paired with a plain vowel: tone on the modified vowel.
                // e.g. ơi(mới), ôi(tối), êu(nếu), âu(đầu), ây(đấy), âo(cháo/nấo)
                // Exception: ươ pair — tone goes on ơ (second), not ư.
                let f_is_modified = matches!(f, 'ơ' | 'ô' | 'ê' | 'â' | 'ă');
                let sc_is_plain = matches!(sc, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
                if f_is_modified && sc_is_plain {
                    prefer_first = true;
                }

                // Standard open pairs that often prefer tone on the first vowel.
                let mut is_open_pair = (f == 'i' && (sc == 'a' || sc == 'u'))
                    || (f == 'u' && (sc == 'a' || sc == 'e'))
                    || (f == 'ư' && (sc == 'a' || sc == 'u'))
                    || (f == 'a'
                        && (sc == 'o' || sc == 'e' || sc == 'i' || sc == 'u' || sc == 'y'))
                    || (f == 'e' && (sc == 'o' || sc == 'u'))
                    || (f == 'o' && sc == 'i')
                    || (f == 'â' && (sc == 'y' || sc == 'u'));

                // Exception: "qu" and "gi" logic
                if chars.len() >= 2 {
                    let p0 = chars[0];
                    let p1 = chars[1];

                    if (p0 == 'q' || p0 == 'Q') && (p1 == 'u' || p1 == 'U') && first == 1 {
                        is_open_pair = false;
                        prefer_first = false;
                    } else if (p0 == 'g' || p0 == 'G') && (p1 == 'i' || p1 == 'I') && first == 1 {
                        is_open_pair = false;
                        prefer_first = false;
                    }
                }

                if prefer_first {
                    first
                } else if is_open_pair {
                    let has_coda = (second + 1) < chars.len();
                    if has_coda { second } else { first }
                } else {
                    second
                }
            }
            _ => (mask & !(1 << mask.trailing_zeros())).trailing_zeros() as usize,
        };

        if let Some(target) = chars.get_mut(target_pos) {
            *target = map_vowel_with_tone(*target, tone);
        }
    }
}
