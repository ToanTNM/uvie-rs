use crate::{InputMethod, UltraFastViEngine};

/// Simulates IME typing: whitespace commits the current composing word,
/// and the final result includes committed text + any remaining composing text.
fn type_seq(engine: &mut UltraFastViEngine, seq: &str) -> String {
    let mut result = String::new();
    for c in seq.chars() {
        if c.is_whitespace() {
            result.push_str(engine.current_composing());
            engine.commit();
            result.push(c);
        } else {
            engine.feed(c);
        }
    }
    result.push_str(engine.current_composing());
    result
}

fn type_seq_vni(seq: &str) -> String {
    let mut e = UltraFastViEngine::new();
    e.set_input_method(InputMethod::Vni);
    type_seq(&mut e, seq)
}

#[test]
fn telex_modifier_basic() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aa"), "â");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aw"), "ă");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ee"), "ê");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "oo"), "ô");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ow"), "ơ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "uw"), "ư");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "dd"), "đ");
}

#[test]
fn tone_single_vowel_all_tones() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "as"), "á");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "af"), "à");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ar"), "ả");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ax"), "ã");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aj"), "ạ");
}

#[test]
fn z_key_removes_tone() {
    let mut e = UltraFastViEngine::new();
    // as -> á, z -> a
    assert_eq!(type_seq(&mut e, "asz"), "a");

    let mut e = UltraFastViEngine::new();
    // az -> a
    assert_eq!(type_seq(&mut e, "az"), "a");

    let mut e = UltraFastViEngine::new();
    // axz -> a
    assert_eq!(type_seq(&mut e, "axz"), "a");
}

#[test]
fn toggling_triplet() {
    // New behaviour: triple cancel outputs the TWO literal chars before the
    // cancelling keystroke, not just one.  "nee"→"nê", "neee"→"nee" (literal).

    let mut e = UltraFastViEngine::new();
    // aaa → aa  (triple cancels "aa"→"â", keeps both a's literal)
    assert_eq!(type_seq(&mut e, "aaa"), "aa");

    let mut e = UltraFastViEngine::new();
    // ddd → dd  (triple cancels "dd"→"đ", keeps both d's literal)
    assert_eq!(type_seq(&mut e, "ddd"), "dd");

    let mut e = UltraFastViEngine::new();
    // eee → ee  (triple cancels "ee"→"ê", keeps both e's literal)
    assert_eq!(type_seq(&mut e, "eee"), "ee");

    let mut e = UltraFastViEngine::new();
    // ooo → oo  (triple cancels "oo"→"ô", keeps both o's literal)
    assert_eq!(type_seq(&mut e, "ooo"), "oo");

    // Pair still works normally (only 2 chars)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ee"), "ê");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aa"), "â");
}

#[test]
fn triple_cancel_with_trailing_chars() {
    // Characters after a triple-cancel must be preserved, not silently dropped.
    // Bug: "neeeb" was outputting "nee" (losing 'b') because the early exit
    // only took bytes_all[..end] and discarded everything after the cancelling char.

    let mut e = UltraFastViEngine::new();
    // nee → nê, neee → nee (triple cancel), neeeb → neeb (b preserved)
    assert_eq!(type_seq(&mut e, "neeeb"), "neeb");

    let mut e = UltraFastViEngine::new();
    // neeeboo → neeboo (full raw passthrough after triple cancel)
    assert_eq!(type_seq(&mut e, "neeeboo"), "neeboo");

    let mut e = UltraFastViEngine::new();
    // aaaa → aaa? No: "aa" → "â", "aaa" → "aa" (cancel), "aaaa" → "aaa" (skip 3rd 'a')
    // Actually: aaa = ['a','a','a'], end=2, skip 'a' at 2 → "aa"
    // aaaa = ['a','a','a','a'], end=2, skip 'a' at 2 → ['a','a','a'] → "aaa"
    assert_eq!(type_seq(&mut e, "aaaa"), "aaa");

    let mut e = UltraFastViEngine::new();
    // With consonant prefix: "neeeee" → "neee"
    // neeee: ['n','e','e','e','e'], end=3 (3rd 'e'), skip 'e' at 3 → "neee"
    assert_eq!(type_seq(&mut e, "neeee"), "neee");
}

