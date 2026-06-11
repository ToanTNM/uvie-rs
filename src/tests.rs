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
    let mut e = UltraFastViEngine::new();
    // aaa -> a
    assert_eq!(type_seq(&mut e, "aaa"), "a");

    let mut e = UltraFastViEngine::new();
    // ddd -> d
    assert_eq!(type_seq(&mut e, "ddd"), "d");

    let mut e = UltraFastViEngine::new();
    // eee -> e
    assert_eq!(type_seq(&mut e, "eee"), "e");

    let mut e = UltraFastViEngine::new();
    // ooo -> o
    assert_eq!(type_seq(&mut e, "ooo"), "o");
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

    // added: dd is adjacent -> đ in Telex (correct behavior, use ddd to get literal dd)
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "added"), "ađed");

    // banana: a..a bubbles, result bânna is structurally valid Vietnamese
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "banana"), "bânna");

    // resset: double-s cancels tone, then Rule 3 keeps second s as literal -> "reset"
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "resset"), "reset");

    // Free-style still works when only vowels/w separate: neues -> nếu
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "neues"), "nếu");

    // Free-style across consonants with tone key: memef -> mềm
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "memef"), "mềm");

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

    // Triple still toggles back
    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "aaa"), "a");

    let mut e = UltraFastViEngine::new();
    assert_eq!(type_seq(&mut e, "eee"), "e");
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
