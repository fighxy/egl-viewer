@testable import EglViewer
import EglCore
import XCTest

/// D-126: what the lock sheets say when the YubiKey cannot answer. The card itself is not
/// here: the unlock and the binding against the core are tested with a software P-256 key in
/// EglCore and library-ffi.
final class YubiKeyTests: XCTestCase {
    func testEveryFailureSaysWhatToDo() {
        let failures: [YubiKeyCard.Failure] = [
            .absent, .nfcStopped, .otherCard, .wrongPin(retries: 2), .pinLocked, .managementKey, .noFreeSlot, .unreadable,
        ]
        for failure in failures {
            let text = failure.errorDescription ?? ""
            XCTAssertFalse(text.isEmpty, "\(failure)")
            XCTAssertFalse(text.hasPrefix("yubikey."), "\(failure) is translated: \(text)")
        }
        XCTAssertTrue(YubiKeyCard.Failure.wrongPin(retries: 2).errorDescription?.contains("2") == true)
    }

    func testTheAppDeclaresTheYubiKeyConnections() throws {
        let protocols = Bundle.main.object(forInfoDictionaryKey: "UISupportedExternalAccessoryProtocols") as? [String]
        XCTAssertEqual(protocols, ["com.yubico.ylp"], "the 5Ci on Lightning")
        let applets = Bundle.main.object(
            forInfoDictionaryKey: "com.apple.developer.nfc.readersession.iso7816.select-identifiers"
        ) as? [String]
        XCTAssertEqual(applets, ["A000000308"], "the PIV applet over NFC")
        XCTAssertNotNil(Bundle.main.object(forInfoDictionaryKey: "NFCReaderUsageDescription"))
    }
}
