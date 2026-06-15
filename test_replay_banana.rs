// Add this to tests.rs temporarily to debug
use crate::ReplayEngine;

#[test]
fn debug_banana_step_by_step() {
    let mut e = ReplayEngine::new();
    
    let chars: Vec<char> = "banana".chars().collect();
    for (idx, &ch) in chars.iter().enumerate() {
        let prev_composing = e.current_composing();
        let (bs, output) = e.feed(ch);
        let new_composing = e.current_composing();
        let committed = e.committed_text();
        
        println!("Step {}: ch='{}'", idx, ch);
        println!("  Prev composing: '{}'", prev_composing);
        println!("  Feed result: bs={}, output='{}'", bs, output);
        println!("  New composing: '{}'", new_composing);
        println!("  Committed text: '{}'", committed);
        println!();
    }
    
    println!("Final state: composing='{}', committed='{}'", 
             e.current_composing(), e.committed_text());
}
