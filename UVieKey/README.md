# UVieKey

Fast, lightweight Vietnamese input method for macOS. Built on the `uvie-rs` Rust engine with a native SwiftUI app layer.

## Architecture

```
┌─────────────────────────────────────┐
│         UVieKey (Swift)             │
│  • CGEventTap, Menu Bar, Settings   │
│  • Smart Switch, Macro, Onboarding  │
├─────────────────────────────────────┤
│         EngineBridge (FFI)          │
│  • uvie.h C ABI wrapper           │
├─────────────────────────────────────┤
│         uvie-rs (Rust)              │
│  • Telex/VNI, Tone Placement      │
│  • Spelling, Quick Input           │
└─────────────────────────────────────┘
```

## Features

- **Input Methods:** Telex, VNI
- **Quick Input:** Quick Start (`j→gi`), Quick Telex (`cc→ch`)
- **Smart Features:** Per-app language memory, macro expansion
- **Modern Orthography:** Defaults to `oà`, `uý` style
- **Zero Allocations:** Rust engine uses no heap in hot path
- **Backspace-Replacement:** Clean composing without IMK limitations

## Build Requirements

- macOS 13+
- Xcode 15+ (Swift 5.9+)
- Rust toolchain (for uvie-rs)

## Build

```bash
# 1. Build uvie-rs library
cd ../ && cargo build --release

# 2. Build UVieKey app
cd UVieKey
chmod +x build.sh
./build.sh

# 3. Run (with uvie library path)
swift run \
    -Xswiftc -I../include \
    -Xswiftc -L../target/release \
    -Xlinker -luvie
```

## Xcode Integration

For full app packaging (entitlements, code signing, notarization), create an Xcode project:

```bash
swift package generate-xcodeproj
```

Then configure:
- **Signing & Capabilities:** Add your Developer ID
- **Entitlements:** Use `UVieKey.entitlements` (disable sandbox for Accessibility)
- **Info.plist:** Use `Info.plist` (set `LSUIElement` = true for menu bar only)

## Permissions

UVieKey requires **Accessibility** permission to intercept keystrokes.

1. Launch UVieKey
2. Click "Grant Permission" in onboarding
3. Add UVieKey to **System Settings → Privacy & Security → Accessibility**

## Development

### Project Structure

```
UVieKey/
├── Sources/UVieKey/
│   ├── App/
│   │   └── UVieKeyApp.swift          # @main entry point
│   ├── Core/
│   │   ├── EngineBridge.swift        # FFI wrapper
│   │   ├── EventTap.swift            # CGEventTap handler
│   │   └── TextDiff.swift            # Backspace-replacement
│   ├── UI/
│   │   ├── MenuBarController.swift   # Status bar icon
│   │   ├── SettingsWindow.swift     # Preferences
│   │   └── OnboardingView.swift     # First-launch flow
│   ├── Features/
│   │   └── InputMethodManager.swift # Toggle + per-app memory
│   └── Utils/
│       ├── AccessibilityChecker.swift # Permission helper
│       └── Defaults.swift           # UserDefaults keys
├── Package.swift
├── Info.plist
├── UVieKey.entitlements
└── build.sh
```

## License

MIT
