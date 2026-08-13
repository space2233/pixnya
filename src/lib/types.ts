export type ConnectionMode = "standard" | "ech" | "compatible";
export type TrafficClass = "oauth" | "api" | "media" | "login_web_view";

export interface PlatformCapabilities {
  rustEch: boolean;
  rustCompatibleDirect: boolean;
  webviewProxy: boolean;
  webviewInsecureBridge: boolean;
}

export interface AppStatus {
  phase: string;
  platform: string;
  architecture: string;
  version: string;
  capabilities: PlatformCapabilities;
}

export type UpdatePhase =
  | "idle"
  | "not_configured"
  | "checking"
  | "up_to_date"
  | "available"
  | "downloading"
  | "ready_to_install"
  | "installing"
  | "awaiting_system_action"
  | "failed";

export type UpdateInstaller = "desktop_tauri" | "android_system";
export type UpdateFailure =
  | "busy"
  | "invalid_source_configuration"
  | "network_or_manifest"
  | "local_state_unavailable"
  | "platform_unavailable"
  | "update_unavailable"
  | "download_verification"
  | "installation_unavailable"
  | "cancelled";

export interface UpdatePreferences {
  autoCheck: boolean;
  autoDownload: boolean;
  unmeteredOnly: boolean;
}

export interface AvailableUpdate {
  version: string;
  notes?: string | null;
  publishedAt?: string | null;
  sizeBytes?: number | null;
}

export interface UpdateSnapshot {
  currentVersion: string;
  source: "github_releases";
  sourceConfigured: boolean;
  installer: UpdateInstaller;
  preferences: UpdatePreferences;
  phase: UpdatePhase;
  readyToInstall: boolean;
  downloadedBytes: number;
  totalBytes?: number | null;
  lastAttemptedAtUnixSeconds?: number | null;
  lastCheckedAtUnixSeconds?: number | null;
  available?: AvailableUpdate | null;
  failure?: UpdateFailure | null;
}

export interface RoutePlan {
  transport:
    | "system"
    | "ech"
    | "compatible_direct"
    | "web_view_system"
    | "web_view_proxy"
    | "web_view_insecure_bridge";
  certificateHost: string;
  echRequirement: "not_applicable" | "accepted" | "platform_managed" | "preflight_only";
  security: "system_tls" | "ech_verified" | "insecure";
  requiresUserAcknowledgement: boolean;
}

export interface ConnectionProbe {
  route: RoutePlan;
  host: string;
  connectedIp?: string;
  candidateAddressCount?: number;
  httpStatus: number;
  latencyMs: number;
  echStatus: "not_applicable" | "accepted";
}

export type DiagnosticTarget = "api" | "media" | "login";
export type DiagnosticStatus = "reachable" | "unreachable" | "platform_route_ready";

export interface DiagnosticCheck {
  target: DiagnosticTarget;
  host: string;
  status: DiagnosticStatus;
  route?: RoutePlan | null;
  connectedIp?: string | null;
  candidateAddressCount?: number | null;
  httpStatus?: number | null;
  latencyMs?: number | null;
  failure?: string | null;
}

export interface ConnectionDiagnosticReport {
  schemaVersion: number;
  applicationVersion: string;
  platform: string;
  architecture: string;
  mode: ConnectionMode;
  capabilities: PlatformCapabilities;
  webviewProxyActive: boolean;
  checks: DiagnosticCheck[];
}

export interface PolicyFailure {
  kind: string;
  host?: string;
}

export interface LoginPreparation {
  route: RoutePlan;
  pkceMethod: "S256";
  callbackTarget: string;
  oauthConfigurationReady: boolean;
  replacedExistingAttempt: boolean;
}

export interface LoginLaunchResult {
  launchId: number;
  mode: ConnectionMode;
  route: RoutePlan;
  target: "desktop_webview_window" | "android_login_activity";
  echPreflight: "not_applicable" | "accepted_by_rust_preflight";
  proxyActive: boolean;
}

