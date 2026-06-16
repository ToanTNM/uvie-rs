import Cocoa
import Carbon

// MARK: - App Classification

/// Apps that need empty-character sentinel before backspace (invalidate autocomplete).
/// Matched by prefix OR exact bundle ID.
/// OpenKey uses: SendEmptyCharacter() + extra backspace for these apps.
private let compoundApps: Set<String> = [
    "com.apple.Safari",
    "com.apple.Notes",
    "com.apple.TextEdit",
    "com.apple.mail",
    "com.apple.iWork",
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "com.brave.Browser",
    "com.brave.Browser.nightly",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.Edge.Dev",
    "com.microsoft.Edge",
    "org.chromium.Chromium",
]

/// Apps that need Accessibility text injection instead of CGEventTap.
/// Spotlight and some secure text fields don't accept synthetic key events.
private let axApps: Set<String> = [
    "com.apple.Spotlight",
]

/// Apps that should bypass IME entirely (system UI, lock screen, etc.)
private let bypassApps: Set<String> = [
    "com.apple.loginwindow",
    "com.apple.securityagent",
    "com.apple.ScreenSaver.Engine",
    "com.apple.systemuiserver",
]

/// Chromium browsers that need Shift+Left Arrow selection
/// instead of plain backspace (avoids duplicate chars).
private let chromiumBrowsers: Set<String> = [
    "com.google.Chrome",
    "com.google.Chrome.canary",
    "com.brave.Browser",
    "com.brave.Browser.nightly",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Dev",
    "com.microsoft.edgemac.Beta",
    "com.microsoft.Edge.Dev",
    "com.microsoft.Edge",
    "org.chromium.Chromium",
    "ai.perplexity.comet",
]

private func checkIsCompoundApp(_ bundleID: String) -> Bool {
    compoundApps.contains(bundleID)
}

private func checkIsChromiumBrowser(_ bundleID: String) -> Bool {
    chromiumBrowsers.contains(bundleID)
}

// MARK: - EventTap

final class EventTap: ObservableObject {
    @Published var isEnabled = true

    private var tap: CFMachPort?
    private var runLoopSource: CFRunLoopSource?
    private let _engine = EngineBridge()
    var engine: EngineBridge { _engine }
    private let eventSource: CGEventSource?

    let inputMethodManager: InputMethodManager
    private let appDetector = AppContextDetector()
    private let axInjector: AXTextInjector

    /// Tag synthetic events so we don't process our own output.
    private let syntheticTag: Int64 = 0x55564945 // "UVIE"

    /// Observer token for UserDefaults runtime changes.
    private var defaultsObserver: NSObjectProtocol?

    init(inputMethodManager: InputMethodManager) {
        self.inputMethodManager = inputMethodManager
        self.axInjector = AXTextInjector(engine: _engine)
        eventSource = CGEventSource(stateID: .privateState)
        applyEngineSettings()
        observeSettingsChanges()
    }

    deinit {
        stop()
        appDetector.stop()
        if let defaultsObserver {
            NotificationCenter.default.removeObserver(defaultsObserver)
        }
    }

    // MARK: - Lifecycle

    func start() {
        guard tap == nil else { return }
        guard AccessibilityChecker.isTrusted else {
            print("EventTap: Accessibility not granted")
            return
        }

        appDetector.start()

        let callback: CGEventTapCallBack = { proxy, type, event, refcon in
            guard let refcon else { return Unmanaged.passRetained(event) }
            let myself = Unmanaged<EventTap>.fromOpaque(refcon).takeUnretainedValue()
            return myself.handle(proxy: proxy, type: type, event: event)
        }

        guard let newTap = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: CGEventMask(
                (1 << CGEventType.keyDown.rawValue) |
                (1 << CGEventType.keyUp.rawValue) |
                (1 << CGEventType.flagsChanged.rawValue)
            ),
            callback: callback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else {
            print("EventTap: Failed to create tap")
            return
        }

        self.tap = newTap
        let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, newTap, 0)
        self.runLoopSource = source
        CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
        CGEvent.tapEnable(tap: newTap, enable: true)

