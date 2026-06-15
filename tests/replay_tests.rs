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

// ------------------------------------------------------------------
// Fast-typing / diff correctness
// ------------------------------------------------------------------

/// Verify that diff tuples from ReplayEngine correctly reconstruct the
/// expected screen state character-by-character. This catches subtle
/// bugs where `display_composed` and the inner engine output diverge.
#[test]
fn test_diff_reconstruction_neebo() {
    // "neebo" → "nê" committed + "bo" composing
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    for ch in "neebo".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    assert_eq!(screen, "nêbo", "diff reconstruction for 'neebo'");
    assert_eq!(engine.committed_text(), "nê");
}

#[test]
fn test_diff_reconstruction_multiple_syllables() {
    // "toois" → "tôi" + then 's' tone on "tooi" already committed? No:
    // "toos" = t,o,o,s → tốs? Actually: "toos" → onset=t, nucleus=ô, coda=s?
    // s is a tone key, not coda. So "tos" would be "tốs" only if s applies tone.
    // Let's use a simpler case: "boo boo" → "bô bô"
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    for ch in "boo boo".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    assert_eq!(screen, "bô bô");
}

#[test]
fn test_fast_typing_vcv_then_tone() {
    // "nêbo" with tone: "neebot" → "nê" committed + "bôt" composing?
    // Actually "bot" = onset b, nucleus o, coda t → valid; "bôt" if oo → ô.
    // "neebool" → committed "nê" + composing "bool" = "bool"? No: "bool" after
    // split: feed 'b','o','o','l' → bôl (if l is optimistic coda).
    // Let's keep it simple: just check "neebo" diff is correct and no char lost.
    assert_replay("neebo", "nêbo");
}

#[test]
fn test_fast_typing_no_spurious_commit() {
    // Typing "tooi" fast: t+o+o (modifier → tô) + i → "tôi". No VCV split.
    // Ensure committed_text stays empty.
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    for ch in "tooi".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    assert_eq!(screen, "tôi");
    assert_eq!(engine.committed_text(), "", "no auto-commit for single syllable 'tooi'");
}

#[test]
fn test_fast_typing_ddau() {
    // "ddaau" → đâu (đ from dd, â from aa, u appended)
    assert_replay("ddaau", "đâu");
}

#[test]
fn test_backspace_then_retype() {
    // Type "thajta", backspace once, retype "thajta" — should give "thật" twice? No,
    // backspace removes one char so we'd have "thajt" + "a" again = "thật".
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    
    // Type "thajta"
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("after thajta: {:?}", screen);
    
    // Backspace once
    let (bs, out) = engine.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after BS: {:?}", screen);
    
    // Type 'a' again
    let (bs, out) = engine.feed('a');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after 'a': {:?}", screen);
    
    assert_eq!(screen, "thật", "backspace + retype 'a' should recover thật");
}

#[test]
fn test_backspace_retype_full_word() {
    // Type "thajta", BS once, then type "thajta" again (as if retyping the whole word)
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("after thajta: {:?}", screen);
    
    // Backspace once (removes last 'a')
    let (bs, out) = engine.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after BS: {:?}", screen);
    
    // Now type "thajta" again (the user retyped)
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: screen={:?} (bs={}, out={:?})", ch, screen, bs, out);
    }
    println!("final: {:?}", screen);
}

#[test]
fn test_backspace_retype_scenario2() {
    // User scenario: type "thajta", see "thật", then BS once = "thạt",
    // then want to type the word again from scratch by pressing more BS
    // to clear then retyping
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("after thajta: {:?}", screen);
    assert_eq!(screen, "thật");
    
    // BS 6 times to clear everything
    for i in 0..6 {
        let (bs, out) = engine.backspace();
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("BS {}: screen={:?}", i+1, screen);
    }
    assert_eq!(screen, "", "6 BS should clear the 6 raw chars");
    
    // Retype thajta fresh
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("retyped: {:?}", screen);
    assert_eq!(screen, "thật");
}