export interface SessionUser {
  id: string;
  name: string;
  account: string;
  avatarUrl?: string;
  isPremium: boolean;
}

export interface SessionSnapshot {
  loggedIn: boolean;
  user?: SessionUser | null;
  expiresAtUnixSeconds?: number | null;
  connectionMode?: ConnectionMode | null;
}

export interface LoginCompletionResult {
  status: "pending" | "completed";
  session?: SessionSnapshot | null;
}

export interface LoginCompletionProgress {
  launchId: number;
  stage: "callback_verified" | "transport_ready" | "token_received" | "session_saved";
  elapsedMs: number;
}

export interface IllustrationAuthor {
  id: string;
  name: string;
  account: string;
  avatarUrl?: string | null;
  isFollowed: boolean;
}

export interface IllustrationSummary {
  id: string;
  title: string;
  kind: string;
  thumbnailUrl?: string | null;
  author: IllustrationAuthor;
  pageCount: number;
  width: number;
  height: number;
  isBookmarked: boolean;
  xRestrict: number;
  sanityLevel: number;
  aiType: number;
  tags: string[];
}

export interface IllustrationPage {
  illustrations: IllustrationSummary[];
  nextCursor?: string | null;
}

export interface NovelSummary {
  id: string;
  title: string;
  caption: string;
  coverUrl?: string | null;
  author: IllustrationAuthor;
  createDate: string;
  pageCount: number;
  textLength: number;
  isBookmarked: boolean;
  xRestrict: number;
  aiType: number;
  tags: string[];
  series?: IllustrationSeries | null;
  totalViews: number;
  totalBookmarks: number;
  totalComments: number;
}

export interface NovelPage {
  novels: NovelSummary[];
  nextCursor?: string | null;
}

export interface NovelSeriesDetail {
  id: string;
  title: string;
  caption: string;
  isOriginal: boolean;
  isConcluded: boolean;
  contentCount: number;
  totalCharacterCount: number;
  author: IllustrationAuthor;
  displayText: string;
  aiType: number;
  watchlistAdded: boolean;
}

export interface NovelSeriesPage {
  series: NovelSeriesDetail;
  firstNovel: NovelSummary;
  latestNovel: NovelSummary;
  novels: NovelSummary[];
  nextCursor?: string | null;
}

export interface NovelDetail {
  novel: NovelSummary;
  visible: boolean;
  isMuted: boolean;
  isOriginal: boolean;
  isMypixivOnly: boolean;
}

export interface NovelContent {
  novelId: string;
  title: string;
  text: string;
  coverUrl?: string | null;
  seriesId?: string | null;
  seriesTitle?: string | null;
  seriesNavigation: NovelSeriesNavigation;
  illustrationIds: string[];
  imageIds: string[];
}

export interface NovelSeriesNavigation {
  previous?: NovelSeriesNavigationItem | null;
  next?: NovelSeriesNavigationItem | null;
}

export interface NovelSeriesNavigationItem {
  id: string;
  title: string;
  coverUrl?: string | null;
  contentOrder: string;
  viewable: boolean;
  viewableMessage?: string | null;
}

export interface UgoiraFrame {
  fileName: string;
  delayMs: number;
}

export interface UgoiraMetadata {
  zipUrl: string;
  frames: UgoiraFrame[];
}

export type OfflineKind = "artwork" | "novel" | "ugoira";

export interface OfflineEntry {
  key: string;
  kind: OfflineKind;
  resourceId: string;
  title: string;
  author: string;
  coverUrl?: string | null;
  storedAtUnixSeconds: number;
  assetCount: number;
  sizeBytes: number;
}

export interface OfflineStats {
  entryCount: number;
  sizeBytes: number;
}

export interface LocalCollection {
  id: number;
  name: string;
  entryCount: number;
}

export interface EntryOrganization {
  entryKey: string;
  collectionId?: number | null;
  tags: string[];
}

