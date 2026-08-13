import type { BookmarkTag, BookmarkTagPage } from "$lib/types";

const MAX_BOOKMARK_TAG_PAGES = 100;

export async function loadAllBookmarkTags(
  fetchPage: (cursor?: string) => Promise<BookmarkTagPage>,
): Promise<BookmarkTag[]> {
  const tags = new Map<string, BookmarkTag>();
  const seenCursors = new Set<string>();
  let cursor: string | undefined;

  for (let pageNumber = 0; pageNumber < MAX_BOOKMARK_TAG_PAGES; pageNumber += 1) {
    const page = await fetchPage(cursor);
    for (const tag of page.tags) {
      const name = tag.name.trim();
      if (!name) continue;
      const existing = tags.get(name);
      tags.set(name, { name, count: Math.max(existing?.count ?? 0, tag.count) });
    }
    const next = page.nextCursor?.trim();
    if (!next) return [...tags.values()];
    if (seenCursors.has(next)) throw { kind: "invalid_response" };
    seenCursors.add(next);
    cursor = next;
  }
  throw { kind: "invalid_response" };
}
