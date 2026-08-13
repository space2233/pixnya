import type { BookmarkContentKind, BookmarkDetail, BookmarkUpdate } from "$lib/types";

export type BookmarkBatchAction = "public" | "private" | "add_tag" | "remove_tag" | "remove";

export function buildBookmarkBatchUpdate(
  kind: BookmarkContentKind,
  resourceId: string,
  detail: BookmarkDetail,
  action: BookmarkBatchAction,
  rawTag = "",
): BookmarkUpdate {
  const tag = rawTag.trim();
  if ((action === "add_tag" || action === "remove_tag") && !tag) throw new Error("invalid_input");
  let tags = detail.tags.filter((item) => item.isRegistered).map((item) => item.name);
  const normalizedTag = tag.toLocaleLowerCase();
  if (action === "add_tag" && !tags.some((item) => item.toLocaleLowerCase() === normalizedTag)) {
    tags = [...tags, tag];
  }
  if (action === "remove_tag") {
    tags = tags.filter((item) => item.toLocaleLowerCase() !== normalizedTag);
  }
  return {
    kind,
    resourceId,
    bookmarked: action !== "remove",
    restrict: action === "public" ? "public" : action === "private" ? "private" : detail.restrict,
    tags,
  };
}
