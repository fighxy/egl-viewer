import EglCore
import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// What a lock sheet does to a folder (D-098).
enum LockAction: Identifiable {
    /// Locks an open folder, or sets the first password of a lock imported without one.
    case lock(Folder)
    case unlock(Folder)
    /// Opens it with the recovery key or phrase and sets a new password.
    case recover(Folder)
    case changePassword(Folder)
    case renewKey(Folder)
    case removeLock(Folder)
    case label(Folder)

    var folder: Folder {
        switch self {
        case let .lock(folder), let .unlock(folder), let .recover(folder), let .changePassword(folder),
             let .renewKey(folder), let .removeLock(folder), let .label(folder):
            folder
        }
    }

    var id: String {
        let kind = switch self {
        case .lock: "lock"
        case .unlock: "unlock"
        case .recover: "recover"
        case .changePassword: "change"
        case .renewKey: "renew"
        case .removeLock: "remove"
        case .label: "label"
        }
        return "\(kind).\(folder.id)"
    }
}

/// One lock operation: the fields it needs, a wrong password said in place, and the recovery
/// key shown once when the operation makes one. The work runs off the main thread (Argon2id).
struct LockSheet: View {
    let open: OpenLibrary
    let action: LockAction
    @Environment(\.dismiss) private var dismiss
    @State private var password = ""
    @State private var confirm = ""
    @State private var current = ""
    @State private var recovery = ""
    @State private var label = ""
    @State private var color: String? = nil
    @State private var working = false
    @State private var wrong = false
    @State private var key: RecoveryKey? = nil
    /// The YubiKeys that also open this folder, for the unlock sheet (D-126).
    @State private var wraps: [PivWrap] = []
    @State private var yubiKeyPin = ""
    @State private var yubiKeyFailure: String?

    var body: some View {
        NavigationStack {
            Group {
                if let key {
                    RecoveryKeyView(key: key) { dismiss() }
                } else {
                    form
                }
            }
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .interactiveDismissDisabled(working || key != nil)
        }
        .onAppear {
            label = action.folder.lockLabel ?? ""
            color = action.folder.lockColor
        }
        .task {
            guard case let .unlock(folder) = action else { return }
            let id = folder.id
            wraps = await open.perform { try $0.pivWraps(id) } ?? []
        }
    }