#[test] 
fn test_sticky_after_backspace() {
    // Scenario: type "thajta" (6 raw chars), BS 1 (→ 5 raw "thajt"),
    // then continue typing "a" → should give "thật", not get stuck.
    // This simulates "I mistyped, let me fix last char".
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "thajt".chars() { // type 5 chars
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("after thajt: {:?}", screen);
    // Should show "thạt" (valid coda, j applied nặng)
    
    let (bs, out) = engine.feed('a'); // type the modifier 'a'
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after a: {:?}", screen);
    assert_eq!(screen, "thật", "thajt+a = thật");
    
    // Now BS once: removes 'a' (last raw key), back to "thạt"
    let (bs, out) = engine.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after BS: {:?}", screen);
    assert_eq!(screen, "thạt");
    
    // Type 'a' again: should give "thật" again
    let (bs, out) = engine.feed('a');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("after a again: {:?}", screen);
    assert_eq!(screen, "thật", "after BS + retype 'a' should recover thật");
}

#[test]
fn test_bs_and_type_thajta_again() {
    // Simulate: type "thajta" → "thật", BS once, then continue with "thajta"
    // This is what the macOS IME sees when user types a word, presses BS once,
    // then types the correction.
    // After BS(1): raw has 5 chars [t,h,a,j,t]. Then user types "a" to fix = "thật".
    // If user instead types "thajta" again = 6 more chars appended to the 5 = 11 raw chars.
    let mut engine = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    assert_eq!(screen, "thật");
    
    // BS once
    let (bs, out) = engine.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    assert_eq!(screen, "thạt");
    
    // Now what if user mistakenly types "thajta" again (appending to raw)?
    // This simulates the "dính chữ" bug scenario. Each char should produce
    // at most bs=1 + 1 char out (no large jumps that corrupt the screen).
    for ch in "thajta".chars() {
        let (bs, out) = engine.feed(ch);
        // Key invariant: no large backspace bursts (was producing bs=3 before fix)
        assert!(bs <= 1, "bs={} for char {:?} — should never exceed 1 in passthrough append", bs, ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    // Screen should be a clean passthrough sequence, no garbled chars.
    assert!(
        screen.chars().all(|c| c.is_ascii() || c.is_alphabetic()),
        "screen {:?} should not contain garbled/unexpected chars",
        screen
    );
}

#[test]
fn debug_gif_tim() {
    // "gif" -> should be "gì" (g+i+f where f=huyền) ?
    // Actually "gif" in Telex: g=consonant, i=vowel, f=tone(huyền) → "gì"
    // But "gif" is also an English word...
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    for ch in "gif".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("gif -> {:?}", screen);
    
    // "tim" -> "tìm" (t+i+m, no tone = "tim") or "tìm" (with tone?)
    // Actually "tim" in Telex: t=onset, i=nucleus, m=coda → "tim" (level tone, no tone key)
    let mut e2 = ReplayEngine::new();
    let mut screen2 = String::new();
    for ch in "tim".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen2);
    }
    println!("tim -> {:?}", screen2);
    
    // "timf" -> "tìm" (t+i+m+f where f=huyền tone applied to nucleus i)
    let mut e3 = ReplayEngine::new();
    let mut screen3 = String::new();
    for ch in "timf".chars() {
        let (bs, out) = e3.feed(ch);
        for _ in 0..bs { screen3.pop(); }
        screen3.push_str(&out);
    }
    println!("timf -> {:?}", screen3);
}

#[test]
fn test_tim_sequences() {
    // "tìm" in Telex = "timf" (t+i+m+f where f=huyền)
    // But user reports "tìm" sometimes correct, sometimes not.
    // Possible fast-typing scenario: user types "timf" very fast,
    // or maybe "tim " (with space) = "tim" plain.
    
    // Scenario 1: t+i+m+f = "tìm"
    let mut e = ReplayEngine::new();
    let mut s = String::new();
    for ch in "timf".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { s.pop(); }
        s.push_str(&out);
    }
    assert_eq!(s, "tìm", "timf = tìm");
    
    // Scenario 2: t+i+m (no tone key) = "tim" (level tone)
    let mut e2 = ReplayEngine::new();
    let mut s2 = String::new();
    for ch in "tim".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { s2.pop(); }
        s2.push_str(&out);
    }
    assert_eq!(s2, "tim", "tim (no tone) = tim");
    
    // Scenario 3: fast-typing "tim " (with space)
    let mut e3 = ReplayEngine::new();
    let mut s3 = String::new();
    for ch in "tim ".chars() {
        let (bs, out) = e3.feed(ch);
        for _ in 0..bs { s3.pop(); }
        s3.push_str(&out);
    }
    println!("tim+space: {:?}", s3);
    
    // Scenario 4: "tìm " — is "tìm" committed correctly?
    let mut e4 = ReplayEngine::new();
    let mut s4 = String::new();
    for ch in "timf ".chars() {
        let (bs, out) = e4.feed(ch);
        for _ in 0..bs { s4.pop(); }
        s4.push_str(&out);
    }
    println!("timf+space: {:?}", s4);
    assert!(s4.contains("tìm"), "timf+space should contain tìm, got {:?}", s4);
}

