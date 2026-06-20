# uvie-rs

Ultra-fast Vietnamese input method engine (Telex, VNI) written in Rust.

A `no_std` / `no-alloc` compatible library with zero external dependencies. Designed for sub-microsecond latency per keystroke through incremental state updates and positive syllable validation.

**macOS implementation**: See [UVieKey](https://github.com/thuupx/UVieKey) for the macOS menu bar app that uses this engine.

## Features

- **Telex & VNI**: Full support for both popular input methods.
- **Modern orthography**: Optional tone placement per new standard (e.g. `hoas` → `hoá`).
- **Per-character state**: Each keystroke gets its own state entry; transforms are bit-flips, not multi-pass reordering.
- **Validate raw keystrokes**: Checks raw ASCII sequence against positive syllable tables before any transform. English passthrough is automatic.
- **Diff-based API**: Returns `(backspace_count, suffix_to_type)` per keystroke for minimal screen updates.
- **No heap in hot path**: Fixed stack buffers, zero allocation during normal operation.
- **`no_std` compatible**: Works in embedded or constrained environments via the `heapless` feature.

## Architecture

The engine uses an incremental, stateful model with per-character state buffers. See [`src/ARCHITECT.md`](src/ARCHITECT.md) for detailed design rationale, state model, and per-keystroke flow.

## Usage

```rust
use uvie::{UltraFastViEngine, InputMethod};

let mut engine = UltraFastViEngine::new();
engine.set_input_method(InputMethod::Telex);

// Feed keystrokes
let result = engine.feed('t');  // "t"
let result = engine.feed('i');  // "ti"
let result = engine.feed('e');  // "tie"
let result = engine.feed('s');  // "tiế"

// Commit word (e.g. on space)
engine.commit();
```

## FFI / C API

The library compiles to `staticlib` and `cdylib`. The C API provides:

- `uvie_engine_new()` / `uvie_engine_free()`
- `uvie_feed(engine, key, &backspace_count, suffix, suffix_len)`
- `uvie_backspace(engine, &backspace_count, suffix, suffix_len)`
- `uvie_set_mode(engine, method)`
- Configuration toggles (modern orthography, etc.)

See [`src/ffi.rs`](src/ffi.rs) for the full C API.

## Building

```bash
# Build release library
cargo build --release

# Run benchmarks
cargo bench

# Run tests
cargo test
```

For `no_std` / `heapless` builds:

```bash
cargo build --release --no-default-features --features heapless
```

## Benchmark

Apple Silicon (`cargo bench`), comparison against the `vi` crate:

| Case | Telex speedup (vi / uvie) | VNI speedup (vi / uvie) |
| ------ | --------------------------: | ------------------------: |
| simple | ~5.8x | ~5.7x |
| sentence | ~6.1x | ~5.3x |
| mixed | ~15.8x | ~10.7x |
| cluster | ~6.7x | ~6.7x |
| ui | ~5.8x | ~2.8x |

Full report: [thuupx.github.io/uvie-rs/criterion/report/](https://thuupx.github.io/uvie-rs/criterion/report/)

## License

MIT OR Apache-2.0