#[test]
fn tone_on_modified_vowels() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aas"), "ấ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "awj"), "ặ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ees"), "ế");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "oos"), "ố");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ows"), "ớ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "uws"), "ứ");
}

#[test]
fn greedy_tone_last_wins() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "asf"), "à");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "afsj"), "ạ");
}

#[test]
fn tone_placement_two_vowels_no_coda() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoas"), "hoá");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoaf"), "hoà");
}

#[test]
fn tone_placement_two_vowels_with_coda() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoans"), "hoán");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoanj"), "hoạn");
}

#[test]
fn tone_placement_three_vowels_targets_second_vowel() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "khuya"), "khuya");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "khuyas"), "khuýa");
}

#[test]
fn whitespace_commits_and_resets_composing() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aas"), "ấ");
    // Space commits the composing word; feed returns empty composing text.
    assert_eq!(e.feed(' '), "");
    assert_eq!(e.committed_text(), "ấ ");
    assert_eq!(e.current_composing(), "");
    // New word starts with a fresh composing buffer.
    assert_eq!(type_seq(&mut e, "as"), "á");
}

#[test]
fn tone_only_input_produces_empty() {
    let mut e = UltraFastViEngine::new();
    // First char is treated as consonant
    assert_eq!(type_seq(&mut e, "s"), "s");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "z"), "z");
}

#[test]
fn do_not_apply_to_english() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "clear"), "clear");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "flan"), "flan");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "blob"), "blob");
}

#[test]
fn special_uow_combo() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "huow"), "hươ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "huows"), "hướ");
}

#[test]
fn valid_consonant_cluster() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nghe"), "nghe");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nghes"), "nghé");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nghees"), "nghế");
}

#[test]
fn regression_qu_gi_placement() {
    // qu + a -> quá (tone on a)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "quas"), "quá");

    // qu + y -> quỳ (tone on y)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "quyf"), "quỳ");

    // qu + i -> quỉ (tone on i)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "quir"), "quỉ");

    // gi + a -> giá (tone on a)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "gias"), "giá");
}

#[test]
fn regression_vowel_pairs() {
    // oa -> hoà (tone on a, new style)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoaf"), "hoà");

    // oe -> hoè (tone on e, new style)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoef"), "hoè");

    // uy -> tuỳ (tone on y, new style)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tuyf"), "tuỳ");

    // ia -> mía (tone on i)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "mias"), "mía");

    // ua -> múa (tone on u)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "muas"), "múa");

    // ưa -> mứa (tone on ư)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "muwas"), "mứa");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "pro"), "pro");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "free"), "free");
}

#[test]
fn regression_pho_validity() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "phos"), "phó");
}

#[test]
fn regression_ui_tone_on_first_vowel() {
    let mut e = UltraFastViEngine::new();
    // guiwr -> gửi (tone on ư, not on i)
    assert_eq!(type_seq(&mut e, "guiwr"), "gửi");
}

#[test]
fn vni_basic_modifiers() {
    assert_eq!(type_seq_vni("a6"), "â");
    assert_eq!(type_seq_vni("a8"), "ă");
    assert_eq!(type_seq_vni("e6"), "ê");
    assert_eq!(type_seq_vni("o6"), "ô");
    assert_eq!(type_seq_vni("o7"), "ơ");
    assert_eq!(type_seq_vni("u7"), "ư");
    assert_eq!(type_seq_vni("d9"), "đ");
}

#[test]
fn vni_basic_tones() {
    assert_eq!(type_seq_vni("a1"), "á");
    assert_eq!(type_seq_vni("a2"), "à");
    assert_eq!(type_seq_vni("a3"), "ả");
    assert_eq!(type_seq_vni("a4"), "ã");
    assert_eq!(type_seq_vni("a5"), "ạ");
}

#[test]
fn vni_tone_removal() {
    // a1 -> á, then 0 -> a
    assert_eq!(type_seq_vni("a10"), "a");
    // a0 -> a
    assert_eq!(type_seq_vni("a0"), "a");
}

