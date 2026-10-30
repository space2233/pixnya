import type { NovelContent, NovelDetail } from "./types";
import { validNovelId } from "./reading-shelf.ts";

type ReadingSource = { detail: NovelDetail; content: NovelContent; offline: boolean };
type ReadingDependencies = {
  readOfflineText: (key: string, name: string) => Promise<string>;
  getNovelDetail: (id: string) => Promise<NovelDetail>;
  getNovelContent: (id: string) => Promise<NovelContent>;
};

export async function loadNovelForReading(
  id: string, offlineOnly: boolean, loggedIn: boolean, api: ReadingDependencies,
): Promise<ReadingSource> {
  if (!validNovelId(id)) throw new Error("invalid_novel_id");
  try {
    const [detailText, contentText] = await Promise.all([
      api.readOfflineText(`novel-${id}`, "detail.json"),
      api.readOfflineText(`novel-${id}`, "content.json"),
    ]);
    const detail = JSON.parse(detailText) as NovelDetail;
    const content = JSON.parse(contentText) as NovelContent;
    if (detail?.novel?.id !== id || content?.novelId !== id || typeof content.text !== "string"
      || !detail.novel.author || !content.seriesNavigation) throw new Error("invalid_offline_novel");
    return { detail, content, offline: true };
  } catch (error) {
    if (offlineOnly) throw error;
  }
  if (!loggedIn) throw new Error("reading_login_required");
  const [detail, content] = await Promise.all([api.getNovelDetail(id), api.getNovelContent(id)]);
  if (detail.novel.id !== id || content.novelId !== id) throw new Error("invalid_novel_id");
  return { detail, content, offline: false };
}

export function readingChapterHref(id: string | undefined, offline: boolean, downloadedIds: ReadonlySet<string>): string | null {
  if (!validNovelId(id)) return null;
  if (!offline) return `/novels/${id}/read`;
  return downloadedIds.has(id) ? `/offline/novels/${id}` : null;
}
