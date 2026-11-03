#!/usr/bin/env python3
"""The iOS string catalog is complete (D-093): every key has English, Russian and Simplified
Chinese; Russian plurals have one, few, many and other; every key-shaped literal in the app's
Swift code (dotted lowercase words, like `welcome.create`) is in the catalog, and the catalog
holds no other keys. Accessibility identifiers, SF Symbol names and type identifiers share
the shape and are left out. Runs on
Linux, without Xcode."""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "ios"
CATALOG = ROOT / "EglViewer" / "Resources" / "Localizable.xcstrings"
LANGUAGES = ("en", "ru", "zh-Hans")
RU_PLURAL = {"one", "few", "many", "other"}
# A key: dotted lowercase words, optionally followed by interpolations (`%lld`, `%@`).
KEY = r'"([a-z]+(?:\.[A-Za-z]+)+(?: \\\([^)]*\))*)"'
LITERAL = re.compile(KEY)


def swift_key(literal: str) -> str:
    """`"library.adding \\(a) \\(b)"` → `library.adding %lld %lld` (integers only so far)."""
    return re.sub(r"\\\([^)]*\)", "%lld", literal)


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
            if "accessibilityIdentifier" in line or "systemName:" in line:
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
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        return 1
    print(f"iOS strings OK: {len(catalog)} keys in {len(LANGUAGES)} languages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