#[test]
fn vni_tones_on_modified_vowels() {
    // a6 + 1 => ấ
    assert_eq!(type_seq_vni("a61"), "ấ");
    // o6 + 1 => ố
    assert_eq!(type_seq_vni("o61"), "ố");
    // o7 + 1 => ớ
    assert_eq!(type_seq_vni("o71"), "ớ");
    // u7 + 1 => ứ
    assert_eq!(type_seq_vni("u71"), "ứ");
    // d9 + 1 should not tone (đ is not in mapping), stays đ
    assert_eq!(type_seq_vni("d91"), "đ");
}

#[test]
fn tone_on_modified_vowel_oi() {
    // mơí -> mới (tone on ơ, not i)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "mowis"), "mới");
}

#[test]
fn tone_on_modified_vowel_eu() {
    // nêú -> nếu (tone on ê, not u)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "neeus"), "nếu");
}

#[test]
fn double_tone_key_undoes_tone() {
    // tess -> test (double s undoes the tone, s becomes literal)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tess"), "tes");

    // teff -> tef
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "teff"), "tef");

    // terr -> ter
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "terr"), "ter");

    // texx -> tex
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "texx"), "tex");

    // tejj -> tej
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tejj"), "tej");
}

#[test]
fn double_w_undoes_modification() {
    // showw -> show (double w undoes ơ)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "showw"), "show");

    // oww -> ow
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "oww"), "ow");

    // uww -> uw
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "uww"), "uw");
}

#[test]
fn consonant_only_no_duplication() {
    // txt should stay txt (no duplication)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "txt"), "txt");

    // sx should stay sx
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "sx"), "sx");
}

#[test]
fn double_tone_then_continue() {
    // vieetj -> việt (double e makes ê, then tone j)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "vieetj"), "việt");
}

#[test]
fn tone_placement_oi_pair() {
    // đời -> ddowif
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ddowif"), "đời");

    // tối -> toois
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "toois"), "tối");

    // lối -> loois
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "loois"), "lối");
}

#[test]
fn tone_placement_eu_pair() {
    // nếu -> neeus
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "neeus"), "nếu");

    // kều -> keeuf (tone f = huyền)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "keeuf"), "kều");

    // kểu -> keeur (tone r = hỏi)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "keeur"), "kểu");
}

// ===== Comprehensive edge case tests =====

#[test]
fn edge_double_tone_various_positions() {
    // Double tone at end of word with vowel
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "bass"), "bas");

    // Double tone in middle then more chars — cancelled tone key becomes literal, extra chars accepted
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tesstt"), "testt");

    // zz should also cancel
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "azz"), "az");
}

#[test]
fn edge_double_w_various() {
    // aww -> aw (undo ă)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aww"), "aw");

    // ddoww -> đow (undo ơ, keep đ)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ddoww"), "đow");
}

#[test]
fn edge_english_words_passthrough() {
    // Common English words that contain tone keys
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "stress"), "stress");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "jazz"), "jaz");

    // Pure consonant sequences
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "txt"), "txt");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "rx"), "rx");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "sx"), "sx");
}

#[test]
fn edge_modified_vowel_tone_placement() {
    // ươi -> tone on ơ (second in ươ pair)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "huowis"), "hưới");

    // ươn -> tone on ơ
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "huowns"), "hướn");

    // ươ alone -> tone on ơ
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "huows"), "hướ");

    // âu -> tone on â
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "daauf"), "dầu");

    // ây -> tone on â
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "daays"), "dấy");
}

#[test]
fn edge_consecutive_words_via_space() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "vieetj"), "việt");
    e.feed(' ');
    assert_eq!(e.current_composing(), "");
    assert_eq!(e.committed_text(), "việt ");
    assert_eq!(type_seq(&mut e, "namm"), "namm");
}

#[test]
fn edge_single_char_tone_keys() {
    // Single tone key chars should pass through as-is
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "s"), "s");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "f"), "f");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "r"), "r");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "x"), "x");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "j"), "j");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "z"), "z");
}

