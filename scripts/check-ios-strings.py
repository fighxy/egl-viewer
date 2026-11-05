#!/usr/bin/env python3
"""The iOS string catalog is complete (D-093): every key has English, Russian and Simplified
Chinese; Russian plurals have one, few, many and other; every key-shaped literal in the app's
Swift code (dotted lowercase words, like `welcome.create`) is in the catalog, and the catalog
holds no other keys. Accessibility identifiers, SF Symbol names and type identifiers share
the shape and are left out. Runs on
Linux, without Xcode. The privacy strings of Info.plist have the three languages too."""

import json
import pathlib
import plistlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "ios"
CATALOG = ROOT / "EglViewer" / "Resources" / "Localizable.xcstrings"
INFO_PLIST = ROOT / "EglViewer" / "Resources" / "Info.plist"
INFO_CATALOG = ROOT / "EglViewer" / "Resources" / "InfoPlist.xcstrings"
LANGUAGES = ("en", "ru", "zh-Hans")
RU_PLURAL = {"one", "few", "many", "other"}
# A key: dotted lowercase words, optionally followed by interpolations (`%lld`, `%@`).
KEY = r'"([a-z][A-Za-z]*(?:\.[A-Za-z]+)+(?: \\\([^)]*\))*)"'
LITERAL = re.compile(KEY)


def swift_key(literal: str) -> str:
    """`"library.adding \\(a) \\(b)"` → `library.adding %lld %lld` (integers only so far)."""
    return re.sub(r"\\\([^)]*\)", "%lld", literal)


def info_plist_problems() -> list[str]:
    """Every privacy string of Info.plist (`NS…UsageDescription`) is in InfoPlist.xcstrings in
    the three languages, with the English text of the plist."""
    plist = plistlib.loads(INFO_PLIST.read_bytes())
    catalog = json.loads(INFO_CATALOG.read_text(encoding="utf-8"))["strings"]
    problems = []
    usage = {key for key in plist if key.startswith("NS") and key.endswith("UsageDescription")}
    if not usage:
        problems.append("Info.plist: no privacy strings")
    for key in sorted(usage):
        localizations = catalog.get(key, {}).get("localizations", {})
        for language in LANGUAGES:
            if language not in localizations:
                problems.append(f"InfoPlist.xcstrings: {key} has no {language}")
        english = localizations.get("en", {}).get("stringUnit", {}).get("value")
        if english is not None and english != plist[key]:
            problems.append(f"InfoPlist.xcstrings: {key} differs from Info.plist in English")
    for key in sorted(set(catalog) - set(plist)):
        problems.append(f"InfoPlist.xcstrings: {key} is not in Info.plist")
    return problems


def main() -> int:
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))["strings"]
    problems = []
    for key, entry in catalog.items():
        localizations = entry.get("localizations", {})
        for language in LANGUAGES:
            if language not in localizations:
                problems.append(f"{key}: no {language}")
        plural = localizations.get("ru", {}).get("variations", {}).get("plural")
        if plural is not None and set(plural) != RU_PLURAL:
            problems.append(f"{key}: Russian plural forms {sorted(plural)}")
    used = set()
    for path in (ROOT / "EglViewer").rglob("*.swift"):
        for line in path.read_text(encoding="utf-8").splitlines():
            # Identifiers and SF Symbol names (`Image(systemName:)`, also in a ternary).
            # A line can say it holds no keys: `// not a key` (a Keychain service, a symbol).
            if "accessibilityIdentifier" in line or "systemName:" in line or "// not a key" in line:
                continue
            for match in LITERAL.finditer(line):
                before = line[: match.start()].rstrip()
                # SF Symbols and type identifiers share the shape.
                if before.endswith(("systemImage:", "exportedAs:", "importedAs:")):
                    continue
                used.add(swift_key(match.group(1)))
    for key in sorted(used - set(catalog)):
        problems.append(f"used but not in the catalog: {key}")
    for key in sorted(set(catalog) - used):
        problems.append(f"in the catalog but unused: {key}")
    if not used:
        problems.append("no keys found in the Swift sources")
    problems += info_plist_problems()
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    print(f"iOS strings OK: {len(catalog)} keys in {len(LANGUAGES)} languages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
