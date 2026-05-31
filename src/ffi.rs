use std::ffi::{c_char, c_int};
use std::ptr;
use std::sync::{Mutex, MutexGuard};

use crate::engine::UltraFastViEngine;
use crate::modes::InputMethod;

/// Opaque handle to the engine. C code only ever holds a pointer to this type;
/// the fields are intentionally not part of the C ABI.
pub struct UvieEngine {
    inner: Mutex<UltraFastViEngine>,
    pending_utf8: Mutex<Vec<u8>>,
}

impl UvieEngine {
    fn new() -> Self {
        Self {
            inner: Mutex::new(UltraFastViEngine::new()),
            pending_utf8: Mutex::new(Vec::with_capacity(4)),
        }
    }
}

fn lock_engine<'a>(engine: *mut UvieEngine) -> Option<MutexGuard<'a, UltraFastViEngine>> {
    if engine.is_null() {
        return None;
    }
    let engine_ref = unsafe { &*engine };
    engine_ref.inner.lock().ok()
}

fn lock_engine_const<'a>(engine: *const UvieEngine) -> Option<MutexGuard<'a, UltraFastViEngine>> {
    if engine.is_null() {
        return None;
    }
    let engine_ref = unsafe { &*engine };
    engine_ref.inner.lock().ok()
}

fn lock_pending<'a>(engine: *mut UvieEngine) -> Option<MutexGuard<'a, Vec<u8>>> {
    if engine.is_null() {
        return None;
    }
    let engine_ref = unsafe { &*engine };
    engine_ref.pending_utf8.lock().ok()
}

fn clear_pending(engine: *mut UvieEngine) {
    if let Some(mut pending) = lock_pending(engine) {
        pending.clear();
    }
}

fn utf8_prefix_len(bytes: &[u8], max_len: usize) -> usize {
    if bytes.len() <= max_len {
        return bytes.len();
    }
    let mut cut = 0usize;
    for (idx, ch) in std::str::from_utf8(bytes)
        .ok()
        .into_iter()
        .flat_map(|s| s.char_indices())
    {
        let next = idx + ch.len_utf8();
        if next > max_len {
            break;
        }
        cut = next;
    }
    cut
}

fn write_output(out: &str, out_buf: *mut c_char, out_len: usize) -> usize {
    if out_buf.is_null() || out_len == 0 {
        return 0;
    }
    let bytes = out.as_bytes();
    let max_write = out_len.saturating_sub(1);
    let write_len = utf8_prefix_len(bytes, max_write);
    if write_len > 0 {
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf as *mut u8, write_len);
            *out_buf.add(write_len) = 0;
        }
    } else {
        unsafe { *out_buf = 0; }
    }
    write_len
}

fn decode_utf8_char(engine: *mut UvieEngine, byte: u8) -> Option<char> {
    if byte < 0x80 {
        if let Some(mut pending) = lock_pending(engine) {
            pending.clear();
        }
        return Some(byte as char);
    }

    let mut pending = lock_pending(engine)?;
    pending.push(byte);
    if pending.len() > 4 {
        pending.clear();
        return None;
    }

    match std::str::from_utf8(&pending) {
        Ok(s) => {
            let mut it = s.chars();
            let ch = it.next();
            if ch.is_some() && it.next().is_none() {
                pending.clear();
                ch
            } else {
                pending.clear();
                None
            }
        }
        Err(err) => {
            if err.error_len().is_some() {
                pending.clear();
            }
            None
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_new() -> *mut UvieEngine {
    Box::into_raw(Box::new(UvieEngine::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_free(engine: *mut UvieEngine) {
    if !engine.is_null() {
        unsafe { drop(Box::from_raw(engine)); }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_clear(engine: *mut UvieEngine) {
    let _ = std::panic::catch_unwind(|| {
        clear_pending(engine);
        if let Some(mut e) = lock_engine(engine) {
            e.clear();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_commit(engine: *mut UvieEngine) {
    let _ = std::panic::catch_unwind(|| {
        clear_pending(engine);
        if let Some(mut e) = lock_engine(engine) {
            e.commit();
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_set_input_method(engine: *mut UvieEngine, method: c_int) {
    let _ = std::panic::catch_unwind(|| {
        clear_pending(engine);
        if let Some(mut e) = lock_engine(engine) {
        let m = match method {
            0 => InputMethod::Telex,
            1 => InputMethod::Vni,
            _ => return,
        };
            e.set_input_method(m);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_get_input_method(engine: *const UvieEngine) -> c_int {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return -1;
        };
        match e.input_method() {
            InputMethod::Vni => 1,
            InputMethod::Telex => 0,
        }
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_backspace(
    engine: *mut UvieEngine,
    out_buf: *mut c_char,
    out_len: usize,
) -> usize {
    std::panic::catch_unwind(|| {
        clear_pending(engine);
        let Some(mut e) = lock_engine(engine) else {
            return 0;
        };
        let result = e.backspace().to_string();
        write_output(&result, out_buf, out_len)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_is_composing(engine: *const UvieEngine) -> c_int {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return 0;
        };
        if e.is_composing() { 1 } else { 0 }
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_is_empty(engine: *const UvieEngine) -> c_int {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return 1;
        };
        if e.is_empty() { 1 } else { 0 }
    })
    .unwrap_or(1)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_current_output(
    engine: *const UvieEngine,
    out_buf: *mut c_char,
    out_len: usize,
) -> usize {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return 0;
        };
        let result = e.current_output();
        write_output(&result, out_buf, out_len)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_current_composing(
    engine: *const UvieEngine,
    out_buf: *mut c_char,
    out_len: usize,
) -> usize {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return 0;
        };
        let result = e.current_composing().to_string();
        write_output(&result, out_buf, out_len)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_committed_text(
    engine: *const UvieEngine,
    out_buf: *mut c_char,
    out_len: usize,
) -> usize {
    std::panic::catch_unwind(|| {
        let Some(e) = lock_engine_const(engine) else {
            return 0;
        };
        let result = e.committed_text().to_string();
        write_output(&result, out_buf, out_len)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn uvie_engine_feed_utf8(
    engine: *mut UvieEngine,
    ch: u8,
    out_buf: *mut c_char,
    out_len: usize,
) -> usize {
    std::panic::catch_unwind(|| {
        if engine.is_null() || out_buf.is_null() || out_len == 0 {
            return 0;
        }

        let decoded = decode_utf8_char(engine, ch);

        let Some(mut e) = lock_engine(engine) else {
            return 0;
        };

        let result = if let Some(c) = decoded {
            e.feed(c)
        } else {
            e.current_composing()
        }
        .to_string();

        write_output(&result, out_buf, out_len)
    })
    .unwrap_or(0)
}
