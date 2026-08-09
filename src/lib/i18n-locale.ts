export type AppLocale = "zh-CN" | "zh-TW" | "en-US";

export function mapSystemLanguages(languages: readonly string[]): AppLocale {
  for (const candidate of languages) {
    const normalized = candidate.trim().replaceAll("_", "-").toLowerCase();
    if (!normalized) continue;

    if (
      normalized === "zh-tw" ||
      normalized === "zh-hk" ||
      normalized === "zh-mo" ||
      normalized.startsWith("zh-hant")
    ) {
      return "zh-TW";
    }

    if (normalized === "zh" || normalized.startsWith("zh-")) {
      return "zh-CN";
    }
  }

  return "en-US";
}