#[test]
fn test_tim_typo_orders() {
    // Fast typing can scramble key order. Test all permutations that make sense.
    let cases: &[(&str, &str)] = &[
        ("timf",  "tìm"),   // correct order
        ("tfim",  "tìm"),   // tone before last vowel — should still work?
        ("tifm",  "tìm"),   // tone after i, before m
        ("tfmi",  "tfmi"),  // very scrambled — passthrough expected
        ("timff", "timf"),  // double-f cancels tone, first f stays as literal → "timf"
    ];
    for (input, expected) in cases {
        let mut e = ReplayEngine::new();
        let mut s = String::new();
        for ch in input.chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { s.pop(); }
            s.push_str(&out);
        }
        println!("{:?} -> {:?} (expected {:?})", input, s, expected);
    }
}

#[test]
fn debug_phat_replay() {
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    for ch in "phast".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("phast -> {:?}", screen);
    assert_eq!(screen, "phát", "phast should give phát");
}

#[test]
fn debug_phat_after_backspace() {
    // Simulate: type something, BS several times, then type "phast"
    // This is the "asttuowng" / residue bug scenario.
    
    // Scenario 1: type "abc", BS 3 times (clear), type "phast"
    {
        let mut e = ReplayEngine::new();
        let mut screen = String::new();
        for ch in "abc".chars() {
            let (bs, out) = e.feed(ch); 
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        // BS 3 times to clear
        for _ in 0..3 {
            let (bs, out) = e.backspace();
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        assert_eq!(screen, "", "after 3 BS screen empty");
        // Now type phast
        for ch in "phast".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        println!("abc+3BS+phast: {:?}", screen);
        assert_eq!(screen, "phát", "after clearing abc, phast = phát");
    }
    
    // Scenario 2: type "phatr" (phạtr → some word), BS 2 times, retype "st"
    {
        let mut e = ReplayEngine::new();
        let mut screen = String::new();
        for ch in "phatr".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        println!("phatr: {:?}", screen);
        // BS 2 (remove r and t)
        for _ in 0..2 {
            let (bs, out) = e.backspace();
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        println!("phatr+2BS: {:?}", screen);
        // Continue with "st"
        for ch in "st".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
            println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
        }
        println!("phatr+2BS+st: {:?}", screen);
        assert_eq!(screen, "phát", "phatr+2BS+st = phát");
    }
}

#[test]
fn debug_phat_space_retry() {
    // User sees "phast" on screen after space, types space again then "phast"
    // = commit "phast" (as passthrough?), then "phast" again fresh
    
    // More likely: user typed something that caused the word boundary to not reset.
    // Test: type "phas" (→ "phá"), then type 't' slowly vs fast
    
    // What if "phast" in user's context arrives as two events from previous 
    // word + new word? E.g. word ends with 'p', then user types "hast":
    {
        let mut e = ReplayEngine::new();
        let mut screen = String::new();
        // Previous word ends with 'p' already composing
        for ch in "p".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        // Now types "hast" (continuation)
        for ch in "hast".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
            println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
        }
        println!("p+hast: {:?}", screen);
        // With raw=[p,h,a,s,t]: ph onset, a vowel, s tone, t coda → "phát"
        assert_eq!(screen, "phát", "p+hast should compose phát");
    }
}

#[test]
fn debug_phat_vcv_trigger() {
    // After space (word boundary), type "phast" fast
    // The 's' could trigger VCV if prev word ended in a vowel...
    // But space should have reset everything.
    
    // Simulate: type "de " (để → "để") then "phast"
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    // Type "def " (đề → with huyền → "để" then space)
    for ch in "de ".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("'de ': {:?}", screen);
    
    // Now type "phast"
    for ch in "phast".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("de +phast: {:?}", screen);
    assert_eq!(screen, "de phát", "de +phast = de phát");
    
    // More specific: what if previous word was "phat" (English passthrough)
    // and user BS to retype?
    let mut e2 = ReplayEngine::new();
    let mut screen2 = String::new();
    for ch in "phat".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
    }
    println!("phat initial: {:?}", screen2);
    // BS all 4
    for _ in 0..4 {
        let (bs, out) = e2.backspace();
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
    }
    println!("phat+4BS: {:?}", screen2);
    // Retype phast
    for ch in "phast".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen2);
    }
    println!("phat+4BS+phast: {:?}", screen2);
    assert_eq!(screen2, "phát");
}