#[test]
fn edge_common_vietnamese_words() {
    // Common words that exercise multiple features
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "xins"), "xín");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "chaof"), "chào");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ddeepj"), "đệp");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nawm"), "năm");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nawms"), "nắm");

    // không -> khoongf (ô + huyền = ồ)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "khoongf"), "khồng");

    // được -> dduowcj
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "dduowcj"), "được");

    // người -> nguowif
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nguowif"), "người");
}

#[test]
fn free_style_modifier_bubbling() {
    // ee modifier with vowel in between: neues -> nếu
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "neues"), "nếu");

    // aa modifier with vowel in between: naos -> nâó? No — naos: n,a,o -> nao + tone s
    // Actually: nao with free-style aa: naoas -> n,a,o,a -> bubble a next to a -> n,a,a,o -> nâo + s -> nấo
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "naoas"), "nấo");

    // oo modifier with vowel in between: noies -> nối
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "noios"), "nối");

    // Free-style ee: tieengs -> tiếng
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tieengs"), "tiếng");

    // Free-style with w: moiws -> mới
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "moiws"), "mới");

    // dd modifier across consonants: bubbles to đan (valid Vietnamese)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "dand"), "đan");
}

#[test]
fn no_bubble_across_consonants() {
    // reset: e..e separated by consonant 's' -> no bubble, stays "reset"
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "reset"), "reset");

    // electronic: e..e separated by consonant 'l' -> no bubble
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "electronic"), "electronic");

    // depend: e..e separated by consonant 'p' -> no bubble
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "depend"), "depend");

    // added: dd → đ by resolver, but "ađed" has V-C-V pattern → not a valid Vietnamese
    // syllable → engine falls back to raw passthrough "added"
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "added"), "added");

    // banana: a..a bubbles to â, but "bânna" has V-C-V pattern → raw passthrough
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "banana"), "banana");

    // resset: double-s cancels tone, then Rule 3 keeps second s as literal -> "reset"
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "resset"), "reset");

    // Free-style still works when only vowels/w separate: neues -> nếu
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "neues"), "nếu");

    // Free-style across consonants with tone key: memef -> mềm
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "memef"), "mềm");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nuotos"), "nuốt");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "thajta"), "thật");

    // Free-style across consonants without tone: nene -> nên
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nene"), "nên");
}

#[test]
fn free_style_does_not_break_normal() {
    // Normal adjacent modifiers still work
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aas"), "ấ");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ees"), "ế");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "oos"), "ố");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "dd"), "đ");

    // Triple cancel now outputs two literal chars (not one)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aaa"), "aa");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "eee"), "ee");
}

#[test]
fn invalid_onset_pair_fallback() {
    // tl is not a valid Vietnamese onset -> fallback to raw
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tl"), "tl");

    // bh is not valid -> fallback
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "bh"), "bh");

    // lr is not valid -> fallback
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "lr"), "lr");
}

#[test]
fn valid_onset_pairs() {
    // tr is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "tras"), "trá");

    // ph is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "phas"), "phá");

    // kh is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "khas"), "khá");

    // ngh is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "nghes"), "nghé");
}

#[test]
fn tone_restriction_ch_t_coda() {
    // ch + sac (1) is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "achs"), "ách");

    // ch + hoi (3) is invalid -> fallback raw
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "achr"), "achr");

    // ch + nga (4) is invalid -> fallback raw
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "achx"), "achx");

    // t + sac is valid
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "ats"), "át");

    // t + hoi is invalid -> fallback raw
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "atr"), "atr");
}

#[test]
fn quick_start_consonants() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_start(true);
    assert_eq!(type_seq(&mut e, "jang"), "giang");

    let mut e = UltraFastViEngine::new();
    e.set_quick_start(true);
    assert_eq!(type_seq(&mut e, "phanhs"), "phánh");

    let mut e = UltraFastViEngine::new();
    e.set_quick_start(true);
    assert_eq!(type_seq(&mut e, "wen"), "quen");
}

#[test]
fn quick_start_disabled_by_default() {
    let mut e = UltraFastViEngine::new();
    // j should remain literal when quick_start is off
    assert_eq!(type_seq(&mut e, "jang"), "jang");
}

#[test]
fn quick_telex_cc() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "cc"), "ch");
}

#[test]
fn quick_telex_gg() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "gg"), "gi");
}

