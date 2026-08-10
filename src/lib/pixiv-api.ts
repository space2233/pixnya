import { invoke } from "@tauri-apps/api/core";
import { m } from "$lib/i18n";
import type {
  CommandFailure,
  IllustrationDetail,
  IllustrationPage,
  IllustrationSeriesPage,
  BookmarkRestrict,
  CommentPage,
  RankingMode,
  SearchTarget,
  TrendingTag,
  UserDetail,
  UserPreviewPage,
  UserWorkKind,
  PixivComment,
  NovelContent,
  NovelDetail,
  NovelPage,
  NovelSeriesPage,
  UgoiraMetadata,
  OfflineEntry,
  OfflineStats,
  PreparedUgoira,
  MediaCacheStats,
  LocalDataClearReport,
  DiagnosticLogSummary,
  DiagnosticLogExportResult,
  DownloadKind,
  DownloadTask,
  DownloadQueueStats,
  StorageStatus,
  ExportDestinationStatus,
  ExportDestinationSelection,
  OfflineExportResult,
  LocalCatalogSnapshot,
  LocalCollection,
  EntryOrganization,
  HistoryClearStats,
  HistoryKind,
  HistoryRecord,
  HistorySnapshot,
} from "$lib/types";

export function getRecommendedIllustrations(cursor?: string): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_recommended_illustrations", {
    cursor: cursor ?? null,
  });
}

export function getRecommendedManga(cursor?: string): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_recommended_manga", { cursor: cursor ?? null });
}

export function getRecommendedNovels(cursor?: string): Promise<NovelPage> {
  return invoke<NovelPage>("get_recommended_novels", { cursor: cursor ?? null });
}

export function getNovelDetail(novelId: string): Promise<NovelDetail> {
  return invoke<NovelDetail>("get_novel_detail", { novelId });
}

export function getNovelContent(novelId: string): Promise<NovelContent> {
  return invoke<NovelContent>("get_novel_content", { novelId });
}

export function getNovelSeries(seriesId: string, cursor?: string): Promise<NovelSeriesPage> {
  return invoke<NovelSeriesPage>("get_novel_series", {
    seriesId,
    cursor: cursor ?? null,
  });
}

export function searchNovels(
  word: string,
  searchTarget: SearchTarget,
  cursor?: string,
): Promise<NovelPage> {
  return invoke<NovelPage>("search_novels", {
    word,
    searchTarget,
    cursor: cursor ?? null,
  });
}

export function getUserNovels(userId: string, cursor?: string): Promise<NovelPage> {
  return invoke<NovelPage>("get_user_novels", { userId, cursor: cursor ?? null });
}

export function getFollowedNovels(cursor?: string): Promise<NovelPage> {
  return invoke<NovelPage>("get_followed_novels", { cursor: cursor ?? null });
}

export function getBookmarkedNovels(
  restrict: BookmarkRestrict,
  cursor?: string,
): Promise<NovelPage> {
  return invoke<NovelPage>("get_bookmarked_novels", { restrict, cursor: cursor ?? null });
}

export function getRankingNovels(
  rankingMode: RankingMode,
  cursor?: string,
): Promise<NovelPage> {
  return invoke<NovelPage>("get_ranking_novels", { rankingMode, cursor: cursor ?? null });
}

export function setNovelBookmark(
  novelId: string,
  bookmarked: boolean,
  restrict: BookmarkRestrict = "public",
): Promise<void> {
  return invoke<void>("set_novel_bookmark", { novelId, bookmarked, restrict });
}

export function getNovelComments(novelId: string, cursor?: string): Promise<CommentPage> {
  return invoke<CommentPage>("get_novel_comments", { novelId, cursor: cursor ?? null });
}

export function getNovelCommentReplies(commentId: string, cursor?: string): Promise<CommentPage> {
  return invoke<CommentPage>("get_novel_comment_replies", {
    commentId,
    cursor: cursor ?? null,
  });
}