    private var form: some View {
        Form {
            Section {
                Text(verbatim: action.folder.lockLabel ?? action.folder.name)
                    .font(.headline)
            } footer: {
                hint
            }
            fields
            if wrong {
                Section {
                    Label("lock.wrongPassword", systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                        .accessibilityIdentifier("lock.wrong")
                }
            }
        }
        .disabled(working)
        .overlay {
            if working { ProgressView() }
        }
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("common.cancel") { dismiss() }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button(confirmTitle, action: run)
                    .disabled(!ready || working)
                    .accessibilityIdentifier("lock.confirm")
            }
        }
    }

    @ViewBuilder
    private var fields: some View {
        switch action {
        case .lock:
            Section {
                SecureField("lock.password", text: $password)
                    .accessibilityIdentifier("lock.password")
                SecureField("lock.confirmPassword", text: $confirm)
                    .accessibilityIdentifier("lock.confirmPassword")
            }
            labelFields
        case .unlock, .removeLock, .renewKey:
            Section {
                SecureField("lock.password", text: $password)
                    .submitLabel(.go)
                    .onSubmit(run)
                    .accessibilityIdentifier("lock.password")
            }
            if case .unlock = action, !wraps.isEmpty {
                YubiKeyUnlockSection(wraps: wraps, pin: $yubiKeyPin, failure: yubiKeyFailure) { wrap in
                    unlockWithYubiKey(wrap)
                }
            }
        case .recover:
            Section {
                TextField("lock.recovery", text: $recovery, axis: .vertical)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .lineLimit(2...4)
                    .accessibilityIdentifier("lock.recovery")
            }
            Section {
                SecureField("lock.newPassword", text: $password)
                SecureField("lock.confirmPassword", text: $confirm)
            }
        case .changePassword:
            Section {
                SecureField("lock.currentPassword", text: $current)
            }
            Section {
                SecureField("lock.newPassword", text: $password)
                SecureField("lock.confirmPassword", text: $confirm)
            }
        case .label:
            labelFields
        }
    }

    private var labelFields: some View {
        Section {
            TextField("lock.label", text: $label)
                .accessibilityIdentifier("lock.label")
            ColorChoice(color: $color)
        } header: {
            Text("lock.labelTitle")
        } footer: {
            Text("lock.labelHint")
        }
    }

    private var hint: Text {
        switch action {
        case let .lock(folder) where folder.isLocked && !folder.hasPassword: Text("lock.setPasswordHint")
        case .lock: Text("lock.lockHint")
        case .unlock: Text("lock.unlockHint")
        case .recover: Text("lock.recoverHint")
        case .changePassword: Text("lock.changeHint")
        case .renewKey: Text("lock.renewHint")
        case .removeLock: Text("lock.removeHint")
        case .label: Text("lock.labelHint")
        }
    }

    private var title: Text {
        switch action {
        case let .lock(folder) where folder.isLocked && !folder.hasPassword: Text("lock.setPassword")
        case .lock: Text("lock.lock")
        case .unlock: Text("lock.unlock")
        case .recover: Text("lock.recover")
        case .changePassword: Text("lock.changePassword")
        case .renewKey: Text("lock.renewKey")
        case .removeLock: Text("lock.removeLock")
        case .label: Text("lock.labelTitle")
        }
    }

    private var confirmTitle: LocalizedStringKey {
        switch action {
        case .lock: "lock.lock"
        case .unlock, .recover: "lock.unlock"
        case .removeLock: "lock.remove"
        default: "common.save"
        }
    }

    /// The new password is the same twice where one is set.
    private var ready: Bool {
        switch action {
        case .lock, .recover:
            !password.isEmpty && password == confirm && (action.isRecover ? !recovery.isEmpty : true)
        case .changePassword:
            !current.isEmpty && !password.isEmpty && password == confirm
        case .unlock, .renewKey:
            !password.isEmpty
        case .removeLock:
            // A lock imported without a password is taken off with an empty one.
            !password.isEmpty || !action.folder.hasPassword
        case .label:
            true
        }
    }

    private func run() {
        guard ready, !working else { return }
        working = true
        wrong = false
        let id = action.folder.id
        let password = password
        let current = current
        let recovery = recovery.trimmingCharacters(in: .whitespacesAndNewlines)
        let label = label.trimmingCharacters(in: .whitespacesAndNewlines)
        let color = color
        Task {
            defer { working = false }
            switch action {
            case .lock:
                if case let .success(made) = await open.lockWork({
                    try $0.lockFolder(id, password: password, label: label.isEmpty ? nil : label, labelColor: color)
                }) {
                    key = made
                }
            case .unlock:
                finish(await open.lockWork { try $0.unlockFolder(id, password: password) })
            case .recover:
                finish(await open.lockWork { try $0.recoverFolder(id, recovery: recovery, newPassword: password) })
            case .changePassword:
                finish(await open.lockWork { try $0.changeFolderPassword(id, current: current, new: password) })
            case .renewKey:
                switch await open.lockWork({ try $0.renewRecoveryKey(id, password: password) }) {
                case let .success(made): key = made
                case let .failure(error): wrong = error.code == .wrongPassword
                }
            case .removeLock:
                finish(await open.lockWork { try $0.removeFolderLock(id, password: password) })
            case .label:
                finish(await open.lockWork { try $0.setLockLabel(id, label: label.isEmpty ? nil : label, color: color) })
            }
        }
    }

    /// The core's challenge, the key's answer, and the folder opens (D-126).
    private func unlockWithYubiKey(_ wrap: PivWrap) {
        guard !working else { return }
        working = true
        wrong = false
        yubiKeyFailure = nil
        let id = action.folder.id
        let pin = wrap.policy == .pin ? yubiKeyPin : nil
        Task {
            defer { working = false }
            guard let challenge = await open.perform({ try $0.pivChallenge(id, serial: wrap.serial) }) else { return }
            let shared: Data
            do {
                shared = try await YubiKeyCard.answer(challenge, serial: wrap.serial, pin: pin)
            } catch {
                yubiKeyFailure = error.localizedDescription
                return
            }
            switch await open.lockWork({ try $0.unlockWithPiv(id, serial: wrap.serial, sharedSecret: shared) }) {
            case .success:
                dismiss()
            case let .failure(error) where error.code == .wrongPassword:
                yubiKeyFailure = String(localized: "yubikey.notThisFolder")
            case .failure:
                break
            }
        }
    }

    private func finish(_ result: Result<Void, EglError>) {
        switch result {
        case .success:
            dismiss()
        case let .failure(error):
            wrong = error.code == .wrongPassword
        }
    }
}

private extension LockAction {
    var isRecover: Bool {
        if case .recover = self { return true }
        return false
    }
}

/// The recovery key, shown once: it is never stored, and it is the only way back into the
/// folder without its password.
struct RecoveryKeyView: View {
    let key: RecoveryKey
    let done: () -> Void
    @State private var saved = false

    var body: some View {
        Form {
            Section {
                Text("lock.keyHint")
            }
            Section("lock.phrase") {
                Text(verbatim: key.phrase)
                    .font(.body.monospaced())
                    .textSelection(.enabled)
                    .accessibilityIdentifier("lock.phraseText")
                Button {
                    Clipboard.copySecret(key.phrase)
                } label: {
                    Label("lock.copyPhrase", systemImage: "doc.on.doc")
                }
            }
            Section("lock.key") {
                Text(verbatim: key.key)
                    .font(.footnote.monospaced())
                    .textSelection(.enabled)
                Button {
                    Clipboard.copySecret(key.key)
                } label: {
                    Label("lock.copyKey", systemImage: "doc.on.doc")
                }
            }
            Section {
                Toggle("lock.savedIt", isOn: $saved)
                    .accessibilityIdentifier("lock.savedIt")
            }
        }
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("common.done", action: done)
                    .disabled(!saved)
                    .accessibilityIdentifier("lock.keyDone")
            }
        }
    }
}

extension Clipboard {
    /// Copies a secret for this device only, for two minutes.
    static func copySecret(_ text: String) {
        UIPasteboard.general.setItems(
            [[UTType.plainText.identifier: text]],
            options: [.localOnly: true, .expirationDate: Date().addingTimeInterval(120)]
        )
    }
}

