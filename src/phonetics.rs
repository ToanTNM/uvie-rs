/// Invalid Vietnamese consonant pairs (onset blacklist).
/// Any 2-char onset not in this table is allowed to pass (for English passthrough).
/// Index = (c1 - b'a') * 26 + (c2 - b'a').
static INVALID_PAIR_TABLE: [bool; 676] = {
    let mut t = [false; 676];
    macro_rules! mark {
        ($a:expr, $b:expr) => {
            t[($a - b'a') as usize * 26 + ($b - b'a') as usize] = true;
        };
    }
    // Clusters that do NOT exist as Vietnamese onsets
    mark!(b'c', b'l'); mark!(b'f', b'l'); mark!(b'b', b'l'); mark!(b'g', b'l');
    mark!(b's', b'l'); mark!(b'p', b'l');
    mark!(b'b', b'r'); mark!(b'b', b'h'); mark!(b'p', b'r'); mark!(b'd', b'r'); mark!(b'f', b'r');
    mark!(b'g', b'r'); mark!(b'k', b'r');
    mark!(b's', b't'); mark!(b's', b'p'); mark!(b's', b'k');
    mark!(b'p', b't'); mark!(b'p', b'c'); mark!(b'p', b'g'); mark!(b'p', b'q');
    mark!(b'p', b's'); mark!(b'p', b'k'); mark!(b'p', b'd'); mark!(b'p', b'f');
    mark!(b'p', b'b');
    // Additional invalid pairs from OpenKey consonant analysis:
    mark!(b'c', b'r'); mark!(b'd', b'j'); mark!(b'd', b'w');
    mark!(b'f', b'f'); mark!(b'f', b's'); mark!(b'f', b't');
    mark!(b'g', b'g'); mark!(b'g', b'j'); mark!(b'g', b'w');
    mark!(b'h', b'h'); mark!(b'h', b'j'); mark!(b'h', b'w');
    mark!(b'j', b'j'); mark!(b'j', b'r');
    mark!(b'k', b'k'); mark!(b'k', b'j'); mark!(b'k', b'w');
    mark!(b'l', b'l'); mark!(b'l', b'j'); mark!(b'l', b'w');
    mark!(b'm', b'm'); mark!(b'm', b'j'); mark!(b'm', b'r'); mark!(b'm', b'w');
    mark!(b'n', b'n'); mark!(b'n', b'j'); mark!(b'n', b'w');
    mark!(b'r', b'r'); mark!(b'r', b'j'); mark!(b'r', b'w');
    mark!(b's', b's'); mark!(b's', b'j'); mark!(b's', b'w');
    mark!(b't', b't'); mark!(b't', b'j'); mark!(b't', b'w');
    mark!(b'v', b'v'); mark!(b'v', b'j'); mark!(b'v', b'w');
    mark!(b'x', b'x'); mark!(b'x', b'j'); mark!(b'x', b'w');
    mark!(b'z', b'z'); mark!(b'z', b'j'); mark!(b'z', b'w');
    mark!(b'q', b'q'); mark!(b'q', b'j'); mark!(b'q', b'r'); mark!(b'q', b't');
    mark!(b'q', b'c'); mark!(b'q', b'l'); mark!(b'q', b'b'); mark!(b'q', b'f');
    t
};

/// Valid Vietnamese nuclei (vowel clusters after resolver).
/// Single vowels are implicitly valid, so this only lists clusters of length >= 2.
const VALID_NUCLEI: &[[char; 2]] = &[
    ['a', 'i'], ['a', 'o'], ['a', 'u'], ['a', 'y'],
    ['e', 'o'], ['e', 'u'], ['i', 'a'], ['i', 'e'], ['i', 'o'], ['i', 'u'],
    ['o', 'a'], ['o', 'e'], ['o', 'i'], ['o', 'o'],
    ['u', 'a'], ['u', 'i'], ['u', 'o'], ['u', 'u'],
    ['u', 'y'], ['y', 'a'], ['y', 'e'],
    // Modified vowel combinations
    ['â', 'u'], ['â', 'y'], ['ê', 'u'],
    ['ă', 'y'],
    ['ơ', 'i'], ['ô', 'i'],
    ['ư', 'a'], ['ư', 'i'], ['ư', 'o'], ['ư', 'u'],
    ['y', 'ê'],
];

