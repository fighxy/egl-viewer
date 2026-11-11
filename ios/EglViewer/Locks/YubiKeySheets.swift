import EglCore
import SwiftUI

/// Unlocking with a YubiKey from the unlock sheet (D-126): one button per bound key, and the
/// key's PIV PIN when its wrap asks for it. A touch wrap makes the key blink until touched.
struct YubiKeyUnlockSection: View {
    let wraps: [PivWrap]
    @Binding var pin: String
    let failure: String?
    let unlock: (PivWrap) -> Void

    var body: some View {
        Section {
            if wraps.contains(where: { $0.policy == .pin }) {
                SecureField("yubikey.pin", text: $pin)
                    .keyboardType(.numberPad)
                    .accessibilityIdentifier("yubikey.pin")
            }
            ForEach(wraps, id: \.serial) { wrap in
                Button {
                    unlock(wrap)
                } label: {
                    Label {
                        Text("yubikey.unlockWith") + Text(verbatim: " · \(wrap.serial)")
                    } icon: {
                        Image(systemName: "key.horizontal")
                    }
                }
                .disabled(wrap.policy == .pin && !(6...8).contains(pin.count))
                .accessibilityIdentifier("yubikey.unlock.\(wrap.serial)")
            }
            if let failure {
                Label {
                    Text(verbatim: failure)
                } icon: {
                    Image(systemName: "exclamationmark.triangle")
                }
                .foregroundStyle(.red)
            }
        } header: {
            Text("yubikey.title")
        } footer: {
            Text("yubikey.unlockHint")
        }
    }
}

/// The YubiKeys of one locked folder (D-126): the bound ones, removing one, and binding the
/// key plugged in now. Both take the folder's password; the password and the recovery key go
/// on opening the folder either way.
struct YubiKeysSheet: View {
    let open: OpenLibrary
    let folder: Folder
    @Environment(\.dismiss) private var dismiss
    @State private var wraps: [PivWrap] = []
    @State private var password = ""
    @State private var policy: PivPolicy = .touch
    @State private var pin = ""
    @State private var working = false
    @State private var failure: String?

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Text(verbatim: folder.lockLabel ?? folder.name)
                        .font(.headline)
                } footer: {
                    Text("yubikey.manageHint")
                }
                if !wraps.isEmpty {
                    Section("yubikey.bound") {
                        ForEach(wraps, id: \.serial) { wrap in
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(verbatim: wrap.serial)
                                    Text(wrap.policy == .pin ? "yubikey.policy.pin" : "yubikey.policy.touch")
                                        .font(.caption)
                                        .foregroundStyle(EglTokens.mutedForeground)
                                }
                                Spacer()
                                Button("yubikey.remove", role: .destructive) { remove(wrap) }
                                    .buttonStyle(.borderless)
                                    .disabled(password.isEmpty || working)
                                    .accessibilityIdentifier("yubikey.remove.\(wrap.serial)")
                            }
                        }
                    }
                }
                Section {
                    SecureField("lock.password", text: $password)
                        .accessibilityIdentifier("yubikey.password")
                } footer: {
                    Text("yubikey.passwordHint")
                }
                if !open.readOnly {
                    Section {
                        Picker("yubikey.policy", selection: $policy) {
                            Text("yubikey.policy.touch").tag(PivPolicy.touch)
                            Text("yubikey.policy.pin").tag(PivPolicy.pin)
                        }
                        .pickerStyle(.segmented)
                        if policy == .pin {
                            SecureField("yubikey.pin", text: $pin)
                                .keyboardType(.numberPad)
                        }
                        Button("yubikey.add") { add() }
                            .disabled(!readyToAdd || working)
                            .accessibilityIdentifier("yubikey.add")
                    } header: {
                        Text("yubikey.addTitle")
                    } footer: {
                        Text("yubikey.addHint")
                    }
                }
                if let failure {
                    Section {
                        Label {
                            Text(verbatim: failure)
                        } icon: {
                            Image(systemName: "exclamationmark.triangle")
                        }
                        .foregroundStyle(.red)
                    }
                }
            }
            .disabled(working)
            .overlay {
                if working { ProgressView() }
            }
            .navigationTitle("yubikey.title")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("common.done") { dismiss() }
                }
            }
            .task { await reload() }
        }
    }

    private var readyToAdd: Bool {
        !password.isEmpty && (policy == .touch || (6...8).contains(pin.count))
    }

    private func reload() async {
        let id = folder.id
        wraps = await open.perform { try $0.pivWraps(id) } ?? []
    }

    private func add() {
        working = true
        failure = nil
        let (id, password, policy, pin) = (folder.id, password, policy, pin)
        Task {
            defer { working = false }
            let card: (serial: String, slot: UInt8, publicKey: Data)
            do {
                card = try await YubiKeyCard.enroll(policy: policy, pin: policy == .pin ? pin : nil)
            } catch {
                failure = error.localizedDescription
                return
            }
            switch await open.lockWork({
                try $0.bindPiv(
                    id, password: password, serial: card.serial, slot: card.slot, policy: policy,
                    publicKey: card.publicKey
                )
            }) {
            case .success:
                self.pin = ""
                await reload()
                open.show(String(localized: "yubikey.added"), undoable: false)
            case let .failure(error) where error.code == .wrongPassword:
                failure = String(localized: "lock.wrongPassword")
            case .failure:
                // Anything else was shown by the library's alert already.
                break
            }
        }
    }

    private func remove(_ wrap: PivWrap) {
        working = true
        failure = nil
        let (id, password) = (folder.id, password)
        Task {
            defer { working = false }
            switch await open.lockWork({ try $0.unbindPiv(id, password: password, serial: wrap.serial) }) {
            case .success:
                await reload()
            case let .failure(error) where error.code == .wrongPassword:
                failure = String(localized: "lock.wrongPassword")
            case .failure:
                // Anything else was shown by the library's alert already.
                break
            }
        }
    }
}
