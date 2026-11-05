import { describe, expect, it } from "vitest";
import { ARCHIVE_PASSWORD_MIN_LEN, archivePasswordIsWeak } from "./password";

describe("archive password policy", () => {
  it("sets the floor at eight characters", () => {
    expect(ARCHIVE_PASSWORD_MIN_LEN).toBe(8);
  });

  it("flags short or one-class passwords as weak", () => {
    expect(archivePasswordIsWeak("12345678")).toBe(true);
    expect(archivePasswordIsWeak("abcdefghijkl")).toBe(true);
    expect(archivePasswordIsWeak("password12")).toBe(true);
  });

  it("accepts a mixed longer passphrase", () => {
    expect(archivePasswordIsWeak("Correct-Horse-1")).toBe(false);
  });
});