const VALID_NUCLEI_3: &[[char; 3]] = &[
    ['i', 'ê', 'u'],
    ['o', 'a', 'i'], ['o', 'a', 'y'],
    ['u', 'y', 'a'], ['u', 'y', 'u'],
    ['ư', 'ơ', 'i'], ['ư', 'ơ', 'u'],
    ['y', 'ê', 'u'],
];

/// Check if the leading consonant cluster (before the first vowel) is a valid onset.
/// Uses a blacklist for 2-char onsets (English passthrough friendly) and only
/// allows `ngh` for 3-char onsets.
#[inline(always)]
pub fn is_valid_onset(chars: &[char], first_vowel_pos: usize) -> bool {
    if first_vowel_pos == 0 {
        return true; // no onset
    }
    if first_vowel_pos == 1 {
        return true; // any single char is accepted (English passthrough)
    }
    if first_vowel_pos == 2 {
        let c1 = chars[0];
        let c2 = chars[1];
        if c1.is_ascii_lowercase() && c2.is_ascii_lowercase() {
            let b1 = c1 as u32 - b'a' as u32;
            let b2 = c2 as u32 - b'a' as u32;
            let table_idx = b1 as usize * 26 + b2 as usize;
            if INVALID_PAIR_TABLE[table_idx] {
                return false;
            }
        }
        return true;
    }
    if first_vowel_pos == 3 {
        // Only "ngh" is a valid 3-char onset in Vietnamese
        return chars[0] == 'n' && chars[1] == 'g' && chars[2] == 'h';
    }
    false // >3 chars before first vowel
}

/// Check if the vowel cluster between first_vowel and last_vowel is a valid nucleus.
/// Single-vowel nuclei are always valid.
#[inline(always)]
pub fn is_valid_nucleus(chars: &[char], first_vowel_pos: usize, last_vowel_pos: usize) -> bool {
    let cluster_len = last_vowel_pos - first_vowel_pos + 1;
    if cluster_len == 1 {
        return true;
    }

    if cluster_len == 2 {
        let c0 = chars[first_vowel_pos];
        let c1 = chars[first_vowel_pos + 1];
        for &[n0, n1] in VALID_NUCLEI.iter() {
            if c0 == n0 && c1 == n1 {
                return true;
            }
        }
        return false;
    }

    if cluster_len == 3 {
        let c0 = chars[first_vowel_pos];
        let c1 = chars[first_vowel_pos + 1];
        let c2 = chars[first_vowel_pos + 2];
        for &[n0, n1, n2] in VALID_NUCLEI_3.iter() {
            if c0 == n0 && c1 == n1 && c2 == n2 {
                return true;
            }
        }
        return false;
    }

    // No valid nucleus longer than 3
    false
}

/// Check trailing consonant cluster (coda) validity.
/// Returns `true` if the coda is invalid.
/// NOTE: We only reject 2-char invalid codas (e.g. "ck", "gh") because
/// single-char trailing consonants may be unfinished typing or English words.
#[inline(always)]
pub fn is_invalid_coda(chars: &[char], last_vowel_pos: usize) -> bool {
    let len = chars.len();
    if last_vowel_pos + 2 >= len {
        return false; // no 2-char coda to validate
    }
    let trailing_len = len - 1 - last_vowel_pos;

    if trailing_len == 2 {
        let c1 = chars[last_vowel_pos + 1];
        let c2 = chars[last_vowel_pos + 2];
        !((c1 == 'c' && c2 == 'h')
            || (c1 == 'n' && c2 == 'g')
            || (c1 == 'n' && c2 == 'h'))
    } else {
        trailing_len > 2 // >2 chars after last vowel
    }
}

/// Check if tone is restricted for the given coda.
/// In Vietnamese orthography, codas `ch` and `t` cannot combine with
/// tones hỏi (3) and ngã (4).
#[inline(always)]
pub fn is_tone_restricted(chars: &[char], last_vowel_pos: usize, tone_id: u8) -> bool {
    if tone_id != 3 && tone_id != 4 {
        return false;
    }
    let len = chars.len();
    if last_vowel_pos + 1 >= len {
        return false;
    }
    let trailing_len = len - 1 - last_vowel_pos;

    if trailing_len == 1 {
        chars[last_vowel_pos + 1] == 't'
    } else if trailing_len == 2 {
        let c1 = chars[last_vowel_pos + 1];
        let c2 = chars[last_vowel_pos + 2];
        c1 == 'c' && c2 == 'h'
    } else {
        false
    }
}