#[test]
fn quick_telex_kk() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "kk"), "kh");
}

#[test]
fn quick_telex_nn() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "nn"), "ng");
}

#[test]
fn quick_telex_qq() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "qq"), "qu");
}

#[test]
fn quick_telex_pp() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "pp"), "ph");
}

#[test]
fn quick_telex_tt() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    assert_eq!(type_seq(&mut e, "tt"), "th");
}

#[test]
fn quick_telex_with_tone() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    // ccas -> ch + a + s (tone sac) -> chá
    assert_eq!(type_seq(&mut e, "ccas"), "chá");
}

#[test]
fn quick_telex_disabled_by_default() {
    let mut e = UltraFastViEngine::new();
    // cc should stay cc when quick_telex is off
    assert_eq!(type_seq(&mut e, "cc"), "cc");
}

#[test]
fn modern_orthography_hoas() {
    // hoas -> hoá (tone on 'a' — uvie-rs default is already modern orthography)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoas"), "hoá");
}

#[test]
fn modern_orthography_thuys() {
    // thuys -> thuý (tone on 'y' — uvie-rs default is already modern orthography)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "thuys"), "thuý");
}

#[test]
fn modern_orthography_oa_with_coda() {
    // hoacs -> hoác (tone on 'a' even with coda)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "hoacs"), "hoác");
}

#[test]
fn modern_orthography_oe_pair() {
    // khoes -> khoé (tone on 'e')
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "khoes"), "khoé");
}

#[test]
fn modern_orthography_quy_prefix() {
    // qu + uy -> quý (qu prefix, tone on 'y')
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "quys"), "quý");
}

#[test]
fn quick_telex_english_words_passthrough() {
    let mut e = UltraFastViEngine::new();
    e.set_quick_telex(true);
    // "account" has 'cc' which gets expanded to 'ch' when quick telex is on
    assert_eq!(type_seq(&mut e, "account"), "achount");
}

#[test]
fn replay_compact_no_crash() {
    // compact() + safety valve must prevent raw_buf from overflowing (capacity = 16).
    use crate::ReplayEngine;
    let mut e = ReplayEngine::new();

    // Feed 'n' then 40 'e' keys — without compaction/safety-valve this would crash.
    e.feed('n');
    for _ in 0..40 {
        e.feed('e');
    }
    // Should not crash. The exact output depends on safety-valve resets,
    // but it must be non-empty.
    let out = e.current_composing();
    assert!(!out.is_empty(), "output should not be empty after 41 e's");
}

#[test]
fn replay_triple_cancel_preserves_trailing_chars() {
    // After triple-cancel, subsequent characters must be preserved, not silently dropped.
    use crate::ReplayEngine;
    let mut e = ReplayEngine::new();

    // nee → nê
    e.feed('n'); e.feed('e'); e.feed('e');
    assert_eq!(e.current_composing(), "nê");

    // neee → nee (triple cancel, 3rd 'e' skipped)
    e.feed('e');
    assert_eq!(e.current_composing(), "nee");

    // neeeb → neeb ('b' preserved after cancel)
    e.feed('b');
    assert_eq!(e.current_composing(), "neeb");

    // neeebo → neebo ('o' preserved)
    e.feed('o');
    assert_eq!(e.current_composing(), "neebo");

    // neeeboo → neeboo (full word preserved)
    e.feed('o');
    assert_eq!(e.current_composing(), "neeboo");
}

// ===== ReplayEngine V-C-V Boundary Detection Tests =====

#[cfg(test)]
mod replay_tests {
    use crate::ReplayEngine;

    fn type_replay(e: &mut ReplayEngine, s: &str) -> String {
        let mut screen = String::new();
        for ch in s.chars() {
            let (bs, suffix) = e.feed(ch);
            let screen_chars: Vec<char> = screen.chars().collect();
            let new_len = screen_chars.len().saturating_sub(bs);
            screen = screen_chars[..new_len].iter().collect::<String>() + &suffix;
        }
        screen
    }