export interface LocalCatalogSnapshot {
  collections: LocalCollection[];
  entries: EntryOrganization[];
  savedFilters: SavedCatalogFilter[];
}

export interface CatalogFilterDraft {
  name: string;
  query: string;
  kind?: OfflineKind | null;
  collectionId?: number | null;
  tag?: string | null;
  sortOrder: "newest" | "oldest" | "title" | "size";
  storedAfter?: number | null;
  storedBefore?: number | null;
  minSizeBytes?: number | null;
  maxSizeBytes?: number | null;
}

export interface SavedCatalogFilter extends CatalogFilterDraft {
  id: number;
}

export interface BatchOrganizationChange {
  entryKeys: string[];
  updateCollection: boolean;
  collectionId?: number | null;
  addTags: string[];
  removeTags: string[];
}

export interface DuplicateGroup {
  reason: "resource_id" | "file_hash";
  signature: string;
  entryKeys: string[];
}

export type HistoryKind = "artwork" | "novel" | "user";

export interface HistoryRecord {
  kind: HistoryKind;
  resourceId: string;
  title: string;
  subtitle: string;
  thumbnailUrl?: string | null;
}

export interface HistoryEntry extends HistoryRecord {
  viewedAtUnixSeconds: number;
}

export interface HistorySnapshot {
  enabled: boolean;
  limit: number;
  entries: HistoryEntry[];
}

export interface HistoryClearStats {
  entriesRemoved: number;
}

export type DownloadKind = OfflineKind;
export type DownloadState = "queued" | "running" | "paused" | "failed" | "completed";
export type DownloadFailure =
  | "authentication"
  | "network"
  | "invalid_response"
  | "storage"
  | "interrupted";

export interface DownloadTask {
  id: number;
  kind: DownloadKind;
  resourceId: string;
  title?: string | null;
  author?: string | null;
  state: DownloadState;
  completedItems: number;
  totalItems: number;
  downloadedBytes: number;
  attemptCount: number;
  failure?: DownloadFailure | null;
  createdAtUnixSeconds: number;
  updatedAtUnixSeconds: number;
}

export interface DownloadQueueStats {
  taskCount: number;
  activeCount: number;
  failedCount: number;
  completedCount: number;
}

export type MediaCacheKind = "thumbnail" | "preview" | "original";

export interface MediaCacheStats {
  entryCount: number;
  sizeBytes: number;
  verifiedBytes: number;
  insecureBytes: number;
  thumbnailBytes: number;
  previewBytes: number;
  originalBytes: number;
  maxBytes: number | null;
}

export type StorageHealth = "healthy" | "low" | "critical";

export interface StorageStatus {
  health: StorageHealth;
  dataTotalBytes: number;
  dataAvailableBytes: number;
  cacheTotalBytes: number;
  cacheAvailableBytes: number;
  writableDownloadBytes: number;
  offlineBytes: number;
  cacheBytes: number;
  cacheLimitBytes: number | null;
  cacheRemainingQuotaBytes: number | null;
  reserveBytes: number;
  warningBytes: number;
}

export type ExportDestinationKind = "desktop_directory" | "android_document_tree";

export interface ExportDestinationStatus {
  configured: boolean;
  kind?: ExportDestinationKind | null;
  label?: string | null;
  accessible: boolean;
  autoExport: boolean;
}

export interface ExportDestinationSelection {
  cancelled: boolean;
  status: ExportDestinationStatus;
}

export interface OfflineExportResult {
  key: string;
  destination: string;
  directoryName: string;
  fileCount: number;
  sizeBytes: number;
}

export type LocalDataClearFailure =
  | "secure_storage"
  | "session"
  | "login_state"
  | "transport_state"
  | "offline_library"
  | "media_cache"
  | "login_web_view"
  | "diagnostic_log"
  | "download_queue"
  | "storage_settings"
  | "export_settings"
  | "update_settings"
  | "local_catalog"
  | "browsing_history";

