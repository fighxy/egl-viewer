import type { Locale } from "@/ipc";

/** Locales the Settings language control offers (D-069). */
export const APP_LOCALES: readonly {
  value: Locale;
  /** Native endonym shown in the language picker. */
  nativeLabel: string;
}[] = [
  { value: "en", nativeLabel: "English" },
  { value: "ru", nativeLabel: "Русский" },
  { value: "zh-CN", nativeLabel: "简体中文" },
];

export function isAppLocale(value: string): value is Locale {
  return APP_LOCALES.some((locale) => locale.value === value);
}