export function addNovelComment(
  novelId: string,
  comment: string,
  parentCommentId?: string,
): Promise<PixivComment> {
  return invoke<PixivComment>("add_novel_comment", {
    novelId,
    comment,
    parentCommentId: parentCommentId ?? null,
  });
}

export function getUgoiraMetadata(illustrationId: string): Promise<UgoiraMetadata> {
  return invoke<UgoiraMetadata>("get_ugoira_metadata", { illustrationId });
}

export function downloadArtwork(illustrationId: string): Promise<OfflineEntry> {
  return invoke<OfflineEntry>("download_artwork", { illustrationId });
}

export function downloadNovel(novelId: string): Promise<OfflineEntry> {
  return invoke<OfflineEntry>("download_novel", { novelId });
}

export function enqueueDownload(
  kind: DownloadKind,
  resourceId: string,
  title?: string | null,
  author?: string | null,
): Promise<DownloadTask> {
  return invoke<DownloadTask>("enqueue_download", {
    kind,
    resourceId,
    title: title ?? null,
    author: author ?? null,
  });
}

export function listDownloadTasks(): Promise<DownloadTask[]> {
  return invoke<DownloadTask[]>("list_download_tasks");
}

export function getDownloadQueueStats(): Promise<DownloadQueueStats> {
  return invoke<DownloadQueueStats>("get_download_queue_stats");
}

export function pauseDownloadTask(taskId: number): Promise<DownloadTask> {
  return invoke<DownloadTask>("pause_download_task", { taskId });
}

export function resumeDownloadTask(taskId: number): Promise<DownloadTask> {
  return invoke<DownloadTask>("resume_download_task", { taskId });
}

export function removeDownloadTask(taskId: number): Promise<boolean> {
  return invoke<boolean>("remove_download_task", { taskId });
}

export function prepareUgoira(illustrationId: string): Promise<PreparedUgoira> {
  return invoke<PreparedUgoira>("prepare_ugoira", { illustrationId });
}

export function listOfflineEntries(): Promise<OfflineEntry[]> {
  return invoke<OfflineEntry[]>("list_offline_entries");
}

export function getLocalCatalogSnapshot(): Promise<LocalCatalogSnapshot> {
  return invoke<LocalCatalogSnapshot>("get_local_catalog_snapshot");
}

export function createLocalCollection(name: string): Promise<LocalCollection> {
  return invoke<LocalCollection>("create_local_collection", { name });
}

export function renameLocalCollection(
  collectionId: number,
  name: string,
): Promise<LocalCollection> {
  return invoke<LocalCollection>("rename_local_collection", { collectionId, name });
}

export function deleteLocalCollection(collectionId: number): Promise<void> {
  return invoke<void>("delete_local_collection", { collectionId });
}

export function organizeOfflineEntry(
  entryKey: string,
  collectionId: number | null,
  tags: string[],
): Promise<EntryOrganization> {
  return invoke<EntryOrganization>("organize_offline_entry", {
    entryKey,
    collectionId,
    tags,
  });
}

export function getBrowsingHistory(): Promise<HistorySnapshot> {
  return invoke<HistorySnapshot>("get_browsing_history");
}

export function setBrowsingHistoryEnabled(enabled: boolean): Promise<HistorySnapshot> {
  return invoke<HistorySnapshot>("set_browsing_history_enabled", { enabled });
}

export function recordBrowsingHistory(record: HistoryRecord): Promise<boolean> {
  return invoke<boolean>("record_browsing_history", { record });
}

export function removeBrowsingHistoryEntry(kind: HistoryKind, resourceId: string): Promise<boolean> {
  return invoke<boolean>("remove_browsing_history_entry", { kind, resourceId });
}

export function clearBrowsingHistory(): Promise<HistoryClearStats> {
  return invoke<HistoryClearStats>("clear_browsing_history");
}

export function getOfflineStats(): Promise<OfflineStats> {
  return invoke<OfflineStats>("get_offline_stats");
}

export function getMediaCacheStats(): Promise<MediaCacheStats> {
  return invoke<MediaCacheStats>("get_media_cache_stats");
}

