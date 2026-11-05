@testable import EglViewer
import XCTest

/// What a release build carries (D-100): the icon, the privacy manifest and the privacy
/// strings, translated.
final class ReleaseTests: XCTestCase {
    private var info: [String: Any] { Bundle.main.infoDictionary ?? [:] }

    func testTheAppHasItsIcon() throws {
        let icons = try XCTUnwrap(info["CFBundleIcons"] as? [String: Any], "the asset catalog adds the icon")
        let primary = try XCTUnwrap(icons["CFBundlePrimaryIcon"] as? [String: Any])
        XCTAssertEqual(primary["CFBundleIconName"] as? String, "AppIcon")
    }

    func testThePrivacyManifestDeclaresNoTrackingAndItsReasons() throws {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "PrivacyInfo", withExtension: "xcprivacy"))
        let manifest = try XCTUnwrap(
            PropertyListSerialization.propertyList(from: Data(contentsOf: url), format: nil) as? [String: Any]
        )
        XCTAssertEqual(manifest["NSPrivacyTracking"] as? Bool, false)
        let types = try XCTUnwrap(manifest["NSPrivacyAccessedAPITypes"] as? [[String: Any]])
        XCTAssertEqual(
            Set(types.compactMap { $0["NSPrivacyAccessedAPIType"] as? String }),
            ["NSPrivacyAccessedAPICategoryUserDefaults", "NSPrivacyAccessedAPICategoryFileTimestamp"]
        )
    }

    func testThePrivacyStringsAreTranslated() throws {
        for key in ["NSFaceIDUsageDescription", "NSPhotoLibraryAddUsageDescription"] {
            XCTAssertNotNil(info[key] as? String, key)
            for language in ["ru", "zh-Hans"] {
                let path = try XCTUnwrap(Bundle.main.path(forResource: language, ofType: "lproj"), language)
                let bundle = try XCTUnwrap(Bundle(path: path))
                let text = bundle.localizedString(forKey: key, value: nil, table: "InfoPlist")
                XCTAssertNotEqual(text, key, "\(key) in \(language)")
                XCTAssertNotEqual(text, info[key] as? String, "\(key) in \(language) is not English")
            }
        }
    }
}