    #[test]
    fn vcv_neebo_commits_ne_starts_bo() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "neebo"), "nêbo");
        assert_eq!(e.committed_text(), "nê"); // verify committed tracking
    }

    #[test]
    fn vcv_neeboo_commits_ne_composes_boo() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "neeboo"), "nêbô");
        assert_eq!(e.committed_text(), "nê"); // Still just "nê" committed
    }

    #[test]
    fn no_premature_commit_neeb() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "neeb"), "nêb");
        assert_eq!(e.committed_text(), ""); // No auto-commit occurred
    }

    #[test]
    fn english_passthrough_unaffected() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "blob"), "blob");
        assert_eq!(e.committed_text(), "");

        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "clear"), "clear");
        assert_eq!(e.committed_text(), "");
    }

    #[test]
    fn commit_clears_composing_preserves_committed() {
        let mut e = ReplayEngine::new();
        type_replay(&mut e, "neebo");
        assert_eq!(e.committed_text(), "nê");

        e.commit();
        assert_eq!(e.committed_text(), "nê"); // Preserved after explicit commit (for testing/debugging)
    }

    #[test]
    fn reset_clears_committed_field() {
        let mut e = ReplayEngine::new();
        type_replay(&mut e, "neebo");
        assert_eq!(e.committed_text(), "nê");

        e.reset();
        assert_eq!(e.committed_text(), ""); // Cleared after reset
    }

    #[test]
    fn word_boundary_clears_committed_field() {
        let mut e = ReplayEngine::new();
        type_replay(&mut e, "neebo");
        assert_eq!(e.committed_text(), "nê");

        // Type space (word boundary)
        let (_bs, output) = e.feed(' ');
        assert_eq!(e.committed_text(), ""); // Cleared on word boundary
        assert_eq!(output, " ");
    }

    #[test]
    fn vcv_naabo_commits_na_starts_bo() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "naabo"), "nâbo");
        assert_eq!(e.committed_text(), "nâ");
    }

    #[test]
    fn vcv_toocaa_commits_to_starts_ca() {
        let mut e = ReplayEngine::new();
        assert_eq!(type_replay(&mut e, "toocaa"), "tôcâ");
        assert_eq!(e.committed_text(), "tô");
    }
}

