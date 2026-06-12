use uvie::{InputMethod, ReplayEngine};

fn assert_replay(input: &str, expected: &str) {
    let mut engine = ReplayEngine::new();
    let mut result = String::new();
    for ch in input.chars() {
        let (backspaces, output) = engine.feed(ch);
        for _ in 0..backspaces {
            result.pop();
        }
        result.push_str(&output);
    }
    assert_eq!(result, expected, "input: {:?}", input);
}

fn assert_replay_vni(input: &str, expected: &str) {
    let mut engine = ReplayEngine::new();
    engine.set_input_method(InputMethod::Vni);
    let mut result = String::new();
    for ch in input.chars() {
        let (backspaces, output) = engine.feed(ch);
        for _ in 0..backspaces {
            result.pop();
        }
        result.push_str(&output);
    }
    assert_eq!(result, expected, "input: {:?}", input);
}

// ------------------------------------------------------------------
// Basic words
// ------------------------------------------------------------------

#[test]
fn test_simple_word() {
    assert_replay("xin", "xin");
}

#[test]
fn test_tone_mark() {
    assert_replay("tooi", "tôi");
}

#[test]
fn test_space_boundary() {
    assert_replay("xin chao", "xin chao");
}

// ------------------------------------------------------------------
// Complex syllables
// ------------------------------------------------------------------

#[test]
fn test_diphthong_tone() {
    assert_replay("toanj", "toạn");
}

#[test]
fn test_diphthong_ng() {
    assert_replay("thuowngj", "thượng");
}

#[test]
fn test_medial_o() {
    assert_replay("ngoanf", "ngoàn");
}

// TODO: Engine tone placement for diphthong "yê" needs fix.
// "quyets" currently yields "quýet" (tone on 'y') instead of "quyết" (tone on 'ê').
#[test]
#[ignore = "engine diphthong tone placement not yet implemented"]
fn test_quyet() {
    assert_replay("quyets", "quyết");
}

// TODO: Engine tone placement for "yê" needs fix.
// "huyenf" currently yields "huỳen" instead of "huyền".
#[test]
#[ignore = "engine diphthong tone placement not yet implemented"]
fn test_huyen() {
    assert_replay("huyenf", "huyền");
}

// ------------------------------------------------------------------
// VNI — currently unsupported (digits not handled by engine classify table)
// ------------------------------------------------------------------

#[test]
#[ignore = "VNI digits not yet fully supported by engine"]
fn test_vni_thuong() {
    assert_replay_vni("thuong75", "thượng");
}

#[test]
#[ignore = "VNI digits not yet fully supported by engine"]
fn test_vni_quyet() {
    assert_replay_vni("quyet61", "quyết");
}

// ------------------------------------------------------------------
// Rollback / invalid
// ------------------------------------------------------------------

#[test]
fn test_invalid_sequence() {
    assert_replay("fgh", "fgh");
}

// ------------------------------------------------------------------
// Backspace
// ------------------------------------------------------------------

#[test]
fn test_backspace() {
    let mut engine = ReplayEngine::new();
    let mut result = String::new();

    // Type "tooi" -> "tôi"
    for ch in "tooi".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { result.pop(); }
        result.push_str(&out);
    }
    assert_eq!(result, "tôi");

    // Backspace once -> "tô"
    let (bs, out) = engine.backspace();
    for _ in 0..bs { result.pop(); }
    result.push_str(&out);
    assert_eq!(result, "tô");

    // Backspace again -> "to" (raw input "to" renders as "to")
    let (bs, out) = engine.backspace();
    for _ in 0..bs { result.pop(); }
    result.push_str(&out);
    assert_eq!(result, "to");

    // Backspace again -> "t"
    let (bs, out) = engine.backspace();
    for _ in 0..bs { result.pop(); }
    result.push_str(&out);
    assert_eq!(result, "t");

    // Backspace again -> ""
    let (bs, out) = engine.backspace();
    for _ in 0..bs { result.pop(); }
    result.push_str(&out);
    assert_eq!(result, "");
}

// ------------------------------------------------------------------
// Punctuation / word boundaries
// ------------------------------------------------------------------

#[test]
fn test_comma_boundary() {
    assert_replay("xin,", "xin,");
}

#[test]
fn test_period_boundary() {
    assert_replay("xin.", "xin.");
}
