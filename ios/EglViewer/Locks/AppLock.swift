import CommonCrypto
import Foundation
import LocalAuthentication
import Observation
import Security

/// Where the app's login lives: a salted PBKDF2 hash of the password, and a token that only
/// Face ID or Touch ID can read (D-098). The Keychain on the phone; memory in tests.
protocol LoginStore: AnyObject {
    func loadHash() -> LoginHash?
    func saveHash(_ hash: LoginHash?) throws
    var hasBiometricToken: Bool { get }
    func saveBiometricToken(_ on: Bool) throws
    /// Reads the token, which asks for Face ID; true when the face (or finger) matched.
    func readBiometricToken(reason: String) async -> Bool
}

/// PBKDF2-HMAC-SHA256 of the password with its own salt.
struct LoginHash: Codable, Equatable {
    var salt: Data
    var hash: Data
    var iterations: Int

    static let defaultIterations = 310_000

    static func make(_ password: String, iterations: Int = defaultIterations) -> LoginHash {
        var salt = Data(count: 16)
        _ = salt.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 16, $0.baseAddress!) }
        return LoginHash(salt: salt, hash: derive(password, salt: salt, iterations: iterations), iterations: iterations)
    }

    func matches(_ password: String) -> Bool {
        let given = Self.derive(password, salt: salt, iterations: iterations)
        // Constant time: every byte is compared.
        guard given.count == hash.count else { return false }
        return zip(given, hash).reduce(0) { $0 | ($1.0 ^ $1.1) } == 0
    }

    static func derive(_ password: String, salt: Data, iterations: Int) -> Data {
        let secret = Array(password.utf8)
        var derived = [UInt8](repeating: 0, count: 32)
        salt.withUnsafeBytes { saltBytes in
            _ = CCKeyDerivationPBKDF(
                CCPBKDFAlgorithm(kCCPBKDF2),
                secret.map { CChar(bitPattern: $0) }, secret.count,
                saltBytes.bindMemory(to: UInt8.self).baseAddress, salt.count,
                CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA256), UInt32(iterations),
                &derived, derived.count
            )
        }
        return Data(derived)
    }
}

/// The app's lock: whether a login is set, whether the app is locked now, and failed tries.
@MainActor
@Observable
final class AppLock {
    private(set) var enabled: Bool
    private(set) var locked: Bool
    private(set) var biometryEnabled: Bool
    /// Until then the password field waits, after too many wrong tries.
    private(set) var waitUntil: Date?
    @ObservationIgnored private var failures = 0
    @ObservationIgnored private let store: LoginStore
    @ObservationIgnored private let iterations: Int

    init(store: LoginStore, iterations: Int = LoginHash.defaultIterations) {
        self.store = store
        self.iterations = iterations
        enabled = store.loadHash() != nil
        locked = store.loadHash() != nil
        biometryEnabled = store.hasBiometricToken
    }

    /// The Keychain on the phone; memory for UI tests, so they never touch the real login.
    static func standard(arguments: [String] = ProcessInfo.processInfo.arguments) -> AppLock {
        arguments.contains("-uiTesting") ? AppLock(store: MemoryLoginStore(), iterations: 1000) : AppLock(store: KeychainLoginStore())
    }

