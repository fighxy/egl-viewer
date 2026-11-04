import LocalAuthentication
import SwiftUI
import UIKit

/// The app's login over everything (D-098): the password or PIN, or Face ID / Touch ID.
struct LoginScreen: View {
    let lock: AppLock
    @State private var password = ""
    @State private var wrong = false
    @FocusState private var focused: Bool

    var body: some View {
        VStack(spacing: 20) {
            Spacer()
            Image(systemName: "lock.fill")
                .font(.system(size: 44))
                .foregroundStyle(EglTokens.mutedForeground)
                .accessibilityHidden(true)
            Text("login.title")
                .font(.title2.weight(.semibold))
            SecureField("login.password", text: $password)
                .textContentType(.password)
                .focused($focused)
                .submitLabel(.go)
                .onSubmit(unlock)
                .padding(12)
                .background(EglTokens.grid, in: RoundedRectangle(cornerRadius: 10))
                .frame(maxWidth: 320)
                .accessibilityIdentifier("login.password")
            if let wait = lock.waitUntil, wait > Date() {
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    let seconds = max(Int(wait.timeIntervalSince(context.date).rounded(.up)), 0)
                    Text("login.wait \(seconds)")
                        .font(.footnote)
                        .foregroundStyle(EglTokens.mutedForeground)
                }
            } else if wrong {
                Text("login.wrong")
                    .font(.footnote)
                    .foregroundStyle(.red)
                    .accessibilityIdentifier("login.wrong")
            }
            Button("login.unlock", action: unlock)
                .buttonStyle(.borderedProminent)
                .disabled(password.isEmpty)
                .accessibilityIdentifier("login.unlock")
            if lock.biometryEnabled {
                Button {
                    Task { await biometry() }
                } label: {
                    Label {
                        lock.biometry == .touchID ? Text("login.touchID") : Text("login.faceID")
                    } icon: {
                        Image(systemName: lock.biometry == .touchID ? "touchid" : "faceid")
                    }
                }
                .accessibilityIdentifier("login.biometry")
            }
            Spacer()
            Spacer()
        }
        .padding(24)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(EglTokens.background.ignoresSafeArea())
        .task {
            if lock.biometryEnabled {
                await biometry()
            } else {
                focused = true
            }
        }
    }

    private func unlock() {
        guard !password.isEmpty else { return }
        wrong = !lock.unlock(password)
        password = ""
    }

    private func biometry() async {
        if !(await lock.unlockWithBiometry(reason: String(localized: "login.biometryReason"))) {
            focused = true
        }
    }
}

/// What hides the app in the app switcher while a login is set.
struct PrivacyCover: View {
    var body: some View {
        ZStack {
            EglTokens.background.ignoresSafeArea()
            Image(systemName: "lock.fill")
                .font(.system(size: 44))
                .foregroundStyle(EglTokens.mutedForeground)
        }
        .accessibilityHidden(true)
    }
}

/// A window above every other one, sheets and full-screen covers included: the login and the
/// privacy cover go there, so nothing of the library shows behind them.
@MainActor
final class CoverWindow {
    private var window: UIWindow?

    func show<Content: View>(_ content: Content) {
        guard let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first else { return }
        let window = self.window ?? UIWindow(windowScene: scene)
        window.windowLevel = .alert + 1
        window.rootViewController = UIHostingController(rootView: content)
        window.isHidden = false
        window.makeKeyAndVisible()
        self.window = window
    }

    func hide() {
        window?.isHidden = true
        window = nil
    }
}