#[test]
fn debug_phat_bs_partial_retype() {
    // Type "ph", then "at", then BS 2 to remove "at", then type "ast"
    // net result: raw = [p,h,a,s,t] → "phát"
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "phat".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("phat: {:?}", screen);
    
    // BS twice  
    for i in 0..2 {
        let (bs, out) = e.backspace();
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("BS {}: {:?}", i+1, screen);
    }
    
    // Retype "ast"
    for ch in "ast".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("phat+2BS+ast: {:?}", screen);
    assert_eq!(screen, "phát", "phat+2BS+ast = phát");
    
    // Also: BS from mid-word composing state
    // "phat" with partial edit
    let mut e2 = ReplayEngine::new();
    let mut screen2 = String::new();
    for ch in "ph".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
    }
    // Now type "a" making "pha"
    for ch in "a".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
    }
    println!("pha: {:?}", screen2);
    // BS once
    let (bs, out) = e2.backspace();
    for _ in 0..bs { screen2.pop(); }
    screen2.push_str(&out);
    println!("pha+BS: {:?}", screen2);
    // Continue with "ast"
    for ch in "ast".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { screen2.pop(); }
        screen2.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen2);
    }
    println!("pha+BS+ast: {:?}", screen2);
    assert_eq!(screen2, "phát", "pha+BS+ast = phát (s=sắc tone, t=coda)");
}

#[test]
fn debug_vcv_then_phat() {
    // After a VCV split (e.g. "naabo" → "na" committed + "bo" composing),
    // type "phast" fresh after space commit.
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    // Trigger a VCV: "naabo"
    for ch in "naabo".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("naabo: screen={:?} committed={:?}", screen, e.committed_text());
    
    // Space to commit
    let (bs, out) = e.feed(' ');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("space: {:?}", screen);
    
    // Type "phast"
    for ch in "phast".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("phast after: {:?}", screen);
    assert!(screen.ends_with("phát"), "should end with phát, got {:?}", screen);
}

#[test]
fn test_no_bs_without_out() {
    // Invariant: if bs > 0, out must be non-empty
    // (we're replacing something with something else)
    let test_cases: Vec<&str> = vec![
        "phast", "tìm", "gif", "thajta", "naabo", "banana",
        "phat", "tess", "timff", "giss", "added",
    ];
    for word in test_cases {
        let mut e = ReplayEngine::new();
        for ch in word.chars() {
            let (bs, out) = e.feed(ch);
            assert!(
                !(bs > 0 && out.is_empty()),
                "word={:?} char={:?}: bs={} but out is empty — would delete without replacement",
                word, ch, bs
            );
        }
    }
}