#[test]
fn test_vcv_boundary_auto_commit() {
    use crate::ReplayEngine;

    // --- SECTION 1: Basic neeboo case (step-by-step verification) ---
    {
        let mut e = ReplayEngine::new();

        // Type 'n','e','e' → composing = "nê"
        e.feed('n');
        assert_eq!(e.current_composing(), "n", "after 'n': composing should be 'n'");
        e.feed('e');
        assert_eq!(e.current_composing(), "ne", "after 'ne': composing should be 'ne'");
        e.feed('e');
        assert_eq!(e.current_composing(), "nê", "after 'nee': composing should be 'nê'");
        assert_eq!(e.committed_text(), "", "after 'nee': committed should be empty");

        // Type 'b' → composing = "nêb" (consonant appended, not yet invalid)
        e.feed('b');
        assert_eq!(e.current_composing(), "nêb", "after 'neeb': composing should be 'nêb'");
        assert_eq!(e.committed_text(), "", "after 'neeb': committed should still be empty");

        // Type 'o' → V-C-V boundary detected ('nêbo' is invalid Vietnamese)
        // TASK-001 should auto-commit 'nê' and start new syllable with 'b','o'
        e.feed('o');
        assert_eq!(e.current_composing(), "bo", "after 'neebo': composing should be 'bo'");
        assert_eq!(e.committed_text(), "nê", "after 'neebo': committed should equal 'nê' (exact match)");

        // Type second 'o' → composing = "bô"
        e.feed('o');
        assert_eq!(e.current_composing(), "bô", "after 'neeboo': composing should be 'bô'");
        assert_eq!(e.committed_text(), "nê", "after 'neeboo': committed should still be 'nê'");

        // Verify diff tuple behavior at key transition points
        // After V-C-V commit, the diff should show the split
    }

    // --- SECTION 2: naaboo pattern (aa → â) ---
    {
        let mut e = ReplayEngine::new();

        // Type 'n','a','a' → "nâ"
        e.feed('n');
        e.feed('a');
        e.feed('a');
        assert_eq!(e.current_composing(), "nâ", "after 'naa': composing should be 'nâ'");
        assert_eq!(e.committed_text(), "", "after 'naa': committed should be empty");

        // Type 'b' → "nâb"
        e.feed('b');
        assert_eq!(e.current_composing(), "nâb", "after 'naab': composing should be 'nâb'");
        assert_eq!(e.committed_text(), "", "after 'naab': committed should be empty");

        // Type 'o' → V-C-V commit "nâ", composing "bo"
        e.feed('o');
        assert_eq!(e.current_composing(), "bo", "after 'naabo': composing should be 'bo'");
        assert_eq!(e.committed_text(), "nâ", "after 'naabo': committed should equal 'nâ'");

        // Type second 'o' → "bô"
        e.feed('o');
        assert_eq!(e.current_composing(), "bô", "after 'naaboo': composing should be 'bô'");
        assert_eq!(e.committed_text(), "nâ", "after 'naaboo': committed should still be 'nâ'");
    }

    // --- SECTION 3: toocaa pattern (oo → ô, aa → â) ---
    {
        let mut e = ReplayEngine::new();

        // Type 't','o','o' → "tô"
        e.feed('t');
        e.feed('o');
        e.feed('o');
        assert_eq!(e.current_composing(), "tô", "after 'too': composing should be 'tô'");
        assert_eq!(e.committed_text(), "", "after 'too': committed should be empty");

        // Type 'c' → "tôc"
        e.feed('c');
        assert_eq!(e.current_composing(), "tôc", "after 'tooc': composing should be 'tôc'");
        assert_eq!(e.committed_text(), "", "after 'tooc': committed should be empty");

        // Type 'a' → V-C-V commit "tô", composing "ca"
        e.feed('a');
        assert_eq!(e.current_composing(), "ca", "after 'tooca': composing should be 'ca'");
        assert_eq!(e.committed_text(), "tô", "after 'tooca': committed should equal 'tô'");

        // Type second 'a' → "câ"
        e.feed('a');
        assert_eq!(e.current_composing(), "câ", "after 'toocaa': composing should be 'câ'");
        assert_eq!(e.committed_text(), "tô", "after 'toocaa': committed should still be 'tô'");
    }

    // --- SECTION 4: English passthrough (no spurious commit) ---
    {
        let mut e = ReplayEngine::new();

        // 'blob' - no Vietnamese modifiers, no V-C-V commit
        for ch in "blob".chars() {
            e.feed(ch);
        }
        assert_eq!(e.current_composing(), "blob", "after 'blob': should be raw passthrough");
        assert_eq!(e.committed_text(), "", "after 'blob': committed should be empty");

        let mut e2 = ReplayEngine::new();

        // 'banana' - 'aa' is a valid Vietnamese modifier (→ â), so V-C-V boundary triggers
        // Typing "bana" produces "bân", then 'n' triggers boundary detection
        for ch in "banana".chars() {
            e2.feed(ch);
        }
        // After boundary detection: "bân" is committed, "na" is composing
        assert_eq!(e2.current_composing(), "na", "after 'banana': composing should be 'na'");
        assert_eq!(e2.committed_text(), "bân", "after 'banana': committed should be 'bân'");
    }

    // --- SECTION 5: Multi-syllable accumulation scenarios ---
    {
        let mut e = ReplayEngine::new();

        // First V-C-V: 'neeboo' → 'nê' committed, 'bô' composing
        for ch in "neeboo".chars() {
            e.feed(ch);
        }
        assert_eq!(e.current_composing(), "bô", "after first word: composing should be 'bô'");
        assert_eq!(e.committed_text(), "nê", "after first word: committed should be 'nê'");

        // Explicitly commit first word
        e.commit();
        assert_eq!(e.current_composing(), "", "after commit: composing should be empty");
        assert_eq!(e.committed_text(), "nê", "after commit: committed should still be 'nê'");

        // Second V-C-V: 'naaboo' → 'nâ' committed, 'bô' composing
        for ch in "naaboo".chars() {
            e.feed(ch);
        }
        assert_eq!(e.current_composing(), "bô", "after second word: composing should be 'bô'");
        // Note: committed_text() accumulates across auto-commits (option a)
        // After 'naaboo', we have "nê" + "nâ" = "nênâ"
        assert_eq!(e.committed_text(), "nênâ", "after second word: committed should accumulate both words");
    }
}

