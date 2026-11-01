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

  it("translates sort field and order labels", async () => {
    expect(i18n.t("sort.added")).toBe("Date Added");
    expect(i18n.t("sort.ascending")).toBe("Ascending");
    expect(i18n.t("sort.byAria", { label: i18n.t("sort.added") })).toBe("Sort by Date Added");

    await i18n.changeLanguage("ru");
    expect(i18n.t("sort.added")).toBe("Дата добавления");
    expect(i18n.t("sort.btime")).toBe("Дата создания");
    expect(i18n.t("sort.mtime")).toBe("Дата изменения");
    expect(i18n.t("sort.name")).toBe("Название");
    expect(i18n.t("sort.ext")).toBe("Расширение");
    expect(i18n.t("sort.size")).toBe("Размер файла");
    expect(i18n.t("sort.resolution")).toBe("Размеры");
    expect(i18n.t("sort.star")).toBe("Оценка");
    expect(i18n.t("sort.tag")).toBe("Теги");
    expect(i18n.t("sort.random")).toBe("Случайно");
    expect(i18n.t("sort.manual")).toBe("Вручную");
    expect(i18n.t("sort.ascending")).toBe("По возрастанию");
    expect(i18n.t("sort.descending")).toBe("По убыванию");
    expect(i18n.t("sort.title")).toBe("Сортировка");
    expect(i18n.t("sort.byAria", { label: i18n.t("sort.added") })).toBe(
      "Сортировка: Дата добавления",
    );

    await i18n.changeLanguage("zh-CN");
    expect(i18n.t("sort.added")).toBe("添加日期");
    expect(i18n.t("sort.ascending")).toBe("升序");
    expect(i18n.t("sort.descending")).toBe("降序");
    expect(i18n.t("sort.byAria", { label: i18n.t("sort.name") })).toBe("按标题排序");

    setAppLocale("en");
  });
});
