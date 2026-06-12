import Cocoa
import Combine

/// Manages Vietnamese/English toggle, hotkeys, and per-app state.
final class InputMethodManager: ObservableObject {
    @Published var isVietnamese = true
    @Published var currentAppBundleID = ""

    private var smartSwitch: SmartSwitchManager?
    private var cancellables = Set<AnyCancellable>()

    var inputMethod: InputMethod {
        get {
            let raw = UserDefaults.standard.string(forKey: DefaultsKey.inputMethod) ?? "telex"
            return raw == "vni" ? .vni : .telex
        }
        set {
            UserDefaults.standard.set(newValue.rawValue, forKey: DefaultsKey.inputMethod)
        }
    }

    init(smartSwitch: SmartSwitchManager? = nil) {
        self.smartSwitch = smartSwitch
        setupAppSwitchObserver()
    }

    func toggle() {
        isVietnamese.toggle()
        saveCurrentAppState()
    }

    func setVietnamese(_ value: Bool) {
        guard isVietnamese != value else { return }
        isVietnamese = value
        saveCurrentAppState()
    }

    // MARK: - App Switch

    private func setupAppSwitchObserver() {
        NSWorkspace.shared.notificationCenter
            .publisher(for: NSWorkspace.didActivateApplicationNotification)
            .receive(on: DispatchQueue.main)
            .sink { [weak self] notification in
                guard let app = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else { return }
                self?.handleAppSwitch(to: app.bundleIdentifier ?? "")
            }
            .store(in: &cancellables)
    }

    private func handleAppSwitch(to bundleID: String) {
        guard !bundleID.isEmpty else { return }

        // Save state for previous app
        saveCurrentAppState()

        currentAppBundleID = bundleID

        // Restore state for new app
        if let smartSwitch, let state = smartSwitch.state(for: bundleID) {
            isVietnamese = state.language
            // Code table could be applied here if needed
        }
    }

    private func saveCurrentAppState() {
        guard !currentAppBundleID.isEmpty else { return }
        smartSwitch?.setState(language: isVietnamese, for: currentAppBundleID)
    }
}