    /// Face ID, Touch ID or none on this device.
    var biometry: LABiometryType {
        let context = LAContext()
        _ = context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: nil)
        return context.biometryType
    }

    func lock() {
        if enabled { locked = true }
    }

    func setPassword(_ password: String) throws {
        try store.saveHash(LoginHash.make(password, iterations: iterations))
        enabled = true
    }

    /// False when `current` is wrong.
    func change(current: String, to password: String) throws -> Bool {
        guard check(current) else { return false }
        try store.saveHash(LoginHash.make(password, iterations: iterations))
        return true
    }

    /// Turns the login off; false when `current` is wrong.
    func turnOff(current: String) throws -> Bool {
        guard check(current) else { return false }
        try store.saveBiometricToken(false)
        try store.saveHash(nil)
        enabled = false
        biometryEnabled = false
        locked = false
        return true
    }

    func setBiometry(_ on: Bool) throws {
        try store.saveBiometricToken(on && enabled)
        biometryEnabled = store.hasBiometricToken
    }

    /// True when the password opened the app.
    func unlock(_ password: String) -> Bool {
        guard check(password) else { return false }
        locked = false
        return true
    }

    func unlockWithBiometry(reason: String) async -> Bool {
        guard biometryEnabled, await store.readBiometricToken(reason: reason) else { return false }
        failures = 0
        waitUntil = nil
        locked = false
        return true
    }

    /// Checks a password, counting wrong tries: from the fifth on, each makes the next wait
    /// longer (30 s, 60 s, …).
    private func check(_ password: String) -> Bool {
        if let waitUntil, waitUntil > Date() { return false }
        guard let hash = store.loadHash() else { return true }
        if hash.matches(password) {
            failures = 0
            waitUntil = nil
            return true
        }
        failures += 1
        if failures >= 5 {
            waitUntil = Date().addingTimeInterval(Double(failures - 4) * 30)
        }
        return false
    }
}

/// The login in the Keychain, on this device only, readable once the phone is unlocked.
final class KeychainLoginStore: LoginStore {
    private let service: String

    init(service: String = "app.eglviewer.login") { // not a key
        self.service = service
    }

    private func query(_ account: String) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account]
    }

    func loadHash() -> LoginHash? {
        var query = query("password")
        query[kSecReturnData as String] = true
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess, let data = result as? Data else { return nil }
        return try? JSONDecoder().decode(LoginHash.self, from: data)
    }

    func saveHash(_ hash: LoginHash?) throws {
        SecItemDelete(query("password") as CFDictionary)
        guard let hash else { return }
        var item = query("password")
        item[kSecValueData as String] = try JSONEncoder().encode(hash)
        item[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        try Self.check(SecItemAdd(item as CFDictionary, nil))
    }

    var hasBiometricToken: Bool {
        var query = query("biometry")
        let context = LAContext()
        context.interactionNotAllowed = true
        query[kSecUseAuthenticationContext as String] = context
        query[kSecReturnAttributes as String] = true
        let status = SecItemCopyMatching(query as CFDictionary, nil)
        // Present but asking for Face ID counts as present.
        return status == errSecSuccess || status == errSecInteractionNotAllowed
    }

    func saveBiometricToken(_ on: Bool) throws {
        SecItemDelete(query("biometry") as CFDictionary)
        guard on else { return }
        guard let access = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly, .biometryCurrentSet, nil
        ) else { throw Self.error(errSecParam) }
        var token = Data(count: 32)
        _ = token.withUnsafeMutableBytes { SecRandomCopyBytes(kSecRandomDefault, 32, $0.baseAddress!) }
        var item = query("biometry")
        item[kSecValueData as String] = token
        item[kSecAttrAccessControl as String] = access
        try Self.check(SecItemAdd(item as CFDictionary, nil))
    }

    func readBiometricToken(reason: String) async -> Bool {
        let service = service
        return await Task.detached {
            let context = LAContext()
            context.localizedReason = reason
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service,
                kSecAttrAccount as String: "biometry", kSecReturnData as String: true,
                kSecUseAuthenticationContext as String: context,
            ]
            return SecItemCopyMatching(query as CFDictionary, nil) == errSecSuccess
        }.value
    }

    private static func check(_ status: OSStatus) throws {
        guard status == errSecSuccess else { throw error(status) }
    }

    private static func error(_ status: OSStatus) -> NSError {
        NSError(domain: NSOSStatusErrorDomain, code: Int(status))
    }
}

/// A login that lives as long as the process: UI tests.
final class MemoryLoginStore: LoginStore {
    private var hash: LoginHash?
    private var token = false

    func loadHash() -> LoginHash? { hash }
    func saveHash(_ hash: LoginHash?) throws { self.hash = hash }
    var hasBiometricToken: Bool { token }
    func saveBiometricToken(_ on: Bool) throws { token = on }
    func readBiometricToken(reason: String) async -> Bool { token }
}