export interface LocalDataClearReport {
  complete: boolean;
  credentialsCleared: boolean;
  sessionCleared: boolean;
  transportStateCleared: boolean;
  offlineEntriesRemoved: number;
  offlineBytesRemoved: number;
  cacheEntriesRemoved: number;
  cacheBytesRemoved: number;
  loginWebviewDataCleared: boolean;
  diagnosticLogEntriesRemoved: number;
  downloadTasksRemoved: number;
  storageSettingsReset: boolean;
  exportSettingsReset: boolean;
  updateSettingsReset: boolean;
  localCollectionsRemoved: number;
  localOrganizedEntriesRemoved: number;
  localTagsRemoved: number;
  browsingHistoryEntriesRemoved: number;
  failedSteps: LocalDataClearFailure[];
}

export interface DiagnosticLogSummary {
  entryCount: number;
  retainedBytes: number;
  maxBytes: number;
  retentionDays: number;
  oldestTimestampUnixSeconds?: number | null;
  newestTimestampUnixSeconds?: number | null;
}

export interface DiagnosticLogExportResult {
  destination: string;
  entryCount: number;
}

export interface PreparedUgoiraFrame {
  assetName: string;
  delayMs: number;
}

export interface PreparedUgoira {
  entry: OfflineEntry;
  frames: PreparedUgoiraFrame[];
}

export type UgoiraExportFormat = "gif" | "apng" | "webm";
export type UgoiraExportPhase =
  | "queued"
  | "preparing"
  | "encoding"
  | "exporting"
  | "completed"
  | "failed"
  | "cancelled";

export interface UgoiraExportTask {
  id: string;
  illustrationId: string;
  format: UgoiraExportFormat;
  phase: UgoiraExportPhase;
  completedUnits: number;
  totalUnits: number;
  destination?: string | null;
  failure?: string | null;
}

export interface IllustrationImage {
  pageIndex: number;
  displayUrl?: string | null;
  originalUrl?: string | null;
}

export interface IllustrationSeries {
  id: string;
  title: string;
}

export interface IllustrationSeriesDetail {
  id: string;
  title: string;
  caption: string;
  coverUrl?: string | null;
  workCount: number;
  createDate: string;
  width: number;
  height: number;
  author: IllustrationAuthor;
  watchlistAdded: boolean;
}

export interface IllustrationSeriesPage {
  series: IllustrationSeriesDetail;
  firstIllustration: IllustrationSummary;
  illustrations: IllustrationSummary[];
  nextCursor?: string | null;
}

export interface IllustrationTag {
  name: string;
  translatedName?: string | null;
}

export interface IllustrationDetail {
  illustration: IllustrationSummary;
  caption: string;
  createDate: string;
  pages: IllustrationImage[];
  totalViews: number;
  totalBookmarks: number;
  totalComments: number;
  tools: string[];
  visible: boolean;
  isMuted: boolean;
  series?: IllustrationSeries | null;
  tags: IllustrationTag[];
}

export interface UserProfile {
  webpage?: string | null;
  gender: string;
  birth: string;
  region: string;
  job: string;
  totalFollowUsers: number;
  totalMypixivUsers: number;
  totalIllustrations: number;
  totalManga: number;
  totalNovels: number;
  totalIllustrationBookmarks: number;
  backgroundImageUrl?: string | null;
  twitterAccount: string;
  twitterUrl?: string | null;
  pawooUrl?: string | null;
  isPremium: boolean;
}

export interface UserDetail {
  user: IllustrationAuthor;
  comment: string;
  profile: UserProfile;
}

export type UserWorkKind = "illust" | "manga";

export type RankingMode = "day" | "week" | "month";
export type SearchTarget =
  | "partial_match_for_tags"
  | "exact_match_for_tags"
  | "title_and_caption";
export type BookmarkRestrict = "public" | "private";
export type BookmarkContentKind = "illustration" | "novel";

export interface BookmarkTagStatus {
  name: string;
  isRegistered: boolean;
}

