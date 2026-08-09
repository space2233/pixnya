import { m } from "./paraglide/messages.js";
import {
  defineCustomClientStrategy,
  getLocale,
  type Locale,
} from "./paraglide/runtime.js";
import { mapSystemLanguages } from "./i18n-locale.ts";

export type LanguagePreference = "system" | Locale;

export const LANGUAGE_PREFERENCE_KEY = "pixiv-client.interface-language";

const supportedPreferences = new Set<LanguagePreference>([
  "system",
  "zh-CN",
  "zh-TW",
  "en-US",
]);

export function readLanguagePreference(): LanguagePreference {
  if (typeof localStorage === "undefined") return "system";
  const stored = localStorage.getItem(LANGUAGE_PREFERENCE_KEY);
  return stored && supportedPreferences.has(stored as LanguagePreference)
    ? (stored as LanguagePreference)
    : "system";
}

export function resolveLanguagePreference(
  preference: LanguagePreference,
  systemLanguages: readonly string[] = browserLanguages(),
): Locale {
  return preference === "system" ? mapSystemLanguages(systemLanguages) : preference;
}

export function currentAppLocale(): Locale {
  return resolveLanguagePreference(readLanguagePreference());
}

export function initializeI18n(): Locale {
  const locale = getLocale();
  if (typeof document !== "undefined") {
    document.documentElement.lang = locale;
    document.documentElement.dir = "ltr";
  }
  return locale;
}

export function setLanguagePreference(preference: LanguagePreference): void {
  if (!supportedPreferences.has(preference)) return;
  localStorage.setItem(LANGUAGE_PREFERENCE_KEY, preference);
  window.location.reload();
}

function browserLanguages(): readonly string[] {
  if (typeof navigator === "undefined") return [];
  if (navigator.languages?.length) return navigator.languages;
  return navigator.language ? [navigator.language] : [];
}

if (typeof window !== "undefined") {
  defineCustomClientStrategy("custom-pixnya", {
    getLocale: () => currentAppLocale(),
    // Paraglide invokes this during initialization. PixNya persists language
    // choices explicitly so this callback must not overwrite "system".
    setLocale: () => undefined,
  });
}

export { m };
export { mapSystemLanguages } from "./i18n-locale.ts";
