import Foundation

// MARK: - C FFI (ReplayEngine)

@_silgen_name("uvie_replay_new")
func uvie_replay_new() -> OpaquePointer?

@_silgen_name("uvie_replay_free")
func uvie_replay_free(_ engine: OpaquePointer?)

@_silgen_name("uvie_replay_reset")
func uvie_replay_reset(_ engine: OpaquePointer?)

@_silgen_name("uvie_replay_set_input_method")
func uvie_replay_set_input_method(_ engine: OpaquePointer?, _ method: Int32)

@_silgen_name("uvie_replay_set_quick_start")
func uvie_replay_set_quick_start(_ engine: OpaquePointer?, _ enabled: Int32)

@_silgen_name("uvie_replay_set_quick_telex")
func uvie_replay_set_quick_telex(_ engine: OpaquePointer?, _ enabled: Int32)

@_silgen_name("uvie_replay_set_modern_orthography")
func uvie_replay_set_modern_orthography(_ engine: OpaquePointer?, _ enabled: Int32)

@_silgen_name("uvie_replay_feed")
func uvie_replay_feed(_ engine: OpaquePointer?, _ ch: CChar, _ out_buf: UnsafeMutablePointer<CChar>?, _ out_len: Int) -> Int

@_silgen_name("uvie_replay_backspace")
func uvie_replay_backspace(_ engine: OpaquePointer?, _ out_buf: UnsafeMutablePointer<CChar>?, _ out_len: Int) -> Int

@_silgen_name("uvie_replay_commit")
func uvie_replay_commit(_ engine: OpaquePointer?, _ out_buf: UnsafeMutablePointer<CChar>?, _ out_len: Int) -> Int

@_silgen_name("uvie_replay_is_composing")
func uvie_replay_is_composing(_ engine: OpaquePointer?) -> Int32

@_silgen_name("uvie_replay_committed_text")
func uvie_replay_committed_text(_ engine: OpaquePointer?, _ out_buf: UnsafeMutablePointer<CChar>?, _ out_len: Int) -> Int

/// ReplayEngine wrapper.
/// Returns (backspace_count, new_output) from Rust, eliminating TextDiff.
final class EngineBridge {
    private var engine: OpaquePointer?

    var isComposing: Bool {
        guard let engine else { return false }
        return uvie_replay_is_composing(engine) != 0
    }

    init() {
        engine = uvie_replay_new()
    }

    deinit {
        if let engine {
            uvie_replay_free(engine)
        }
    }

    // MARK: - Configuration

    func setInputMethod(_ method: InputMethod) {
        guard let engine else { return }
        uvie_replay_set_input_method(engine, method == .vni ? 1 : 0)
    }

    func setQuickStart(_ enabled: Bool) {
        guard let engine else { return }
        uvie_replay_set_quick_start(engine, enabled ? 1 : 0)
    }

    func setQuickTelex(_ enabled: Bool) {
        guard let engine else { return }
        uvie_replay_set_quick_telex(engine, enabled ? 1 : 0)
    }

    func setModernOrthography(_ enabled: Bool) {
        guard let engine else { return }
        uvie_replay_set_modern_orthography(engine, enabled ? 1 : 0)
    }

    // MARK: - Keystroke handling

    /// Feed a single character. Returns (backspaces, new_output).
    func feed(char: Character) -> (Int, String) {
        guard let engine else { return (0, "") }
        var buf = [CChar](repeating: 0, count: 128)
        let bs = uvie_replay_feed(engine, CChar(char.asciiValue ?? 0), &buf, buf.count)
        return (bs, String(cString: buf))
    }

    /// Backspace. Returns (backspaces, new_output).
    func backspace() -> (Int, String) {
        guard let engine else { return (0, "") }
        var buf = [CChar](repeating: 0, count: 128)
        let bs = uvie_replay_backspace(engine, &buf, buf.count)
        return (bs, String(cString: buf))
    }

    func commit() -> (Int, String) {
        guard let engine else { return (0, "") }
        var buf = [CChar](repeating: 0, count: 128)
        let bs = uvie_replay_commit(engine, &buf, buf.count)
        return (bs, String(cString: buf))
    }

    func reset() {
        guard let engine else { return }
        uvie_replay_reset(engine)
    }

    func committedText() -> String {
        guard let engine else { return "" }
        var buf = [CChar](repeating: 0, count: 128)
        _ = uvie_replay_committed_text(engine, &buf, buf.count)
        return String(cString: buf)
    }
}

enum InputMethod: String, CaseIterable, Identifiable {
    case telex
    case vni
    var id: String { rawValue }
    var displayName: String {
        switch self {
        case .telex: return "Telex"
        case .vni: return "VNI"
        }
    }
}