export function clearMediaCache(): Promise<MediaCacheStats> {
  return invoke<MediaCacheStats>("clear_media_cache", { confirmed: true });
}

export function getStorageStatus(): Promise<StorageStatus> {
  return invoke<StorageStatus>("get_storage_status");
}

export function setMediaCacheLimit(cacheLimitBytes: number): Promise<StorageStatus> {
  return invoke<StorageStatus>("set_media_cache_limit", { cacheLimitBytes });
}

export function getExportDestinationStatus(): Promise<ExportDestinationStatus> {
  return invoke<ExportDestinationStatus>("get_export_destination_status");
}

export function selectExportDestination(title: string): Promise<ExportDestinationSelection> {
  return invoke<ExportDestinationSelection>("select_export_destination", { title });
}

export function clearExportDestination(): Promise<ExportDestinationStatus> {
  return invoke<ExportDestinationStatus>("clear_export_destination");
}

export function setAutoExportDownloads(autoExport: boolean): Promise<ExportDestinationStatus> {
  return invoke<ExportDestinationStatus>("set_auto_export_downloads", { autoExport });
}

export function exportOfflineEntry(entryKey: string): Promise<OfflineExportResult> {
  return invoke<OfflineExportResult>("export_offline_entry", { entryKey });
}

export function clearLocalData(confirmation: string): Promise<LocalDataClearReport> {
  return invoke<LocalDataClearReport>("clear_local_data", {
    request: { confirmation },
  });
}

export function getDiagnosticLogSummary(): Promise<DiagnosticLogSummary> {
  return invoke<DiagnosticLogSummary>("get_diagnostic_log_summary");
}

export function exportDiagnosticLogs(): Promise<DiagnosticLogExportResult> {
  return invoke<DiagnosticLogExportResult>("export_diagnostic_logs");
}

export function clearDiagnosticLogs(): Promise<DiagnosticLogSummary> {
  return invoke<DiagnosticLogSummary>("clear_diagnostic_logs", { confirmed: true });
}

export function readOfflineAsset(key: string, assetName: string): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_offline_asset", { key, assetName });
}

export function readOfflineText(key: string, assetName: string): Promise<string> {
  return invoke<string>("read_offline_text", { key, assetName });
}

export function removeOfflineEntry(key: string): Promise<boolean> {
  return invoke<boolean>("remove_offline_entry", { key });
}

export function getIllustrationDetail(illustrationId: string): Promise<IllustrationDetail> {
  return invoke<IllustrationDetail>("get_illustration_detail", { illustrationId });
}

export function getIllustrationSeries(
  seriesId: string,
  cursor?: string,
): Promise<IllustrationSeriesPage> {
  return invoke<IllustrationSeriesPage>("get_illustration_series", {
    seriesId,
    cursor: cursor ?? null,
  });
}

export function getRelatedIllustrations(
  illustrationId: string,
  cursor?: string,
): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_related_illustrations", {
    illustrationId,
    cursor: cursor ?? null,
  });
}

export function getUserDetail(userId: string): Promise<UserDetail> {
  return invoke<UserDetail>("get_user_detail", { userId });
}

export function getUserIllustrations(
  userId: string,
  workKind: UserWorkKind,
  cursor?: string,
): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_user_illustrations", {
    userId,
    workKind,
    cursor: cursor ?? null,
  });
}

export function getRankingIllustrations(
  rankingMode: RankingMode,
  cursor?: string,
): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_ranking_illustrations", {
    rankingMode,
    cursor: cursor ?? null,
  });
}

export function getTrendingTags(): Promise<TrendingTag[]> {
  return invoke<TrendingTag[]>("get_trending_tags");
}

export function searchIllustrations(
  word: string,
  searchTarget: SearchTarget,
  cursor?: string,
): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("search_illustrations", {
    word,
    searchTarget,
    cursor: cursor ?? null,
  });
}