export interface BookmarkDetail {
  restrict: BookmarkRestrict;
  tags: BookmarkTagStatus[];
}

export interface BookmarkTag {
  name: string;
  count: number;
}

export interface BookmarkTagPage {
  tags: BookmarkTag[];
  nextCursor?: string | null;
}

export interface BookmarkUpdate {
  kind: BookmarkContentKind;
  resourceId: string;
  bookmarked: boolean;
  restrict: BookmarkRestrict;
  tags: string[];
}

export type BookmarkUpdateFailure =
  | "authentication_required"
  | "invalid_input"
  | "request_failed"
  | "rejected"
  | "invalid_response";

export interface BookmarkUpdateResult {
  kind: BookmarkContentKind;
  resourceId: string;
  succeeded: boolean;
  failure?: BookmarkUpdateFailure | null;
}

export interface TrendingTag {
  name: string;
  translatedName?: string | null;
  illustration: IllustrationSummary;
}

export interface UserPreview {
  user: IllustrationAuthor;
  illustrations: IllustrationSummary[];
  isMuted: boolean;
}

export interface UserPreviewPage {
  users: UserPreview[];
  nextCursor?: string | null;
}

export interface PixivCommentParent {
  id: string;
  text: string;
  userName: string;
}

export interface PixivComment {
  id: string;
  text: string;
  date: string;
  user?: IllustrationAuthor | null;
  hasReplies: boolean;
  parent?: PixivCommentParent | null;
  stamp?: PixivCommentStamp | null;
}

export interface PixivCommentStamp {
  id: string;
  url: string;
}

export interface CommentPage {
  comments: PixivComment[];
  nextCursor?: string | null;
  totalComments: number;
  commentAccessControl: number;
}

export interface NotificationContent {
  text: string;
  leftIcon?: string | null;
  leftImage?: string | null;
  rightIcon?: string | null;
  rightImage?: string | null;
}

export interface NotificationViewMore {
  title: string;
  unreadExists: boolean;
}

export interface PixivNotification {
  id: string;
  typeId: number;
  isRead: boolean;
  createdDatetime: string;
  targetUrl?: string | null;
  content: NotificationContent;
  viewMore?: NotificationViewMore | null;
}

export interface NotificationPage {
  notifications: PixivNotification[];
  nextCursor?: string | null;
}

export interface AccessBlockPage {
  users: IllustrationAuthor[];
  nextCursor?: string | null;
}

export interface MutedTag {
  name: string;
  translatedName?: string | null;
  isPremiumSlot: boolean;
}

export interface MutedUser {
  user: IllustrationAuthor;
  isPremiumSlot: boolean;
}

export interface MuteTextLimits {
  withoutPremium: number;
  withPremium: number;
}

export interface MuteSettings {
  tags: MutedTag[];
  users: MutedUser[];
  limitCount: number;
  textLimits: MuteTextLimits;
}

export interface CommentSubmission {
  text: string;
  stampId?: string | null;
}

export interface CommandFailure {
  kind?: string;
  httpStatus?: number;
  availableBytes?: number;
  requiredBytes?: number;
  reserveBytes?: number;
}

export interface BackupSummary {
  formatVersion: number;
  applicationVersion: string;
  componentCount: number;
  offlineFileCount: number;
  offlineIncluded: boolean;
  totalBytes: number;
  containsCredentials: boolean;
}

export interface BackupSelectionResult {
  cancelled: boolean;
  label?: string | null;
  preview?: BackupSummary | null;
}

export interface BackupExportResult {
  destination: string;
  summary: BackupSummary;
}

export interface BackupRestoreStartResult {
  transactionId: number;
  frontend: import("$lib/local-backup").FrontendBackupState;
  summary: BackupSummary;
}

export interface FrontendBackupRecovery {
  transactionId: number;
  frontend: import("$lib/local-backup").FrontendBackupState;
}

export type BackupRestoreStrategy = "merge" | "replace";