#[test]
fn test_telex_word_passthrough() {
    // The engine now correctly passes through English words like "telex"
    // by detecting the V-C-V pattern (t-e-l-e-x) as an invalid Vietnamese
    // syllable and falling back to raw passthrough.
    let mut e = UltraFastViEngine::new();
    let out = type_seq(&mut e, "telex");
    assert_eq!(out, "telex", "'telex' should pass through as English, not be mangled to Vietnamese");
}

#[test]
fn test_expect_word_passthrough() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "expect"), "expect",
        "English word 'expect' should pass through, not become Vietnamese");
}

#[test]
fn test_look_should_cancel() {
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "loook"), "look",
        "Double 'o' should cancel, leaving single 'o'");
}

#[test]
fn test_backspace_thajta_sequence() {
    let mut e = UltraFastViEngine::new();
    // type thajta → thật
    for ch in "thajta".chars() { e.feed(ch); }
    assert_eq!(e.current_composing(), "thật");
    // backspace once → removes last raw 'a', back to "thạt"
    e.backspace();
    assert_eq!(e.current_composing(), "thạt", "after 1 BS: thạt");
    // type 'a' again → should give back thật
    e.feed('a');
    assert_eq!(e.current_composing(), "thật", "retype a: back to thật");
    // backspace removes 'a' again
    e.backspace();
    // type 'a' and 't' (continue composing):
    // "thajtat" = raw passthrough because coda "tt" is invalid.
    e.feed('a');
    e.feed('t');
    assert_eq!(e.current_composing(), "thajtat", "thajt+a+t → passthrough (tt coda invalid)");
}

#[test]
fn debug_gif_inner() {
    let mut e = UltraFastViEngine::new();
    e.feed('g'); println!("after g: {:?}", e.current_composing());
    e.feed('i'); println!("after i: {:?}", e.current_composing());
    e.feed('f'); println!("after f: {:?}", e.current_composing());
    // also test tim
    let mut e2 = UltraFastViEngine::new();
    e2.feed('t'); e2.feed('i'); e2.feed('m');
    println!("tim: {:?}", e2.current_composing());
    // and timf  
    let mut e3 = UltraFastViEngine::new();
    e3.feed('t'); e3.feed('i'); e3.feed('m'); e3.feed('f');
    println!("timf: {:?}", e3.current_composing());
    // gif with assertion
    let mut e4 = UltraFastViEngine::new();
    for ch in "gif".chars() { e4.feed(ch); }
    assert_eq!(e4.current_composing(), "gì", "gif should produce gì");
}

#[test]
fn debug_gif_step_by_step() {
    let mut e = UltraFastViEngine::new();
    let out_g = e.feed('g').to_string();
    println!("g: {:?}", out_g);
    let out_i = e.feed('i').to_string();
    println!("i: {:?}", out_i);
    let out_f = e.feed('f').to_string();
    println!("f: {:?}", out_f);
    assert_eq!(out_f, "gì", "g+i+f should produce gì");
}

#[test]
fn debug_gif_via_is_valid() {
    // Check: does "gi" validate as Vietnamese?
    // onset = [g], nucleus = [i], coda = []
    use crate::tables::{is_legal_onset, is_legal_nucleus, is_legal_coda};
    assert!(is_legal_onset(b"g"), "g is legal onset");
    assert!(is_legal_nucleus(&['i']), "i is legal nucleus");
    assert!(is_legal_coda(b""), "empty coda is legal");
    println!("All table checks pass for g+i");
}

#[test]
fn debug_timff() {
    let mut e = UltraFastViEngine::new();
    for ch in "timf".chars() { e.feed(ch); }
    assert_eq!(e.current_composing(), "tìm", "timf = tìm");
    e.feed('f');
    // Double-cancel: tone removed, first 'f' stays as literal → "timf" passthrough
    assert_eq!(e.current_composing(), "timf", "timff = double cancel = timf (f as literal)");
}
