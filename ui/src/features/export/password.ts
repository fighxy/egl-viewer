/** Minimum archive passphrase length (D-102; matches `ARCHIVE_PASSWORD_MIN_LEN` in Rust). */
export const ARCHIVE_PASSWORD_MIN_LEN = 8;

const COMMON = new Set([
  "password",
  "password1",
  "password12",
  "password123",
  "12345678",
  "123456789",
  "1234567890",
  "qwertyui",
  "qwerty123",
  "abcdefgh",
  "letmein1",
  "iloveyou",
  "admin123",
  "welcome1",
  "monkey12",
  "dragon12",
  "master12",
  "login123",
  "passw0rd",
  "changeme",
]);

/** Soft warning: short, one character class, or a well-known weak string (D-102). */
export function archivePasswordIsWeak(password: string): boolean {
  const chars = [...password];
  if (chars.length < 12) return true;
  let lower = false;
  let upper = false;
  let digit = false;
  let other = false;
  for (const c of chars) {
    if (c >= "a" && c <= "z") lower = true;
    else if (c >= "A" && c <= "Z") upper = true;
    else if (c >= "0" && c <= "9") digit = true;
    else other = true;
  }
  const classes = Number(lower) + Number(upper) + Number(digit) + Number(other);
  if (classes < 2) return true;
  return COMMON.has(password.toLowerCase());
}
