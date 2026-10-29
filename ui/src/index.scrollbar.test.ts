import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const css = readFileSync(resolve(process.cwd(), "src/index.css"), "utf8");

function block(selector: string): string {
  const start = css.indexOf(selector);
  expect(start).toBeGreaterThanOrEqual(0);
  const open = css.indexOf("{", start);
  const close = css.indexOf("}", open);
  return css.slice(open, close);
}

describe("scrollbar theme", () => {
  it("follows the app theme with the ring token, not a hardcoded colour", () => {
    expect(block(":root {")).toContain("color-scheme: light");
    expect(block(".dark {")).toContain("color-scheme: dark");
    expect(css).toContain("scrollbar-color: var(--ring) transparent");
    expect(css).toContain("background-color: var(--ring)");
    expect(css).toContain("background-color: var(--muted-foreground)");
    expect(css).not.toMatch(/scrollbar[^;\n]*#[0-9a-fA-F]{3,8}/);
    expect(css).not.toMatch(/::-webkit-scrollbar[^}]*#[0-9a-fA-F]{3,8}/);
  });
});
