import { beforeEach, describe, expect, it } from "vitest";
import { en } from "./en";
import { catalogKeys, i18n, setAppLocale } from "./index";
import { ru } from "./ru";
import { zhCN } from "./zh-CN";

describe("i18n catalogs", () => {
  beforeEach(() => {
    setAppLocale("en");
  });

  it("ru and zh-CN cover every English key", () => {
    const english = catalogKeys(en).sort();
    expect(catalogKeys(ru).sort()).toEqual(english);
    expect(catalogKeys(zhCN).sort()).toEqual(english);
  });

  it("switches language and translates settings title", async () => {
    expect(i18n.t("settings.title")).toBe("Settings");
    await i18n.changeLanguage("ru");
    expect(i18n.t("settings.title")).toBe("Параметры");
    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("settings.title")).toBe("设置");
    setAppLocale("en");
    expect(i18n.t("settings.title")).toBe("Settings");
  });

  it("falls back to English for an unknown locale tag", () => {
    setAppLocale("de");
    expect(i18n.language).toBe("en");
  });
});