        DispatchQueue.global(qos: .userInteractive).async {
            CFRunLoopRun()
        }
    }

    func stop() {
        if let tap {
            CGEvent.tapEnable(tap: tap, enable: false)
        }
        if let runLoopSource {
            CFRunLoopRemoveSource(CFRunLoopGetCurrent(), runLoopSource, .commonModes)
        }
        tap = nil
        runLoopSource = nil
        appDetector.stop()
    }

    // MARK: - Settings

    /// Read all engine-relevant settings from UserDefaults and push to the
    /// shared engine. Called on init and whenever defaults change at runtime.
    func applyEngineSettings() {
        let defaults = UserDefaults.standard
        let method = defaults.string(forKey: DefaultsKey.inputMethod) ?? "telex"
        _engine.setInputMethod(method == "vni" ? .vni : .telex)
        _engine.setQuickStart(defaults.bool(forKey: DefaultsKey.quickStart))
        _engine.setQuickTelex(defaults.bool(forKey: DefaultsKey.quickTelex))
        _engine.setModernOrthography(defaults.bool(forKey: DefaultsKey.modernOrthography))
    }

    /// Observe runtime setting changes so toggling Quick Telex, Modern
    /// Orthography, etc. in Settings takes effect without restart.
    private func observeSettingsChanges() {
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            self?.applyEngineSettings()
        }
    }

    // MARK: - Event Handling

    private let breakKeyCodes: Set<Int64> = [
        36,  48,  53,  116, 121, 123, 124, 125, 126, 115, 119, 114, 117,
    ]

    private func isBreakKey(_ keyCode: Int64) -> Bool {
        breakKeyCodes.contains(keyCode)
    }

    private var isCompoundApp: Bool {
        checkIsCompoundApp(appDetector.bundleID)
    }

    private var isChromium: Bool {
        checkIsChromiumBrowser(appDetector.bundleID)
    }

    private var isAXApp: Bool {
        axApps.contains(appDetector.bundleID)
    }

    private var shouldBypass: Bool {
        bypassApps.contains(appDetector.bundleID)
    }

    private func handle(proxy: CGEventTapProxy, type: CGEventType, event: CGEvent) -> Unmanaged<CGEvent>? {
        // Skip our own synthetic events
        if event.getIntegerValueField(.eventSourceStateID) == syntheticTag {
            return Unmanaged.passRetained(event)
        }

        // Bypass system UI apps
        if shouldBypass {
            return Unmanaged.passRetained(event)
        }

        // Pass through flags changes
        if type == .flagsChanged {
            return Unmanaged.passRetained(event)
        }

        // Only handle keyDown/keyUp
        guard type == .keyDown || type == .keyUp else {
            return Unmanaged.passRetained(event)
        }

        let keyCode = event.getIntegerValueField(.keyboardEventKeycode)
        let flags = event.flags

        // Pass through modifier combinations
        if flags.contains(.maskCommand) || flags.contains(.maskControl) ||
           flags.contains(.maskAlternate) || flags.contains(.maskSecondaryFn) {
            return Unmanaged.passRetained(event)
        }

        // Pass through Command keys themselves
        if keyCode == 55 || keyCode == 54 {
            return Unmanaged.passRetained(event)
        }

        // In English mode, pass everything through
        guard inputMethodManager.isVietnamese else {
            return Unmanaged.passRetained(event)
        }

        // --- AX mode (Spotlight, etc.) ---
        if isAXApp {
            return handleAXEvent(type: type, keyCode: keyCode, event: event)
        }

        // --- Backspace ---
        if keyCode == 51 {
            // Always pass keyUp through so the OS sees the full key cycle
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }

            let (bs, out) = _engine.backspace()
            if bs == 0 && out.isEmpty {
                // Not composing — let OS handle it
                return Unmanaged.passRetained(event)
            }

            if isCompoundApp {
                // Step 1: invalidate autocomplete dropdown with empty char
                sendEmptyCharacter()
                // Step 2: OpenKey adds +1 backspace for compound apps
                let adjustedBs = bs + 1
                if isChromium {
                    // Chromium: Shift+Left select then overwrite
                    applySelectionBackspaces(adjustedBs)
                } else {
                    // Safari/Notes: normal backspace
                    applyBackspaces(adjustedBs)
                }
            } else {
                applyBackspaces(bs)
            }
            postText(out)
            return nil
        }

        // --- Space ---
        if keyCode == 49 {
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }
            if type == .keyDown {
                let (bs, out) = _engine.commit()
                if bs > 0 {
                    if isCompoundApp {
                        sendEmptyCharacter()
                        let adjustedBs = bs + 1
                        if isChromium {
                            applySelectionBackspaces(adjustedBs)
                        } else {
                            applyBackspaces(adjustedBs)
                        }
                    } else {
                        applyBackspaces(bs)
                    }
                }
                postText(out)
            }
            return Unmanaged.passRetained(event)
        }

        // --- Break keys (Enter, Tab, Arrows, etc.) ---
        if isBreakKey(keyCode) {
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }
            if type == .keyDown {
                let (bs, out) = _engine.commit()
                if bs > 0 {
                    if isCompoundApp {
                        sendEmptyCharacter()
                        let adjustedBs = bs + 1
                        if isChromium {
                            applySelectionBackspaces(adjustedBs)
                        } else {
                            applyBackspaces(adjustedBs)
                        }
                    } else {
                        applyBackspaces(bs)
                    }
                }
                postText(out)
            }
            return Unmanaged.passRetained(event)
        }

        // --- Regular character keys ---
        if type == .keyUp {
            return nil  // Suppress original keyUp; we already sent synthetic
        }

        guard let firstChar = characterFromCGEvent(event) else {
            return Unmanaged.passRetained(event)
        }

        let (bs, out) = _engine.feed(char: firstChar)
        if bs > 0 {
            if isCompoundApp {
                sendEmptyCharacter()
                let adjustedBs = bs + 1
                if isChromium {
                    applySelectionBackspaces(adjustedBs)
                } else {
                    applyBackspaces(adjustedBs)
                }
            } else {
                applyBackspaces(bs)
            }
        }
        postText(out)
        return nil
    }

    // MARK: - AX Mode (Accessibility text injection)

    private func handleAXEvent(type: CGEventType, keyCode: Int64, event: CGEvent) -> Unmanaged<CGEvent>? {
        // Backspace
        if keyCode == 51 {
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }
            let success = axInjector.backspace()
            return success ? nil : Unmanaged.passRetained(event)
        }

        // Space — commit and pass through
        if keyCode == 49 {
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }
            axInjector.commit()
            return Unmanaged.passRetained(event)
        }

        // Break keys — commit and pass through
        if isBreakKey(keyCode) {
            if type == .keyUp {
                return Unmanaged.passRetained(event)
            }
            axInjector.commit()
            return Unmanaged.passRetained(event)
        }

        // Regular character keys
        if type == .keyUp {
            return nil  // Suppress original keyUp
        }

        guard let firstChar = characterFromCGEvent(event) else {
            return Unmanaged.passRetained(event)
        }

        let success = axInjector.feed(char: firstChar)
        return success ? nil : Unmanaged.passRetained(event)
    }

    private func characterFromCGEvent(_ event: CGEvent) -> Character? {
        // Use `.characters` (not `.charactersIgnoringModifiers`) so that
        // Shift-held key events (e.g. Shift+A → 'A') preserve uppercase.
        if let nsEvent = NSEvent(cgEvent: event),
           let chars = nsEvent.characters,
           let firstChar = chars.first {
            return firstChar
        }
        var length: Int = 0
        var buffer = [UniChar](repeating: 0, count: 4)
        event.keyboardGetUnicodeString(maxStringLength: 4, actualStringLength: &length, unicodeString: &buffer)
        guard length > 0 else { return nil }
        return String(utf16CodeUnits: buffer, count: length).first
    }

    // MARK: - Synthetic Output

    /// Standard backspaces.
    private func applyBackspaces(_ count: Int) {
        guard let eventSource, count > 0 else { return }
        for _ in 0..<count {
            let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 51, keyDown: true)
            down?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
            down?.post(tap: .cghidEventTap)
            let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 51, keyDown: false)
            up?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
            up?.post(tap: .cghidEventTap)
        }
    }

    /// Chromium fix: Shift+Left Arrow to select, then type overwrites.
    private func applySelectionBackspaces(_ count: Int) {
        guard let eventSource, count > 0 else { return }
        for _ in 0..<count {
            let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 123, keyDown: true)
            down?.flags = .maskShift
            down?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
            down?.post(tap: .cghidEventTap)
            let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 123, keyDown: false)
            up?.flags = .maskShift
            up?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
            up?.post(tap: .cghidEventTap)
        }
    }

    /// Send U+202F (Narrow No-Break Space) to invalidate autocomplete dropdown.
    private func sendEmptyCharacter() {
        guard let eventSource else { return }
        let emptyChar: UniChar = 0x202F
        let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: true)
        down?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
        down?.keyboardSetUnicodeString(stringLength: 1, unicodeString: [emptyChar])
        down?.post(tap: .cghidEventTap)
        let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: false)
        up?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
        up?.post(tap: .cghidEventTap)
    }

    private func postText(_ string: String) {
        guard let eventSource, !string.isEmpty else { return }
        let utf16 = Array(string.utf16)
        guard !utf16.isEmpty else { return }
        let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: true)
        down?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
        down?.keyboardSetUnicodeString(stringLength: utf16.count, unicodeString: utf16)
        down?.post(tap: .cghidEventTap)
        let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: false)
        up?.setIntegerValueField(.eventSourceStateID, value: syntheticTag)
        up?.post(tap: .cghidEventTap)
    }
}
