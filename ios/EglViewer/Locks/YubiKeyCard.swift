import EglCore
import ExternalAccessory
import Foundation
import YubiKit

/// The YubiKey as a PIV card on the iPhone (D-126), through YubiKit: a YubiKey 5C in the USB-C
/// port (iPhone 15 and later), or a 5Ci on Lightning. The card does the P-256 ECDH the core asks
/// for; its private key never leaves it, and the core never sees the card.
enum YubiKeyCard {
    /// What went wrong, in words the lock sheets show.
    enum Failure: LocalizedError, Equatable {
        case absent
        case otherCard
        case wrongPin(retries: Int)
        case pinLocked
        case managementKey
        case noFreeSlot
        case unreadable

        var errorDescription: String? {
            switch self {
            case .absent: String(localized: "yubikey.absent")
            case .otherCard: String(localized: "yubikey.otherCard")
            case let .wrongPin(retries): String(localized: "yubikey.wrongPin \(retries)")
            case .pinLocked: String(localized: "yubikey.pinLocked")
            case .managementKey: String(localized: "yubikey.managementKey")
            case .noFreeSlot: String(localized: "yubikey.noFreeSlot")
            case .unreadable: String(localized: "yubikey.unreadable")
            }
        }
    }

    /// The factory PIV management key (3DES, or AES-192 on firmware 5.7), the one desktop
    /// enrollment uses too. A key changed from it is refused with a plain reason.
    private static let defaultManagementKey = Data([
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
    ])

    /// The retired slots `age-plugin-yubikey` uses, 0x82–0x95.
    private static let retired: [UInt8] = Array(0x82...0x95)

    /// The YubiKey plugged in now: USB-C first, then Lightning. Nothing plugged in is `absent`
    /// at once, rather than a wait without end.
    private static func connect() async throws -> any SmartCardConnection {
        if let device = try? await USBSmartCardConnection.availableDevices().first {
            return try await USBSmartCardConnection(slot: device)
        }
        let lightning = EAAccessoryManager.shared().connectedAccessories.contains {
            $0.protocolStrings.contains("com.yubico.ylp") // not a key: the 5Ci accessory protocol
        }
        if lightning {
            return try await LightningSmartCardConnection()
        }
        throw Failure.absent
    }

    /// Runs `work` on a PIV session of the plugged-in key and closes the connection after.
    private static func withSession<T>(_ work: (PIVSession) async throws -> T) async throws -> T {
        let connection = try await connect()
        do {
            let session = try await PIVSession.makeSession(connection: connection)
            let value = try await work(session)
            await connection.close(error: nil)
            return value
        } catch {
            await connection.close(error: error)
            throw error
        }
    }

    private static func verify(_ pin: String, on session: PIVSession) async throws {
        switch try await session.verifyPin(pin) {
        case .success: return
        case let .fail(retries): throw Failure.wrongPin(retries: retries)
        case .pinLocked: throw Failure.pinLocked
        }
    }

    /// The card's answer to the core's challenge: P-256 ECDH in the challenge's slot, after the
    /// PIN for a PIN wrap (a touch wrap makes the card wait for the touch). `serial` must be
    /// the card the wrap was made for.
    static func answer(_ challenge: PivChallenge, serial: String, pin: String?) async throws -> Data {
        guard let slot = PIV.Slot(rawValue: challenge.slot),
              let peer = EC.PublicKey(x963: challenge.ephemeral, curve: .secp256r1)
        else { throw Failure.unreadable }
        return try await withSession { session in
            let found = String(try await session.getSerialNumber())
            guard found == serial else { throw Failure.otherCard }
            if challenge.policy == .pin {
                try await verify(pin ?? "", on: session)
            }
            return try await session.deriveSharedSecret(in: slot, with: peer)
        }
    }

    /// A new P-256 key in the first free retired slot, asking for a touch or for the PIN on
    /// each use, as the desktop makes it (D-056). Returns what binding the folder needs.
    static func enroll(policy: PivPolicy, pin: String?) async throws -> (serial: String, slot: UInt8, publicKey: Data) {
        try await withSession { session in
            let serial = String(try await session.getSerialNumber())
            do {
                try await session.authenticate(with: defaultManagementKey)
            } catch {
                throw Failure.managementKey
            }
            if policy == .pin {
                try await verify(pin ?? "", on: session)
            }
            var free: PIV.Slot?
            for raw in retired {
                guard let slot = PIV.Slot(rawValue: raw) else { continue }
                // An empty slot has no metadata: the card answers with an error.
                if (try? await session.getMetadata(in: slot)) == nil {
                    free = slot
                    break
                }
            }
            guard let slot = free else { throw Failure.noFreeSlot }
            let key = try await session.generateKey(
                in: slot,
                type: .ec(.secp256r1),
                pinPolicy: policy == .pin ? .always : .never,
                touchPolicy: policy == .touch ? .always : .never
            )
            guard case let .ec(publicKey) = key else { throw Failure.unreadable }
            return (serial, slot.rawValue, publicKey.x963)
        }
    }
}