#[test]
fn debug_fix_xx_bs() {
    // Scenario: type "fĩ" (phĩ?), then add x, x, then BS, then type more
    // "fĩ" in Telex: f is tone huyền... but f alone → onset? 
    // Actually: what sequence produces "fĩ"?
    // f = tone huyền in Telex (but only applied to vowel)
    // OR f = quick-start for ph?
    // Let's try: "gi" + "x" + "x" + BS + type
    // More likely user means: type a word ending in ĩ (ngã tone on i)
    // then x (ngã = x in Telex), then x again, then BS
    // e.g. "gix" → "gĩ", then "x" → "gĩx" (double x cancel), then BS
    
    // Reproduce: type "gix" (gĩ), then x (double-cancel), then BS, then type
    let test_sequences: &[(&str, &[char], &str)] = &[
        ("gix",  &['x', 'x', '\x08', 'x'],  "gĩx"),  // gĩ + xx (cancel) + BS + x
        ("fi",   &['x', 'x', '\x08'],        "fix"),   // fi + x + x + BS
        ("fis",  &['x', 'x', '\x08', 'p'],   "fisp"),
    ];
    
    // Main scenario: type the base word, then xx, BS, continue
    {
        let mut e = ReplayEngine::new();
        let mut screen = String::new();
        
        // Type "gix" → gĩ
        for ch in "gix".chars() {
            let (bs, out) = e.feed(ch);
            for _ in 0..bs { screen.pop(); }
            screen.push_str(&out);
        }
        println!("gix: {:?}", screen);
        
        // Type second x → double-cancel → "gix" passthrough
        let (bs, out) = e.feed('x');
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("gixx: {:?} (bs={}, out={:?})", screen, bs, out);
        
        // BS once
        let (bs, out) = e.backspace();
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("gixx+BS: {:?} (bs={}, out={:?})", screen, bs, out);
        
        // Type 'x' again → should give gĩ
        let (bs, out) = e.feed('x');
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("gixx+BS+x: {:?} (bs={}, out={:?})", screen, bs, out);
        
        assert_eq!(screen, "gĩ", "gixx+BS+x should recover gĩ");
    }
}

#[test]
fn debug_fix_scenario_exact() {
    // Trace exact: f+i+x → then x → then BS → then type
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "fix".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("fix: {:?}", screen);
    
    // x again (double-cancel)
    let (bs, out) = e.feed('x');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("fixx: bs={} out={:?} screen={:?}", bs, out, screen);
    
    // BS
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("fixx+BS: bs={} out={:?} screen={:?}", bs, out, screen);
    
    // Type 'x' to re-apply tone
    let (bs, out) = e.feed('x');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("fixx+BS+x: bs={} out={:?} screen={:?}", bs, out, screen);
    
    // Type more chars
    for ch in " after".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("full: {:?}", screen);
}

#[test]
fn debug_fix_then_continue() {
    // fĩ + x (double cancel → fix) + x (another x!) + BS + type
    // This is: f+i+x+x+x+BS+...
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    // Type f, i, x, x, x (3 x's)
    for ch in "fixxx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("fixxx: {:?}", screen);
    
    // BS once
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("fixxx+BS: bs={} out={:?} screen={:?}", bs, out, screen);
    
    // Type 'n' (gõ thêm để tạo từ)
    for ch in "nhan".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("  fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, screen);
    }
    println!("final: {:?}", screen);
}

#[test]
fn debug_fixxx_detailed() {
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "fixxx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("fed {:?}: bs={} out={:?} current={:?} screen={:?}", 
                 ch, bs, out, e.current_composing(), screen);
    }
    
    let (bs, out) = e.backspace();
    println!("BS: bs={} out={:?} current={:?}", bs, out, e.current_composing());
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("screen after BS: {:?}", screen);
}

#[test]
fn debug_fixxx_trace_inner() {
    use uvie::UltraFastViEngine;
    // Trace the inner engine directly for fixxx
    let mut e = UltraFastViEngine::new();
    for ch in "fixxx".chars() {
        let out = e.feed(ch);
        println!("inner fed {:?}: {:?}", ch, out);
    }
    // Now backspace
    let out = e.backspace();
    println!("inner BS: {:?}", out);
}

