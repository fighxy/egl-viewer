import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import type { Locale } from "@/ipc";
import { en, type MessageCatalog } from "./en";
import { isAppLocale } from "./locales";
import { ru } from "./ru";
import { zhCN } from "./zh-CN";

export { APP_LOCALES, isAppLocale } from "./locales";
export type { MessageCatalog };

const resources = {
  en: { translation: en },
  ru: { translation: ru },
  "zh-CN": { translation: zhCN },
} as const;

void i18n.use(initReactI18next).init({
  resources,
  lng: "en",
  fallbackLng: "en",
  defaultNS: "translation",
  interpolation: { escapeValue: false },
  // Nested catalogs use `.`; command ids contain dots, so they live under `commands.*`.
  keySeparator: ".",
  returnNull: false,
});

/** Apply a persisted or chosen UI language (D-069). */
export function setAppLocale(locale: Locale | string): void {
  const next = isAppLocale(locale) ? locale : "en";
  if (i18n.language !== next) {
    void i18n.changeLanguage(next);
  }
}

/** Flat key paths of a catalog, for coverage tests. */
export function catalogKeys(catalog: object, prefix = ""): string[] {
  const keys: string[] = [];
  for (const [key, value] of Object.entries(catalog)) {
    const path = prefix ? `${prefix}.${key}` : key;
    if (value !== null && typeof value === "object" && !Array.isArray(value)) {
      keys.push(...catalogKeys(value, path));
    } else {
      keys.push(path);
    }
  }
  return keys;
}

export { i18n };
export default i18n;
