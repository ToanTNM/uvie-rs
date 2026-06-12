import Cocoa
import ApplicationServices

/// Injects text via Accessibility API (AXUIElement) instead of CGEventTap.
/// Used for apps where synthetic key events don't work (Spotlight, some secure fields).
final class AXTextInjector {
    private let _engine = EngineBridge()
    var engine: EngineBridge { _engine }

    /// Cached focused element to avoid repeated lookups.
    private weak var cachedElement: AXUIElement?
    private var lastElementPID: pid_t = 0

    // MARK: - Engine config pass-through

    func setInputMethod(_ method: InputMethod) { _engine.setInputMethod(method) }
    func setQuickStart(_ enabled: Bool) { _engine.setQuickStart(enabled) }
    func setQuickTelex(_ enabled: Bool) { _engine.setQuickTelex(enabled) }

    // MARK: - Keystroke handling

    /// Feed a character. Returns true if AX injection succeeded.
    func feed(char: Character) -> Bool {
        guard let element = getFocusedTextElement() else { return false }

        let (bs, out) = _engine.feed(char: char)
        guard bs > 0 || !out.isEmpty else {
            // No change — but we still need to show the character
            // (happens when engine returns empty for a literal char)
            return false
        }

        guard let current = getTextValue(element) else { return false }

        var newText = current
        for _ in 0..<bs { newText = String(newText.dropLast()) }
        newText += out

        setTextValue(element, text: newText)
        setCursorToEnd(element, length: newText.count)
        return true
    }

    /// Backspace. Returns true if AX injection succeeded.
    func backspace() -> Bool {
        guard let element = getFocusedTextElement() else { return false }

        let (bs, out) = _engine.backspace()
        guard bs > 0 || !out.isEmpty else { return false }

        guard let current = getTextValue(element) else { return false }

        var newText = current
        for _ in 0..<bs { newText = String(newText.dropLast()) }
        newText += out

        setTextValue(element, text: newText)
        setCursorToEnd(element, length: newText.count)
        return true
    }

    /// Commit on word boundary.
    func commit() {
        _ = _engine.commit()
    }

    func reset() {
        _engine.reset()
    }

    // MARK: - AX Helpers

    private func getFocusedTextElement() -> AXUIElement? {
        let systemWide = AXUIElementCreateSystemWide()
        var focusedElement: CFTypeRef?
        let result = AXUIElementCopyAttributeValue(
            systemWide,
            kAXFocusedUIElementAttribute as CFString,
            &focusedElement
        )
        guard result == .success else { return nil }
        let element = focusedElement as! AXUIElement

        // Verify it's a text field (has Value attribute)
        var value: CFTypeRef?
        let hasValue = AXUIElementCopyAttributeValue(element, kAXValueAttribute as CFString, &value)
        guard hasValue == .success else { return nil }

        return element
    }

    private func getTextValue(_ element: AXUIElement) -> String? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, kAXValueAttribute as CFString, &value) == .success else {
            return nil
        }
        return value as? String
    }

    private func setTextValue(_ element: AXUIElement, text: String) {
        AXUIElementSetAttributeValue(element, kAXValueAttribute as CFString, text as CFTypeRef)
    }

    private func setCursorToEnd(_ element: AXUIElement, length: Int) {
        var range = CFRange(location: length, length: 0)
        guard let axRange = AXValueCreate(.cfRange, &range) else { return }
        AXUIElementSetAttributeValue(element, kAXSelectedTextRangeAttribute as CFString, axRange)
    }
}
