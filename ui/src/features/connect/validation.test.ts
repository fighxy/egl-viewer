import { describe, expect, it } from "vitest";
import { isValid, validateConnectForm } from "./validation";

const form = (host: string, port: string) => ({ host, port, token: "" });

describe("validateConnectForm", () => {
  it("accepts the defaults and ordinary addresses", () => {
    for (const host of [
      "127.0.0.1",
      "localhost",
      "desktop-pc",
      "192.168.1.20",
      "pc.tail1.ts.net",
    ]) {
      expect(isValid(validateConnectForm(form(host, "41595"))), host).toBe(true);
    }
  });

  it("requires a host", () => {
    expect(validateConnectForm(form("", "41595")).host).toBeDefined();
    expect(validateConnectForm(form("   ", "41595")).host).toBeDefined();
  });

  it("rejects hosts with spaces or slashes", () => {
    expect(validateConnectForm(form("my pc", "41595")).host).toBeDefined();
    expect(validateConnectForm(form("http://pc", "41595")).host).toBeDefined();
  });

  it("accepts ports from 1 to 65535 only", () => {
    expect(validateConnectForm(form("pc", "1")).port).toBeUndefined();
    expect(validateConnectForm(form("pc", "65535")).port).toBeUndefined();
    for (const port of ["", "0", "65536", "-1", "41595a", "4.5"]) {
      expect(validateConnectForm(form("pc", port)).port, port).toBeDefined();
    }
  });

  it("does not require a token", () => {
    expect(validateConnectForm({ host: "pc", port: "41595", token: "" })).toEqual({});
  });
});
