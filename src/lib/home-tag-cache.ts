const PIXIV_CLIENT_HOME_TAGS_V1 = "pixiv-client:home-tags:v1";
const MAX_TAG_COUNT = 12;
const MAX_TAG_LENGTH = 100;

type TagLike = { name: string };
type StoredHomeTags = { tags: string[]; savedAt: number };

function normalizeTags(values: unknown): string[] {
  if (!Array.isArray(values)) return [];
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const value of values) {
    if (typeof value !== "string") continue;
    const tag = value.trim();
    if (!tag || tag.length > MAX_TAG_LENGTH || seen.has(tag)) continue;
    seen.add(tag);
    tags.push(tag);
    if (tags.length === MAX_TAG_COUNT) break;
  }
  return tags;
}

export function loadHomeTagCache(): string[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const stored = JSON.parse(localStorage.getItem(PIXIV_CLIENT_HOME_TAGS_V1) ?? "null") as
      | StoredHomeTags
      | null;
    return normalizeTags(stored?.tags);
  } catch {
    return [];
  }
}

export function saveHomeTagCache(tags: readonly TagLike[]): string[] {
  const normalized = normalizeTags(tags.map((tag) => tag.name));
  if (normalized.length === 0 || typeof localStorage === "undefined") return normalized;
  const stored: StoredHomeTags = { tags: normalized, savedAt: Date.now() };
  try {
    localStorage.setItem(PIXIV_CLIENT_HOME_TAGS_V1, JSON.stringify(stored));
  } catch {
    // A full or unavailable WebView store must not break the home feed.
  }
  return normalized;
}
