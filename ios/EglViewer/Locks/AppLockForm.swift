import LocalAuthentication
import SwiftUI

/// The app's login (D-098): set a password or PIN, change it, turn it off, and Face ID. A page
/// of Settings (D-099).
struct AppLockForm: View {
    let lock: AppLock
    @State private var current = ""
    @State private var password = ""
    @State private var confirm = ""
    @State private var message: LocalizedStringKey? = nil
    @State private var failed = false

    var body: some View {
        Form {
            Section {
                Label(stateText, systemImage: stateSymbol)
                    .accessibilityIdentifier("appLock.state")
            } footer: {
                Text("appLock.hint")
            }
            if lock.enabled {
                Section("appLock.current") {
                    SecureField("appLock.currentPassword", text: $current)
                        .accessibilityIdentifier("appLock.current")
                }
            }
            Section(sectionText) {
                SecureField("appLock.newPassword", text: $password)
                    .accessibilityIdentifier("appLock.new")
                SecureField("appLock.confirmPassword", text: $confirm)
                    .accessibilityIdentifier("appLock.confirm")
                Button(saveText, action: save)
                    .disabled(password.isEmpty || password != confirm || (lock.enabled && current.isEmpty))
                    .accessibilityIdentifier("appLock.save")
            }
            if lock.enabled {
                if lock.biometry != .none {
                    Section {
                        Toggle(isOn: Binding(get: { lock.biometryEnabled }, set: setBiometry)) {
                            lock.biometry == .touchID ? Text("appLock.touchID") : Text("appLock.faceID")
                        }
                    }
                }
                Section {
                    Button("appLock.turnOff", role: .destructive, action: turnOff)
                        .disabled(current.isEmpty)
                        .accessibilityIdentifier("appLock.turnOff")
                }
            }
            if let message {
                Section {
                    Text(message)
                        .foregroundStyle(failed ? .red : EglTokens.mutedForeground)
                        .accessibilityIdentifier("appLock.message")
                }
            }
        }
        .navigationTitle("appLock.title")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var stateSymbol: String {
        lock.enabled ? "lock.fill" : "lock.open" // not a key
    }

    private var stateText: LocalizedStringKey { lock.enabled ? "appLock.on" : "appLock.off" }
    private var sectionText: LocalizedStringKey { lock.enabled ? "appLock.change" : "appLock.set" }
    private var saveText: LocalizedStringKey { lock.enabled ? "appLock.changeAction" : "appLock.setAction" }

    private func save() {
        do {
            if lock.enabled {
                guard try lock.change(current: current, to: password) else { return wrongCurrent() }
                say("appLock.changed")
            } else {
                try lock.setPassword(password)
                say("appLock.setDone")
            }
            current = ""
            password = ""
            confirm = ""
        } catch {
            say("appLock.failed", failed: true)
        }
    }

    private func turnOff() {
        do {
            guard try lock.turnOff(current: current) else { return wrongCurrent() }
            current = ""
            say("appLock.turnedOff")
        } catch {
            say("appLock.failed", failed: true)
        }
    }

    private func setBiometry(_ on: Bool) {
        do {
            try lock.setBiometry(on)
        } catch {
            say("appLock.failed", failed: true)
        }
    }

    private func wrongCurrent() {
        say("appLock.wrong", failed: true)
    }

    private func say(_ text: LocalizedStringKey, failed: Bool = false) {
        message = text
        self.failed = failed
    }
}