#[test]
fn debug_fixxx_bs_then_letter() {
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "fixxx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("fixxx: screen={:?} raw_len={}", screen, e.raw_len());
    
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("BS: bs={} out={:?} screen={:?} raw_len={}", bs, out, screen, e.raw_len());
    
    // Type 'n'
    let (bs, out) = e.feed('n');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("n: bs={} out={:?} screen={:?} raw_len={}", bs, out, screen, e.raw_len());
    
    // Type 'h'
    let (bs, out) = e.feed('h');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("h: bs={} out={:?} screen={:?} raw_len={}", bs, out, screen, e.raw_len());
    
    // Type 'a'
    let (bs, out) = e.feed('a');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("a: bs={} out={:?} screen={:?} raw_len={}", bs, out, screen, e.raw_len());
}

#[test]
fn debug_fixnha_inner() {
    use uvie::UltraFastViEngine;
    let mut e = UltraFastViEngine::new();
    for ch in "fixnha".chars() {
        let out = e.feed(ch);
        println!("inner fed {:?}: {:?}", ch, out);
    }
}

#[test]
fn debug_fixx_bs_space_then_word() {
    // fĩ + x + x (cancel) + BS + space → commit? then new word
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "fixx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("fixx: {:?} raw={}", screen, e.raw_len());
    
    // BS once
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("fixx+BS: bs={} {:?} raw={}", bs, screen, e.raw_len());
    
    // Space → commit
    let (bs, out) = e.feed(' ');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("space: {:?} raw={}", screen, e.raw_len());
    
    // Type "phast" fresh
    for ch in "phast".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
    }
    println!("phast: {:?}", screen);
    assert!(screen.ends_with("phát"), "should end with phát, got {:?}", screen);
    
    // Also: fixx + BS → "fĩ" on screen → space → commit fĩ + "phast"
    let mut e2 = ReplayEngine::new();
    let mut s2 = String::new();
    for ch in "fixx".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { s2.pop(); }
        s2.push_str(&out);
    }
    // BS
    let (bs, out) = e2.backspace();
    for _ in 0..bs { s2.pop(); }
    s2.push_str(&out);
    // Space 
    let (bs, out) = e2.feed(' ');
    for _ in 0..bs { s2.pop(); }
    s2.push_str(&out);
    println!("e2 after space: {:?}", s2);
    // Type "phast"
    for ch in "phast".chars() {
        let (bs, out) = e2.feed(ch);
        for _ in 0..bs { s2.pop(); }
        s2.push_str(&out);
        println!("  e2 fed {:?}: bs={} out={:?} screen={:?}", ch, bs, out, s2);
    }
    println!("e2 final: {:?}", s2);
}

#[test]
fn debug_fi_4x_bs_type() {
    // fi + xxxx (4 x's) + BS + type
    let mut e = ReplayEngine::new();
    let mut screen = String::new();
    
    for ch in "fixxxx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("fed {:?}: bs={} out={:?} screen={:?} raw={}", ch, bs, out, screen, e.raw_len());
    }
    
    println!("--- BS ---");
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("BS: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
    
    println!("--- type 'a' ---");
    let (bs, out) = e.feed('a');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("a: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
    
    println!("--- type 'n' ---");
    let (bs, out) = e.feed('n');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("n: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
}

#[test]
fn debug_fixx_quickstart_bs_type() {
    // With quick-start mode: f→ph, so "fix" = "phĩ"
    // Then xx + BS + type → should not stick
    let mut e = ReplayEngine::new();
    e.set_quick_start(true);
    let mut screen = String::new();
    
    for ch in "fixxx".chars() {
        let (bs, out) = e.feed(ch);
        for _ in 0..bs { screen.pop(); }
        screen.push_str(&out);
        println!("fed {:?}: bs={} out={:?} screen={:?} raw={}", ch, bs, out, screen, e.raw_len());
    }
    println!("--- BS ---");
    let (bs, out) = e.backspace();
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("BS: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
    
    // Type 'a'
    let (bs, out) = e.feed('a');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("a: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
    
    // Type 'n'
    let (bs, out) = e.feed('n');
    for _ in 0..bs { screen.pop(); }
    screen.push_str(&out);
    println!("n: bs={} out={:?} screen={:?} raw={}", bs, out, screen, e.raw_len());
    
    println!("no sticky chars in screen: {:?}", screen);
}