export function searchUsers(word: string, cursor?: string): Promise<UserPreviewPage> {
  return invoke<UserPreviewPage>("search_users", { word, cursor: cursor ?? null });
}

export function getFollowedUsers(
  restrict: BookmarkRestrict,
  cursor?: string,
): Promise<UserPreviewPage> {
  return invoke<UserPreviewPage>("get_followed_users", {
    restrict,
    cursor: cursor ?? null,
  });
}

export function getFollowedIllustrations(cursor?: string): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_followed_illustrations", {
    cursor: cursor ?? null,
  });
}

export function getBookmarkedIllustrations(
  restrict: BookmarkRestrict,
  cursor?: string,
): Promise<IllustrationPage> {
  return invoke<IllustrationPage>("get_bookmarked_illustrations", {
    restrict,
    cursor: cursor ?? null,
  });
}

export function setIllustrationBookmark(
  illustrationId: string,
  bookmarked: boolean,
  restrict: BookmarkRestrict = "public",
): Promise<void> {
  return invoke<void>("set_illustration_bookmark", { illustrationId, bookmarked, restrict });
}

export function setUserFollow(
  userId: string,
  followed: boolean,
  restrict: BookmarkRestrict = "public",
): Promise<void> {
  return invoke<void>("set_user_follow", { userId, followed, restrict });
}

export function getIllustrationComments(
  illustrationId: string,
  cursor?: string,
): Promise<CommentPage> {
  return invoke<CommentPage>("get_illustration_comments", {
    illustrationId,
    cursor: cursor ?? null,
  });
}

export function getCommentReplies(commentId: string, cursor?: string): Promise<CommentPage> {
  return invoke<CommentPage>("get_comment_replies", { commentId, cursor: cursor ?? null });
}

export function addIllustrationComment(
  illustrationId: string,
  comment: string,
  parentCommentId?: string,
): Promise<PixivComment> {
  return invoke<PixivComment>("add_illustration_comment", {
    illustrationId,
    comment,
    parentCommentId: parentCommentId ?? null,
  });
}

export function describeDataFailure(error: unknown): string {
  const failure = asFailure(error);
  switch (failure.kind) {
    case "authentication_required":
      return m.data_error_authentication();
    case "invalid_identifier":
      return m.data_error_identifier();
    case "invalid_input":
      return m.data_error_input();
    case "invalid_cursor":
      return m.data_error_cursor();
    case "transport_unavailable":
    case "request_failed":
      return m.data_error_transport();
    case "upstream_rejected":
      return failure.httpStatus === 429
        ? m.data_error_rate_limit()
        : m.data_error_upstream({ status: failure.httpStatus ?? m.data_error_unknown_status() });
    case "invalid_response":
      return m.data_error_response();
    case "token_refresh_failed":
      return m.data_error_token_refresh();
    case "secure_storage_unavailable":
      return m.data_error_secure_storage();
    case "offline_unavailable":
      return m.data_error_offline();
    case "offline_not_found":
      return m.data_error_offline_not_found();
    case "local_catalog_unavailable":
      return m.data_error_catalog();
    case "local_collection_not_found":
      return m.data_error_collection_not_found();
    case "local_collection_conflict":
      return m.data_error_collection_conflict();
    case "browsing_history_unavailable":
      return m.data_error_history();
    case "diagnostic_log_unavailable":
      return m.data_error_diagnostics();
    case "export_unavailable":
      return m.data_error_export();
    case "download_queue_unavailable":
      return m.data_error_queue();
    case "download_task_not_found":
      return m.data_error_task_not_found();
    case "download_transition_invalid":
      return m.data_error_transition();
    case "storage_capacity_exceeded":
      return m.data_error_capacity();
    case "storage_unavailable":
      return m.data_error_storage();
    case "export_destination_unavailable":
      return m.data_error_export_destination();
    default:
      return m.data_error_default();
  }
}

function asFailure(error: unknown): CommandFailure {
  if (typeof error === "object" && error !== null) {
    return error as CommandFailure;
  }
  return {};
}
