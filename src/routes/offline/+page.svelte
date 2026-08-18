<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    describeDataFailure,
    batchOrganizeOfflineEntries,
    batchRemoveOfflineEntries,
    createLocalCollection,
    deleteLocalCollection,
    deleteLocalCatalogFilter,
    exportOfflineEntry,
    getExportDestinationStatus,
    getLocalCatalogSnapshot,
    getOfflineStats,
    findOfflineDuplicates,
    listDownloadTasks,
    listOfflineEntries,
    pauseDownloadTask,
    organizeOfflineEntry,
    removeDownloadTask,
    removeOfflineEntry,
    resumeDownloadTask,
    renameLocalCollection,
    saveLocalCatalogFilter,
  } from "$lib/pixiv-api";
  import type {
    DownloadFailure,
    DownloadKind,
    DownloadState,
    DownloadTask,
    ExportDestinationStatus,
    EntryOrganization,
    LocalCatalogSnapshot,
    OfflineEntry,
    OfflineStats,
    DuplicateGroup,
    SavedCatalogFilter,
  } from "$lib/types";

  let entries = $state<OfflineEntry[]>([]);
  let stats = $state<OfflineStats>({ entryCount: 0, sizeBytes: 0 });
  let tasks = $state<DownloadTask[]>([]);
  let libraryStatus = $state<"loading" | "ready" | "error">("loading");
  let queueStatus = $state<"loading" | "ready" | "error">("loading");
  let libraryError = $state("");
  let queueError = $state("");
  let removing = $state("");
  let confirming = $state("");
  let taskActionId = $state<number | null>(null);
  let confirmingTaskId = $state<number | null>(null);
  let exportDestination = $state<ExportDestinationStatus | null>(null);
  let exporting = $state("");
  let exportNotice = $state("");
  let exportNoticeIsError = $state(false);
  let catalog = $state<LocalCatalogSnapshot>({ collections: [], entries: [], savedFilters: [] });
  let catalogStatus = $state<"loading" | "ready" | "error">("loading");
  let catalogError = $state("");
  let catalogNotice = $state("");
  let catalogNoticeIsError = $state(false);
  let catalogAction = $state("");
  let newCollectionName = $state("");
  let renamingCollectionId = $state<number | null>(null);
  let renameCollectionName = $state("");
  let confirmingCollectionId = $state<number | null>(null);
  let organizingKey = $state("");
  let organizationCollectionId = $state("");
  let organizationTags = $state("");
  let libraryQuery = $state("");
  let kindFilter = $state<"all" | DownloadKind>("all");
  let collectionFilter = $state("all");
  let tagFilter = $state("all");
  let sortOrder = $state<"newest" | "oldest" | "title" | "size">("newest");
  let storedAfter = $state("");
  let storedBefore = $state("");
  let minSizeBytes = $state("");
  let maxSizeBytes = $state("");
  let savedFilterName = $state("");
  let selectedEntryKeys = $state<string[]>([]);
  let batchCollectionId = $state("keep");
  let batchAddTags = $state("");
  let batchRemoveTags = $state("");
  let batchConfirmDelete = $state(false);
  let duplicateGroups = $state<DuplicateGroup[]>([]);
  let duplicateStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let queueRequestId = 0;
  let libraryRequestId = 0;
  let catalogRequestId = 0;
  let restoredFromHistory = false;

  type OfflineLibrarySnapshot = {
    entries: OfflineEntry[];
    stats: OfflineStats;
    tasks: DownloadTask[];
    libraryStatus: "loading" | "ready" | "error";
    queueStatus: "loading" | "ready" | "error";
    libraryError: string;
    queueError: string;
    exportDestination: ExportDestinationStatus | null;
    catalog: LocalCatalogSnapshot;
    catalogStatus: "loading" | "ready" | "error";
    catalogError: string;
    libraryQuery: string;
    kindFilter: "all" | DownloadKind;
    collectionFilter: string;
    tagFilter: string;
    sortOrder: "newest" | "oldest" | "title" | "size";
    storedAfter: string;
    storedBefore: string;
    minSizeBytes: string;
    maxSizeBytes: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<OfflineLibrarySnapshot>({
      entries, stats, tasks, libraryStatus, queueStatus, libraryError, queueError,
      exportDestination, catalog, catalogStatus, catalogError, libraryQuery,
      kindFilter, collectionFilter, tagFilter, sortOrder, storedAfter, storedBefore,
      minSizeBytes, maxSizeBytes,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<OfflineLibrarySnapshot>(key);
      if (!value) return;
      restoredFromHistory = true;
      queueRequestId += 1;
      libraryRequestId += 1;
      catalogRequestId += 1;
      entries = value.entries;
      stats = value.stats;
      tasks = value.tasks;
      libraryStatus = value.libraryStatus;
      queueStatus = value.queueStatus;
      libraryError = value.libraryError;
      queueError = value.queueError;
      exportDestination = value.exportDestination;
      catalog = value.catalog;
      catalogStatus = value.catalogStatus;
      catalogError = value.catalogError;
      libraryQuery = value.libraryQuery;
      kindFilter = value.kindFilter;
      collectionFilter = value.collectionFilter;
      tagFilter = value.tagFilter;
      sortOrder = value.sortOrder;
      storedAfter = value.storedAfter ?? "";
      storedBefore = value.storedBefore ?? "";
      minSizeBytes = value.minSizeBytes ?? "";
      maxSizeBytes = value.maxSizeBytes ?? "";
    },
  };

  const availableTags = $derived.by(() => {
    const tags = new Set<string>();
    for (const organization of catalog.entries) {
      for (const tag of organization.tags) tags.add(tag);
    }
    return [...tags].sort((left, right) => left.localeCompare(right, currentAppLocale()));
  });

  const filteredEntries = $derived.by(() => {
    const query = libraryQuery.trim().toLocaleLowerCase();
    const organizations = new Map(catalog.entries.map((entry) => [entry.entryKey, entry]));
    const collections = new Map(catalog.collections.map((collection) => [collection.id, collection.name]));
    const result = entries.filter((entry) => {
      const organization = organizations.get(entry.key);
      if (kindFilter !== "all" && entry.kind !== kindFilter) return false;
      if (collectionFilter === "unfiled" && organization?.collectionId != null) return false;
      if (collectionFilter !== "all" && collectionFilter !== "unfiled") {
        if (organization?.collectionId !== Number(collectionFilter)) return false;
      }
      if (tagFilter !== "all" && !organization?.tags.includes(tagFilter)) return false;
      const after = dateBoundary(storedAfter, false);
      const before = dateBoundary(storedBefore, true);
      const minimum = byteBoundary(minSizeBytes);
      const maximum = byteBoundary(maxSizeBytes);
      if (after !== null && entry.storedAtUnixSeconds < after) return false;
      if (before !== null && entry.storedAtUnixSeconds > before) return false;
      if (minimum !== null && entry.sizeBytes < minimum) return false;
      if (maximum !== null && entry.sizeBytes > maximum) return false;
      if (!query) return true;
      const collectionName = organization?.collectionId == null
        ? ""
        : collections.get(organization.collectionId) ?? "";
      return [entry.title, entry.author, entry.resourceId, collectionName, ...(organization?.tags ?? [])]
        .some((value) => value.toLocaleLowerCase().includes(query));
    });
    result.sort((left, right) => {
      if (sortOrder === "oldest") return left.storedAtUnixSeconds - right.storedAtUnixSeconds;
      if (sortOrder === "title") return left.title.localeCompare(right.title, currentAppLocale());
      if (sortOrder === "size") return right.sizeBytes - left.sizeBytes;
      return right.storedAtUnixSeconds - left.storedAtUnixSeconds;
    });
    return result;
  });

  function dateBoundary(value: string, endOfDay: boolean): number | null {
    if (!value) return null;
    const timestamp = Date.parse(`${value}T${endOfDay ? "23:59:59" : "00:00:00"}`);
    return Number.isFinite(timestamp) ? Math.floor(timestamp / 1000) : null;
  }

  function byteBoundary(value: string): number | null {
    if (!value.trim()) return null;
    const parsed = Number(value);
    return Number.isFinite(parsed) && parsed >= 0 ? Math.floor(parsed * 1024 * 1024) : null;
  }

  function tagList(value: string): string[] {
    return [...new Set(value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean))];
  }

  onMount(() => {
    let disposed = false;
    let unlisten: UnlistenFn | null = null;
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;
    const initialRefreshTimer = setTimeout(() => {
      if (!restoredFromHistory) void refreshAll();
    }, 0);

    void listen<DownloadTask | null>("pixiv-download-queue-changed", ({ payload }) => {
      if (disposed) return;
      if (refreshTimer !== null) clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => {
        refreshTimer = null;
        void refreshQueue(false);
        if (payload?.state === "completed") void refreshLibrary(false);
      }, 120);
    }).then((listener) => {
      if (disposed) listener();
      else unlisten = listener;
    }).catch(() => {
      if (!disposed) {
        queueError = m.offline_queue_listen_failed();
      }
    });

    return () => {
      disposed = true;
      clearTimeout(initialRefreshTimer);
      if (refreshTimer !== null) clearTimeout(refreshTimer);
      unlisten?.();
    };
  });

  async function refreshAll() {
    await Promise.all([refreshLibrary(true), refreshQueue(true), refreshExportDestination()]);
  }

  async function refreshExportDestination() {
    try {
      exportDestination = await getExportDestinationStatus();
    } catch {
      exportDestination = null;
    }
  }

  async function refreshLibrary(showLoading: boolean) {
    const requestId = ++libraryRequestId;
    if (showLoading) libraryStatus = "loading";
    libraryError = "";
    try {
      const [nextEntries, nextStats] = await Promise.all([
        listOfflineEntries(),
        getOfflineStats(),
      ]);
      if (requestId !== libraryRequestId) return;
      entries = nextEntries;
      selectedEntryKeys = selectedEntryKeys.filter((key) => nextEntries.some((entry) => entry.key === key));
      stats = nextStats;
      libraryStatus = "ready";
      await refreshCatalog(showLoading);
    } catch (error) {
      if (requestId !== libraryRequestId) return;
      libraryError = describeDataFailure(error);
      libraryStatus = "error";
    }
  }

  async function refreshCatalog(showLoading: boolean) {
    const requestId = ++catalogRequestId;
    if (showLoading) catalogStatus = "loading";
    catalogError = "";
    try {
      const snapshot = await getLocalCatalogSnapshot();
      if (requestId !== catalogRequestId) return;
      catalog = snapshot;
      catalogStatus = "ready";
      if (collectionFilter !== "all" && collectionFilter !== "unfiled") {
        const selected = Number(collectionFilter);
        if (!snapshot.collections.some((collection) => collection.id === selected)) {
          collectionFilter = "all";
        }
      }
      if (tagFilter !== "all" && !snapshot.entries.some((entry) => entry.tags.includes(tagFilter))) {
        tagFilter = "all";
      }
    } catch (error) {
      if (requestId !== catalogRequestId) return;
      catalogError = describeDataFailure(error);
      catalogStatus = "error";
    }
  }

  async function refreshQueue(showLoading: boolean) {
    const requestId = ++queueRequestId;
    if (showLoading) queueStatus = "loading";
    queueError = "";
    try {
      const nextTasks = await listDownloadTasks();
      if (requestId !== queueRequestId) return;
      tasks = nextTasks;
      queueStatus = "ready";
    } catch (error) {
      if (requestId !== queueRequestId) return;
      queueError = describeDataFailure(error);
      queueStatus = "error";
    }
  }

  async function removeEntry(key: string) {
    if (removing) return;
    removing = key;
    libraryError = "";
    try {
      await removeOfflineEntry(key);
      confirming = "";
      if (organizingKey === key) closeOrganizationEditor();
      await refreshLibrary(false);
    } catch (error) {
      libraryError = describeDataFailure(error);
    } finally {
      removing = "";
    }
  }

  function organizationFor(key: string): EntryOrganization | undefined {
    return catalog.entries.find((entry) => entry.entryKey === key);
  }

  function collectionName(collectionId?: number | null): string {
    if (collectionId == null) return "";
    return catalog.collections.find((collection) => collection.id === collectionId)?.name ?? "";
  }

  function showCatalogNotice(message: string, isError = false) {
    catalogNotice = message;
    catalogNoticeIsError = isError;
  }

  async function createCollection(event: SubmitEvent) {
    event.preventDefault();
    if (catalogAction) return;
    catalogAction = "create";
    showCatalogNotice("");
    try {
      const created = await createLocalCollection(newCollectionName);
      newCollectionName = "";
      showCatalogNotice(m.offline_collection_created({ name: created.name }));
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  function beginRenameCollection(collectionId: number, name: string) {
    renamingCollectionId = collectionId;
    renameCollectionName = name;
    confirmingCollectionId = null;
  }

  async function saveCollectionRename(event: SubmitEvent) {
    event.preventDefault();
    if (catalogAction || renamingCollectionId === null) return;
    const collectionId = renamingCollectionId;
    catalogAction = `rename-${collectionId}`;
    showCatalogNotice("");
    try {
      const renamed = await renameLocalCollection(collectionId, renameCollectionName);
      renamingCollectionId = null;
      renameCollectionName = "";
      showCatalogNotice(m.offline_collection_renamed({ name: renamed.name }));
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  async function removeCollection(collectionId: number) {
    if (catalogAction) return;
    if (confirmingCollectionId !== collectionId) {
      confirmingCollectionId = collectionId;
      return;
    }
    catalogAction = `delete-${collectionId}`;
    showCatalogNotice("");
    try {
      await deleteLocalCollection(collectionId);
      confirmingCollectionId = null;
      if (collectionFilter === String(collectionId)) collectionFilter = "all";
      showCatalogNotice(m.offline_collection_deleted());
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  function openOrganizationEditor(entry: OfflineEntry) {
    const organization = organizationFor(entry.key);
    organizingKey = entry.key;
    organizationCollectionId = organization?.collectionId == null
      ? ""
      : String(organization.collectionId);
    organizationTags = organization?.tags.join("，") ?? "";
    confirming = "";
    showCatalogNotice("");
  }

  function closeOrganizationEditor() {
    organizingKey = "";
    organizationCollectionId = "";
    organizationTags = "";
  }

  async function saveOrganization(event: SubmitEvent, entry: OfflineEntry) {
    event.preventDefault();
    if (catalogAction) return;
    const tags = organizationTags
      .split(/[,，;；\n]/)
      .map((tag) => tag.trim())
      .filter(Boolean);
    const collectionId = organizationCollectionId ? Number(organizationCollectionId) : null;
    catalogAction = `organize-${entry.key}`;
    showCatalogNotice("");
    try {
      await organizeOfflineEntry(entry.key, collectionId, tags);
      closeOrganizationEditor();
      showCatalogNotice(m.offline_organization_updated({ title: entry.title || entry.resourceId }));
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  function resetLibraryFilters() {
    libraryQuery = "";
    kindFilter = "all";
    collectionFilter = "all";
    tagFilter = "all";
    sortOrder = "newest";
    storedAfter = "";
    storedBefore = "";
    minSizeBytes = "";
    maxSizeBytes = "";
  }

  async function saveCurrentFilter(event: SubmitEvent) {
    event.preventDefault();
    if (catalogAction || !savedFilterName.trim()) return;
    catalogAction = "save-filter";
    showCatalogNotice("");
    try {
      await saveLocalCatalogFilter({
        name: savedFilterName,
        query: libraryQuery,
        kind: kindFilter === "all" ? null : kindFilter,
        collectionId: /^\d+$/.test(collectionFilter) ? Number(collectionFilter) : null,
        tag: tagFilter === "all" ? null : tagFilter,
        sortOrder,
        storedAfter: dateBoundary(storedAfter, false),
        storedBefore: dateBoundary(storedBefore, true),
        minSizeBytes: byteBoundary(minSizeBytes),
        maxSizeBytes: byteBoundary(maxSizeBytes),
      });
      savedFilterName = "";
      showCatalogNotice(m.offline_filter_saved());
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  function applySavedFilter(filter: SavedCatalogFilter) {
    libraryQuery = filter.query;
    kindFilter = filter.kind ?? "all";
    collectionFilter = filter.collectionId == null ? "all" : String(filter.collectionId);
    tagFilter = filter.tag ?? "all";
    sortOrder = filter.sortOrder;
    storedAfter = filter.storedAfter == null ? "" : new Date(filter.storedAfter * 1000).toISOString().slice(0, 10);
    storedBefore = filter.storedBefore == null ? "" : new Date(filter.storedBefore * 1000).toISOString().slice(0, 10);
    minSizeBytes = filter.minSizeBytes == null ? "" : String(Math.round(filter.minSizeBytes / 1024 / 1024));
    maxSizeBytes = filter.maxSizeBytes == null ? "" : String(Math.round(filter.maxSizeBytes / 1024 / 1024));
  }

  async function removeSavedFilter(filterId: number) {
    if (catalogAction) return;
    catalogAction = `delete-filter-${filterId}`;
    try {
      await deleteLocalCatalogFilter(filterId);
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  function toggleSelection(key: string) {
    selectedEntryKeys = selectedEntryKeys.includes(key)
      ? selectedEntryKeys.filter((entryKey) => entryKey !== key)
      : [...selectedEntryKeys, key];
    batchConfirmDelete = false;
  }

  function selectVisibleEntries() {
    const visible = filteredEntries.map((entry) => entry.key);
    selectedEntryKeys = visible.every((key) => selectedEntryKeys.includes(key))
      ? selectedEntryKeys.filter((key) => !visible.includes(key))
      : [...new Set([...selectedEntryKeys, ...visible])];
    batchConfirmDelete = false;
  }

  async function applyBatchOrganization() {
    if (catalogAction || selectedEntryKeys.length === 0) return;
    catalogAction = "batch-organize";
    showCatalogNotice("");
    try {
      await batchOrganizeOfflineEntries({
        entryKeys: selectedEntryKeys,
        updateCollection: batchCollectionId !== "keep",
        collectionId: batchCollectionId && batchCollectionId !== "keep" ? Number(batchCollectionId) : null,
        addTags: tagList(batchAddTags),
        removeTags: tagList(batchRemoveTags),
      });
      batchAddTags = "";
      batchRemoveTags = "";
      showCatalogNotice(m.offline_batch_updated({ count: selectedEntryKeys.length }));
      await refreshCatalog(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  async function removeSelectedEntries() {
    if (!batchConfirmDelete) {
      batchConfirmDelete = true;
      return;
    }
    if (catalogAction || selectedEntryKeys.length === 0) return;
    catalogAction = "batch-delete";
    try {
      const removed = await batchRemoveOfflineEntries(selectedEntryKeys);
      selectedEntryKeys = [];
      batchConfirmDelete = false;
      showCatalogNotice(m.offline_batch_deleted({ count: removed.length }));
      await refreshLibrary(false);
    } catch (error) {
      showCatalogNotice(describeDataFailure(error), true);
    } finally {
      catalogAction = "";
    }
  }

  async function scanDuplicates() {
    if (duplicateStatus === "loading") return;
    duplicateStatus = "loading";
    showCatalogNotice("");
    try {
      duplicateGroups = await findOfflineDuplicates();
      duplicateStatus = "ready";
    } catch (error) {
      duplicateStatus = "error";
      showCatalogNotice(describeDataFailure(error), true);
    }
  }

  function requestRemoval(key: string) {
    if (confirming === key) void removeEntry(key);
    else confirming = key;
  }

  async function exportEntry(entry: OfflineEntry) {
    if (exporting || removing) return;
    exporting = entry.key;
    exportNotice = "";
    exportNoticeIsError = false;
    try {
      const result = await exportOfflineEntry(entry.key);
      exportNotice = m.offline_export_success({ count: result.fileCount, destination: result.destination });
    } catch (error) {
      exportNotice = describeDataFailure(error);
      exportNoticeIsError = true;
      await refreshExportDestination();
    } finally {
      exporting = "";
    }
  }

  async function changeTaskState(task: DownloadTask) {
    if (taskActionId !== null) return;
    taskActionId = task.id;
    queueError = "";
    confirmingTaskId = null;
    try {
      if (task.state === "queued" || task.state === "running") {
        await pauseDownloadTask(task.id);
      } else if (task.state === "paused" || task.state === "failed") {
        await resumeDownloadTask(task.id);
      }
      await refreshQueue(false);
    } catch (error) {
      queueError = describeDataFailure(error);
    } finally {
      taskActionId = null;
    }
  }

  async function removeTask(task: DownloadTask) {
    if (taskActionId !== null || task.state === "running") return;
    if (confirmingTaskId !== task.id) {
      confirmingTaskId = task.id;
      return;
    }
    taskActionId = task.id;
    queueError = "";
    try {
      await removeDownloadTask(task.id);
      confirmingTaskId = null;
      await refreshQueue(false);
    } catch (error) {
      queueError = describeDataFailure(error);
    } finally {
      taskActionId = null;
    }
  }

  function formatBytes(value: number): string {
    if (value < 1024) return `${value} B`;
    if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
    return `${(value / 1024 ** 3).toFixed(2)} GiB`;
  }

  function entryHref(entry: OfflineEntry): string {
    return `/offline/${entry.kind === "artwork" ? "artworks" : entry.kind === "novel" ? "novels" : "ugoira"}/${entry.resourceId}`;
  }

  function taskHref(task: DownloadTask): string {
    return `/offline/${task.kind === "artwork" ? "artworks" : task.kind === "novel" ? "novels" : "ugoira"}/${task.resourceId}`;
  }

  function kindLabel(kind: DownloadKind): string {
    return kind === "artwork"
      ? m.offline_kind_artwork()
      : kind === "novel"
        ? m.offline_kind_novel()
        : "Ugoira";
  }

  const stateLabels: Record<DownloadState, () => string> = {
    queued: m.offline_state_queued,
    running: m.offline_state_running,
    paused: m.offline_state_paused,
    failed: m.offline_state_failed,
    completed: m.offline_state_completed,
  };

  const failureLabels: Record<DownloadFailure, () => string> = {
    authentication: m.offline_failure_auth,
    network: m.offline_failure_network,
    invalid_response: m.offline_failure_response,
    storage: m.offline_failure_storage,
    interrupted: m.offline_failure_interrupted,
  };

  function progressPercent(task: DownloadTask): number {
    if (task.state === "completed") return 100;
    if (task.totalItems === 0) return 0;
    return Math.min(100, Math.round((task.completedItems / task.totalItems) * 100));
  }
</script>

<svelte:head><title>{m.offline_title()} · PixNya</title></svelte:head>

<AppShell title={m.offline_title()}>
  <main class="offline-page">
    <header class="page-header">
      <div>
        <h1 class="page-title">{m.offline_title()}</h1>
      </div>
      <div class="stats"><strong>{stats.entryCount}</strong><span>{m.offline_stats({ size: formatBytes(stats.sizeBytes) })}</span></div>
    </header>

    <section class="content-section queue-section" aria-labelledby="queue-title">
      <div class="section-heading">
        <div><span class="heading-icon"><Icon name="download" size={18} /></span><div><h2 id="queue-title">{m.offline_queue_title()}</h2></div></div>
        <button class="section-refresh" type="button" disabled={queueStatus === "loading"} onclick={() => refreshQueue(true)}>{m.common_refresh()}</button>
      </div>

      {#if queueStatus === "loading"}
        <div class="compact-state"><span class="spinner"></span><p>{m.offline_queue_loading()}</p></div>
      {:else if queueStatus === "error"}
        <div class="compact-state error" role="alert"><p>{queueError}</p><button type="button" onclick={() => refreshQueue(true)}>{m.common_retry()}</button></div>
      {:else if tasks.length === 0}
        <div class="queue-empty"><p>{m.offline_queue_empty()}</p><span>{m.offline_queue_empty_hint()}</span></div>
      {:else}
        <div class="task-list">
          {#each tasks as task (task.id)}
            <article class="task-row">
              <span class="task-kind">{kindLabel(task.kind)}</span>
              <div class="task-main">
                <div class="task-title-line">
                  <h3>{task.title || m.offline_work_fallback({ id: task.resourceId })}</h3>
                  <span class:failed={task.state === "failed"} class:done={task.state === "completed"} class="state-badge">{stateLabels[task.state]()}</span>
                </div>
                <p>{task.author || m.offline_unknown_author()}{task.attemptCount > 1 ? ` · ${m.offline_attempt({ count: task.attemptCount })}` : ""}</p>
                <div class="progress-line">
                  <div class="progress-track" role="progressbar" aria-label={m.offline_download_progress({ title: task.title || task.resourceId })} aria-valuemin="0" aria-valuemax="100" aria-valuenow={progressPercent(task)}>
                    <span style={`width: ${progressPercent(task)}%`}></span>
                  </div>
                  <small>{task.totalItems > 0 ? `${task.completedItems}/${task.totalItems}` : stateLabels[task.state]()} · {formatBytes(task.downloadedBytes)}</small>
                </div>
                {#if task.failure}<p class="failure-note">{failureLabels[task.failure]()}</p>{/if}
              </div>
              <div class="task-actions">
                {#if task.state === "completed"}
                  <a href={taskHref(task)}>{m.offline_open()}</a>
                {:else}
                  <button type="button" disabled={taskActionId !== null} onclick={() => changeTaskState(task)}>
                    {taskActionId === task.id ? m.offline_processing() : task.state === "queued" || task.state === "running" ? m.offline_pause() : task.state === "failed" ? m.common_retry() : m.offline_continue()}
                  </button>
                {/if}
                <button class:confirm={confirmingTaskId === task.id} type="button" disabled={taskActionId !== null || task.state === "running"} title={task.state === "running" ? m.offline_pause_first() : m.offline_remove_queue_only()} onclick={() => removeTask(task)}>
                  {taskActionId === task.id ? m.offline_processing() : confirmingTaskId === task.id ? m.offline_remove_confirm() : m.offline_remove()}
                </button>
              </div>
            </article>
          {/each}
        </div>
      {/if}
      {#if queueError && queueStatus === "ready"}<p class="inline-error" role="alert">{queueError}</p>{/if}
    </section>

    <section class="content-section library-section" aria-labelledby="library-title">
      <div class="section-heading">
        <div><span class="heading-icon library"><Icon name="book" size={18} /></span><div><h2 id="library-title">{m.offline_library_title()}</h2></div></div>
        <button class="section-refresh" type="button" disabled={libraryStatus === "loading"} onclick={() => refreshLibrary(true)}>{m.common_refresh()}</button>
      </div>

      {#if exportDestination && !exportDestination.configured}
        <p class="export-guidance">{m.offline_export_guidance_before()}<a href="/settings#storage">{m.offline_settings_link()}</a>{m.offline_export_guidance_after()}</p>
      {:else if exportDestination?.configured}
        <p class="export-guidance ready">{m.offline_export_current({ label: exportDestination.label ?? "", mode: exportDestination.autoExport ? m.offline_export_auto() : m.offline_export_manual() })}</p>
      {/if}
      {#if exportNotice}<p class="export-notice" class:error={exportNoticeIsError} role="status">{exportNotice}</p>{/if}
      {#if catalogNotice}<p class="catalog-notice" class:error={catalogNoticeIsError} role="status">{catalogNotice}</p>{/if}

      {#if entries.length > 0}
        <div class="catalog-tools" aria-label={m.offline_filter_label()}>
          <label class="library-search"><span>{m.offline_search_local()}</span><input bind:value={libraryQuery} type="search" maxlength="120" placeholder={m.offline_search_placeholder()} /></label>
          <label><span>{m.offline_filter_type()}</span><select bind:value={kindFilter}><option value="all">{m.offline_all_types()}</option><option value="artwork">{m.offline_kind_artwork()}</option><option value="novel">{m.offline_kind_novel()}</option><option value="ugoira">Ugoira</option></select></label>
          <label><span>{m.offline_filter_collection()}</span><select bind:value={collectionFilter} disabled={catalogStatus !== "ready"}><option value="all">{m.offline_all_collections()}</option><option value="unfiled">{m.offline_unfiled()}</option>{#each catalog.collections as collection (collection.id)}<option value={String(collection.id)}>{collection.name} ({collection.entryCount})</option>{/each}</select></label>
          <label><span>{m.offline_filter_tags()}</span><select bind:value={tagFilter} disabled={catalogStatus !== "ready"}><option value="all">{m.offline_all_tags()}</option>{#each availableTags as tag (tag)}<option value={tag}>{tag}</option>{/each}</select></label>
          <label><span>{m.offline_sort()}</span><select bind:value={sortOrder}><option value="newest">{m.offline_sort_newest()}</option><option value="oldest">{m.offline_sort_oldest()}</option><option value="title">{m.offline_sort_title()}</option><option value="size">{m.offline_sort_size()}</option></select></label>
          <button type="button" onclick={resetLibraryFilters}>{m.common_reset()}</button>
        </div>
        <details class="advanced-catalog">
          <summary>{m.offline_advanced_filters()}</summary>
          <div class="advanced-grid">
            <label><span>{m.offline_stored_after()}</span><input bind:value={storedAfter} type="date" /></label>
            <label><span>{m.offline_stored_before()}</span><input bind:value={storedBefore} type="date" /></label>
            <label><span>{m.offline_min_size()}</span><input bind:value={minSizeBytes} type="number" min="0" step="1" /></label>
            <label><span>{m.offline_max_size()}</span><input bind:value={maxSizeBytes} type="number" min="0" step="1" /></label>
          </div>
          <form class="saved-filter-form" onsubmit={saveCurrentFilter}>
            <input bind:value={savedFilterName} maxlength="128" placeholder={m.offline_filter_name()} />
            <button disabled={!!catalogAction || !savedFilterName.trim()}>{m.offline_save_filter()}</button>
          </form>
          {#if catalog.savedFilters.length}
            <div class="saved-filters">
              {#each catalog.savedFilters as filter (filter.id)}
                <button type="button" onclick={() => applySavedFilter(filter)}>{filter.name}</button>
                <button type="button" class="danger" aria-label={m.common_delete()} onclick={() => removeSavedFilter(filter.id)}>×</button>
              {/each}
            </div>
          {/if}
        </details>
      {/if}

      {#if entries.length > 0}
        <details class="advanced-catalog batch-panel">
          <summary>{m.offline_batch_title({ count: selectedEntryKeys.length })}</summary>
          <div class="batch-toolbar">
            <button type="button" onclick={selectVisibleEntries}>{m.offline_select_visible()}</button>
            <select bind:value={batchCollectionId}><option value="keep">{m.offline_keep_collections()}</option><option value="">{m.offline_no_collection()}</option>{#each catalog.collections as collection (collection.id)}<option value={String(collection.id)}>{collection.name}</option>{/each}</select>
            <input bind:value={batchAddTags} placeholder={m.offline_batch_add_tags()} />
            <input bind:value={batchRemoveTags} placeholder={m.offline_batch_remove_tags()} />
            <button type="button" disabled={!selectedEntryKeys.length || !!catalogAction} onclick={applyBatchOrganization}>{m.offline_apply_batch()}</button>
            <button type="button" class:confirm={batchConfirmDelete} disabled={!selectedEntryKeys.length || !!catalogAction} onclick={removeSelectedEntries}>{batchConfirmDelete ? m.common_confirm_delete() : m.offline_delete_selected()}</button>
          </div>
          <div class="duplicate-tools">
            <button type="button" disabled={duplicateStatus === "loading"} onclick={scanDuplicates}>{duplicateStatus === "loading" ? m.account_controls_loading() : m.offline_scan_duplicates()}</button>
            <p>{m.offline_duplicate_report_only()}</p>
            {#if duplicateStatus === "ready"}
              {#each duplicateGroups as group (`${group.reason}-${group.signature}`)}
                <div class="duplicate-group"><strong>{group.reason === "resource_id" ? m.offline_duplicate_resource() : m.offline_duplicate_hash()}</strong><span>{group.entryKeys.join(" · ")}</span></div>
              {:else}<p>{m.offline_no_duplicates()}</p>{/each}
            {/if}
          </div>
        </details>
      {/if}

      <details class="collection-manager">
          <summary><span>{m.offline_manage_collections()}</span><small>{m.offline_collection_count({ count: catalog.collections.length })}</small></summary>
          <div class="collection-manager-body">
            <form class="new-collection" onsubmit={createCollection}>
              <label for="new-collection-name">{m.offline_new_collection()}</label>
              <input id="new-collection-name" bind:value={newCollectionName} maxlength="128" placeholder={m.offline_collection_example()} />
              <button type="submit" disabled={!!catalogAction || catalogStatus !== "ready"}>{catalogAction === "create" ? m.offline_creating() : m.offline_create()}</button>
            </form>
            {#if catalogStatus === "loading"}
              <p class="catalog-state">{m.offline_collections_loading()}</p>
            {:else if catalogStatus === "error"}
              <p class="catalog-state error" role="alert">{catalogError}<button type="button" onclick={() => refreshCatalog(true)}>{m.common_retry()}</button></p>
            {:else if catalog.collections.length === 0}
              <p class="catalog-state">{m.offline_collections_empty()}</p>
            {:else}
              <div class="collection-list">
                {#each catalog.collections as collection (collection.id)}
                  <div class="collection-row">
                    {#if renamingCollectionId === collection.id}
                      <form onsubmit={saveCollectionRename}>
                        <input bind:value={renameCollectionName} maxlength="128" aria-label={m.offline_rename_collection({ name: collection.name })} />
                        <button type="submit" disabled={!!catalogAction}>{catalogAction === `rename-${collection.id}` ? m.common_saving() : m.common_save()}</button>
                        <button type="button" disabled={!!catalogAction} onclick={() => renamingCollectionId = null}>{m.common_cancel()}</button>
                      </form>
                    {:else}
                      <div><strong>{collection.name}</strong><span>{m.offline_collection_entries({ count: collection.entryCount })}</span></div>
                      <div class="collection-actions">
                        <button type="button" disabled={!!catalogAction} onclick={() => beginRenameCollection(collection.id, collection.name)}>{m.common_rename()}</button>
                        <button class:confirm={confirmingCollectionId === collection.id} type="button" disabled={!!catalogAction} onclick={() => removeCollection(collection.id)}>{catalogAction === `delete-${collection.id}` ? m.common_deleting() : confirmingCollectionId === collection.id ? m.common_confirm_delete() : m.common_delete()}</button>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
      </details>

      {#if libraryStatus === "loading"}
        <div class="state"><span class="spinner"></span><p>{m.offline_library_loading()}</p></div>
      {:else if libraryStatus === "error"}
        <div class="state error" role="alert"><p>{libraryError}</p><button type="button" onclick={() => refreshLibrary(true)}>{m.common_retry()}</button></div>
      {:else if entries.length === 0}
        <div class="empty"><Icon name="download" size={30} /><h2>{m.offline_empty()}</h2><p>{m.offline_empty_hint()}</p></div>
      {:else if filteredEntries.length === 0}
        <div class="empty filter-empty"><Icon name="search" size={27} /><h2>{m.offline_filter_empty()}</h2><p>{m.offline_filter_empty_hint({ count: entries.length })}</p><button type="button" onclick={resetLibraryFilters}>{m.offline_reset_filters()}</button></div>
      {:else}
        <p class="filter-summary">{m.offline_filter_summary({ visible: filteredEntries.length, total: entries.length })}</p>
        <div class="entries">
          {#each filteredEntries as entry (entry.key)}
            {@const organization = organizationFor(entry.key)}
            <article class="entry-row">
              <label class="entry-select"><input type="checkbox" checked={selectedEntryKeys.includes(entry.key)} onchange={() => toggleSelection(entry.key)} aria-label={m.offline_select_entry({ title: entry.title || entry.resourceId })} /></label>
              <a class="entry-main" href={entryHref(entry)}>
                <span class="kind">{kindLabel(entry.kind)}</span>
                <div class="entry-copy">
                  <h2>{entry.title || m.offline_work_fallback({ id: entry.resourceId })}</h2>
                  <p>{entry.author || m.offline_unknown_author()} · {m.offline_file_count({ count: entry.assetCount })} · {formatBytes(entry.sizeBytes)}</p>
                  {#if organization?.collectionId != null || organization?.tags.length}
                    <div class="organization-badges">
                      {#if organization.collectionId != null}<span class="collection-badge">{collectionName(organization.collectionId)}</span>{/if}
                      {#each organization.tags as tag (tag)}<span>#{tag}</span>{/each}
                    </div>
                  {/if}
                </div>
                <b>{m.offline_open()} ›</b>
              </a>
              <div class="entry-actions">
                <button type="button" disabled={catalogStatus !== "ready" || !!catalogAction || !!exporting || !!removing} onclick={() => organizingKey === entry.key ? closeOrganizationEditor() : openOrganizationEditor(entry)}>{organizingKey === entry.key ? m.offline_collapse() : m.offline_organize()}</button>
                <button type="button" disabled={!exportDestination?.configured || !!exporting || !!removing} onclick={() => exportEntry(entry)}>
                  {exporting === entry.key ? m.offline_exporting() : m.offline_export()}
                </button>
                <button type="button" class:confirm={confirming === entry.key} disabled={!!exporting || removing === entry.key} onclick={() => requestRemoval(entry.key)}>{removing === entry.key ? m.common_deleting() : confirming === entry.key ? m.common_confirm_delete() : m.common_delete()}</button>
              </div>
              {#if organizingKey === entry.key}
                <form class="organize-editor" onsubmit={(event) => saveOrganization(event, entry)}>
                  <div>
                    <label for={`collection-${entry.key}`}>{m.offline_local_collection()}</label>
                    <select id={`collection-${entry.key}`} bind:value={organizationCollectionId}><option value="">{m.offline_no_collection()}</option>{#each catalog.collections as collection (collection.id)}<option value={String(collection.id)}>{collection.name}</option>{/each}</select>
                  </div>
                  <div class="tag-editor">
                    <label for={`tags-${entry.key}`}>{m.offline_local_tags()}</label>
                    <input id={`tags-${entry.key}`} bind:value={organizationTags} maxlength="768" placeholder={m.offline_tags_placeholder()} />
                  </div>
                  <div class="organize-actions"><button type="button" disabled={!!catalogAction} onclick={closeOrganizationEditor}>{m.common_cancel()}</button><button type="submit" disabled={!!catalogAction}>{catalogAction === `organize-${entry.key}` ? m.common_saving() : m.offline_save_organization()}</button></div>
                </form>
              {/if}
            </article>
          {/each}
        </div>
      {/if}
      {#if libraryError && libraryStatus === "ready"}<p class="inline-error" role="alert">{libraryError}</p>{/if}
    </section>
  </main>
</AppShell>

<style>
  .offline-page { width: min(980px, 100%); margin: 0 auto; padding: 26px 28px 70px; }
  .page-header { display: flex; gap: 20px; align-items: center; justify-content: space-between; }
  .page-header h1 { margin: 0; font-size: var(--type-title); }
  .stats { min-width: 120px; padding: 12px 16px; border: 1px solid var(--line); border-radius: 10px; background: white; text-align: right; }
  .stats strong, .stats span { display: block; }
  .stats strong { font-size: var(--type-section); }
  .stats span { margin-top: 3px; color: var(--muted); font-size: var(--type-caption); }
  .content-section { overflow: hidden; margin-top: 22px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .section-heading { display: flex; min-height: 68px; gap: 14px; align-items: center; justify-content: space-between; padding: 14px 16px; border-bottom: 1px solid var(--line); }
  .section-heading > div { display: flex; min-width: 0; gap: 11px; align-items: center; }
  .section-heading h2 { margin: 0; }
  .section-heading h2 { font-size: var(--type-body); }
  .section-heading button, .compact-state button, .state button { padding: 8px 15px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 18px; background: #f5fbff; cursor: pointer; font-size: var(--type-body); }
  .section-refresh { min-width: 68px; flex: 0 0 auto; white-space: nowrap; word-break: keep-all; }
  .section-heading button:disabled { cursor: default; opacity: .55; }
  .heading-icon { display: grid; width: 38px; height: 38px; flex: 0 0 auto; place-items: center; color: var(--pixiv-blue); border-radius: 50%; background: #eaf7ff; }
  .heading-icon.library { color: #4b8b6b; background: #edf9f2; }
  .task-list { display: grid; }
  .task-row { display: grid; grid-template-columns: 82px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 14px 16px; border-bottom: 1px solid var(--line); }
  .task-row:last-child { border-bottom: 0; }
  .task-kind { display: grid; height: 46px; place-items: center; color: #4e7b94; border-radius: 7px; background: #edf7fc; font-size: var(--type-caption); font-weight: 700; }
  .task-main { min-width: 0; }
  .task-title-line { display: flex; min-width: 0; gap: 8px; align-items: center; }
  .task-title-line h3 { overflow: hidden; margin: 0; font-size: var(--type-small); text-overflow: ellipsis; white-space: nowrap; }
  .task-main > p { margin: 4px 0 0; color: var(--muted); font-size: var(--type-caption); }
  .state-badge { flex: 0 0 auto; padding: 3px 7px; color: #54788c; border-radius: 10px; background: #edf7fc; font-size: var(--type-caption); font-weight: 700; }
  .state-badge.failed { color: #a24e5c; background: #fff0f3; }
  .state-badge.done { color: #3b7a58; background: #eaf8f0; }
  .progress-line { display: flex; gap: 9px; align-items: center; margin-top: 8px; }
  .progress-track { overflow: hidden; height: 5px; flex: 1 1 auto; border-radius: 4px; background: #e9eef2; }
  .progress-track span { display: block; height: 100%; border-radius: inherit; background: var(--pixiv-blue); transition: width .18s ease; }
  .progress-line small { flex: 0 0 auto; color: var(--muted); font-size: var(--type-caption); }
  .task-main .failure-note { color: #a24e5c; }
  .task-actions { display: flex; gap: 7px; }
  .task-actions button, .task-actions a { min-width: 52px; padding: 8px 11px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 17px; background: white; cursor: pointer; font-size: var(--type-body); text-align: center; text-decoration: none; }
  .task-actions button:last-child { color: #8b6570; border-color: #eadbe0; }
  .task-actions button.confirm { color: white; border-color: #b24d5e; background: #b24d5e; }
  .task-actions button:disabled { cursor: default; opacity: .48; }
  .compact-state, .queue-empty { display: grid; min-height: 92px; gap: 7px; place-items: center; color: var(--muted); text-align: center; }
  .compact-state p, .queue-empty p, .queue-empty span { margin: 0; font-size: var(--type-caption); }
  .queue-empty p { color: var(--text); font-size: var(--type-small); font-weight: 700; }
  .catalog-tools { display: grid; grid-template-columns: minmax(190px, 1.7fr) repeat(4, minmax(108px, 1fr)) auto; gap: 9px; align-items: end; padding: 14px 16px; border-bottom: 1px solid var(--line); background: #fbfdff; }
  .catalog-tools label { display: grid; min-width: 0; gap: 5px; color: var(--muted); font-size: var(--type-caption); font-weight: 700; }
  .catalog-tools input, .catalog-tools select, .new-collection input, .collection-row input, .organize-editor input, .organize-editor select { width: 100%; height: 34px; min-width: 0; padding: 0 10px; color: var(--text); border: 1px solid #dce4ea; border-radius: 7px; outline: none; background: white; font: inherit; font-size: var(--type-caption); }
  .catalog-tools input:focus, .catalog-tools select:focus, .new-collection input:focus, .collection-row input:focus, .organize-editor input:focus, .organize-editor select:focus { border-color: var(--pixiv-blue); box-shadow: 0 0 0 2px rgba(0,150,250,.1); }
  .catalog-tools select:disabled { opacity: .55; }
  .catalog-tools > button, .filter-empty button { height: 34px; padding: 0 13px; color: #60727d; border: 1px solid #dce4ea; border-radius: 17px; background: white; cursor: pointer; font-size: var(--type-body); }
  .advanced-catalog { padding: 0 16px; border-bottom: 1px solid var(--line); background: #fbfdff; }
  .advanced-catalog summary { padding: 12px 0; cursor: pointer; font-size: var(--type-caption); font-weight: 750; }
  .advanced-grid { display: grid; grid-template-columns: repeat(4,minmax(110px,1fr)); gap: 9px; }
  .advanced-grid label { display: grid; gap: 5px; color: var(--muted); font-size: var(--type-caption); font-weight: 700; }
  .advanced-grid input,.saved-filter-form input,.batch-toolbar input,.batch-toolbar select { width: 100%; height: 34px; min-width: 0; padding: 0 10px; border: 1px solid #dce4ea; border-radius: 7px; background: white; font: inherit; font-size: var(--type-caption); }
  .saved-filter-form { display: flex; gap: 8px; margin: 12px 0; }
  .saved-filter-form button,.saved-filters button,.batch-toolbar button,.duplicate-tools button { min-height: 32px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 16px; background: white; cursor: pointer; font-size: var(--type-body); }
  .saved-filters { display: flex; flex-wrap: wrap; gap: 5px; padding-bottom: 12px; }.saved-filters .danger{min-width:32px;color:#a24e5c;border-color:#eadbe0;margin-left:-5px}
  .batch-panel { background: #f7fbfe; }.batch-toolbar { display:grid;grid-template-columns:auto minmax(110px,1fr) repeat(2,minmax(130px,1fr)) auto auto;gap:8px;padding-bottom:12px}.batch-toolbar button.confirm{color:white;background:#b24d5e;border-color:#b24d5e}.duplicate-tools{padding:12px 0;border-top:1px solid var(--line)}.duplicate-tools>p{color:var(--muted);font-size:var(--type-caption)}.duplicate-group{display:flex;gap:10px;padding:7px 0;font-size:var(--type-caption)}.duplicate-group strong{min-width:80px}.duplicate-group span{overflow-wrap:anywhere}.entry-select{display:grid;align-self:stretch;padding:18px 0 0 14px}.entry-select input{width:16px;height:16px;accent-color:var(--pixiv-blue)}
  .collection-manager { border-bottom: 1px solid var(--line); background: #fff; }
  .collection-manager summary { display: flex; min-height: 43px; gap: 9px; align-items: center; padding: 10px 16px; cursor: pointer; list-style-position: inside; }
  .collection-manager summary span { font-size: var(--type-caption); font-weight: 750; }
  .collection-manager summary small { color: var(--muted); font-size: var(--type-caption); font-weight: 400; }
  .collection-manager-body { padding: 0 16px 14px; }
  .new-collection { display: grid; grid-template-columns: auto minmax(150px, 1fr) auto; gap: 9px; align-items: center; padding: 11px; border-radius: 8px; background: #f6f9fb; }
  .new-collection label { color: #586974; font-size: var(--type-caption); font-weight: 700; }
  .new-collection button, .collection-row button, .catalog-state button, .organize-actions button { height: 32px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 16px; background: white; cursor: pointer; font-size: var(--type-body); }
  .new-collection button:disabled, .collection-row button:disabled, .organize-actions button:disabled { cursor: default; opacity: .5; }
  .catalog-state { margin: 11px 0 0; color: var(--muted); font-size: var(--type-caption); text-align: center; }
  .catalog-state.error { color: #a24e5c; }
  .catalog-state button { margin-left: 8px; }
  .collection-list { display: grid; margin-top: 8px; border: 1px solid var(--line); border-radius: 8px; }
  .collection-row { display: flex; min-height: 48px; gap: 12px; align-items: center; justify-content: space-between; padding: 8px 11px; border-bottom: 1px solid var(--line); }
  .collection-row:last-child { border-bottom: 0; }
  .collection-row > div:first-child strong, .collection-row > div:first-child span { display: block; }
  .collection-row strong { font-size: var(--type-caption); }
  .collection-row span { margin-top: 3px; color: var(--muted); font-size: var(--type-caption); }
  .collection-row > form { display: grid; width: 100%; grid-template-columns: minmax(0,1fr) auto auto; gap: 7px; }
  .collection-actions { display: flex; gap: 7px; }
  .collection-actions button:last-child { color: #a24e5c; border-color: #eadbe0; }
  .collection-actions button.confirm { color: white; border-color: #b24d5e; background: #b24d5e; }
  .filter-summary { margin: 0; padding: 8px 16px; color: var(--muted); border-bottom: 1px solid var(--line); background: #fbfdff; font-size: var(--type-caption); text-align: right; }
  .entries { display: grid; }
  .entry-row { display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; border-bottom: 1px solid var(--line); }
  .entry-row:last-child { border-bottom: 0; }
  .entry-main { display: grid; grid-template-columns: 78px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 16px; color: var(--text); text-decoration: none; }
  .entry-copy { min-width: 0; }
  .kind { display: grid; height: 46px; place-items: center; color: #4e7b94; border-radius: 7px; background: #edf7fc; font-size: var(--type-caption); font-weight: 700; }
  .entry-row h2 { overflow: hidden; margin: 0; font-size: var(--type-body); text-overflow: ellipsis; white-space: nowrap; }
  .entry-row p { margin: 5px 0 0; color: var(--muted); font-size: var(--type-caption); }
  .entry-main > b { color: var(--pixiv-blue); font-size: var(--type-caption); }
  .organization-badges { display: flex; overflow: hidden; gap: 5px; align-items: center; margin-top: 7px; }
  .organization-badges span { overflow: hidden; max-width: 120px; padding: 3px 6px; color: #526f82; border-radius: 10px; background: #edf6fb; font-size: var(--type-caption); text-overflow: ellipsis; white-space: nowrap; }
  .organization-badges span.collection-badge { color: #397457; background: #eaf8f0; font-weight: 700; }
  .entry-actions { display: grid; align-self: stretch; grid-template-columns: repeat(3, minmax(58px, auto)); border-left: 1px solid var(--line); }
  .entry-actions button { min-width: 62px; padding: 0 12px; color: var(--pixiv-blue); border: 0; background: #f8fcff; cursor: pointer; font-size: var(--type-body); }
  .entry-actions button + button { border-left: 1px solid var(--line); }
  .entry-actions button:last-child { color: #a24e5c; background: #fffafb; }
  .entry-actions button.confirm { color: white; background: #b24d5e; }
  .entry-actions button:disabled { cursor: default; opacity: .48; }
  .organize-editor { display: grid; grid-column: 1 / -1; grid-template-columns: minmax(150px,.7fr) minmax(260px,1.6fr) auto; gap: 12px; align-items: end; padding: 14px 16px; border-top: 1px solid var(--line); background: #f8fbfd; }
  .organize-editor > div { display: grid; min-width: 0; gap: 5px; }
  .organize-editor label { color: #53656f; font-size: var(--type-caption); font-weight: 700; }
  .organize-actions { display: flex !important; gap: 7px !important; }
  .organize-actions button:last-child { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .export-guidance, .export-notice { margin: 0; padding: 10px 16px; color: #7a6542; border-bottom: 1px solid #f0e5cf; background: #fffaf1; font-size: var(--type-caption); line-height: 1.55; }
  .export-guidance.ready { color: #397457; border-color: #d8ebdf; background: #f3fbf6; }
  .export-guidance a { margin: 0 3px; color: var(--pixiv-blue); font-weight: 700; }
  .export-notice { color: #397457; border-color: #d8ebdf; background: #f3fbf6; }
  .export-notice.error { color: #a24e5c; border-color: #f2dce2; background: #fff9fa; }
  .catalog-notice { margin: 0; padding: 10px 16px; color: #397457; border-bottom: 1px solid #d8ebdf; background: #f3fbf6; font-size: var(--type-caption); line-height: 1.55; }
  .catalog-notice.error { color: #a24e5c; border-color: #f2dce2; background: #fff9fa; }
  .state, .empty { display: grid; min-height: 180px; gap: 10px; place-items: center; color: var(--muted); text-align: center; }
  .empty h2, .empty p, .state p { margin: 0; }
  .empty h2 { color: var(--text); font-size: var(--type-label); }
  .empty p, .state p { font-size: var(--type-caption); }
  .filter-empty button { margin-top: 2px; }
  .spinner { width: 28px; height: 28px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .compact-state .spinner { width: 20px; height: 20px; border-width: 2px; }
  .inline-error { margin: 0; padding: 9px 16px; color: #a24e5c; border-top: 1px solid #f2dce2; background: #fff9fa; font-size: var(--type-caption); text-align: center; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 620px) {
    .offline-page { padding: 18px 12px 90px; }
    .page-header { align-items: stretch; flex-direction: column; }
    .page-header h1 { font-size: var(--type-section); }
    .stats { text-align: left; }
    .stats span { font-size: var(--type-small); }
    .section-heading { align-items: flex-start; }
    .section-heading h2 { font-size: var(--type-label); }
    .section-heading button { margin-top: 3px; font-size: var(--type-body); }
    .task-row { grid-template-columns: 58px minmax(0, 1fr); gap: 10px; padding: 14px 12px; }
    .task-kind { height: 42px; font-size: var(--type-small); }
    .task-title-line h3 { font-size: var(--type-body); }
    .state-badge { font-size: var(--type-caption); }
    .task-main > p, .task-main .failure-note { font-size: var(--type-small); line-height: 1.45; }
    .progress-line { align-items: flex-start; flex-direction: column; }
    .progress-track { width: 100%; flex: none; }
    .progress-line small { font-size: var(--type-small); }
    .task-actions { grid-column: 1 / -1; justify-content: flex-end; }
    .task-actions button, .task-actions a { min-width: 70px; font-size: var(--type-body); }
    .compact-state p, .queue-empty span { font-size: var(--type-small); }
    .queue-empty p { font-size: var(--type-body); }
    .catalog-tools { grid-template-columns: 1fr 1fr; gap: 10px; padding: 12px; }
    .catalog-tools .library-search { grid-column: 1 / -1; }
    .catalog-tools label { font-size: var(--type-small); }
    .catalog-tools input, .catalog-tools select { height: 40px; font-size: var(--type-small); }
    .catalog-tools > button { height: 40px; align-self: end; font-size: var(--type-body); }
    .collection-manager summary { align-items: flex-start; flex-direction: column; padding: 12px; }
    .collection-manager summary span { font-size: var(--type-small); }
    .collection-manager summary small { font-size: var(--type-small); }
    .collection-manager-body { padding: 0 12px 12px; }
    .new-collection { grid-template-columns: 1fr auto; }
    .new-collection label { grid-column: 1 / -1; font-size: var(--type-small); }
    .new-collection input, .collection-row input { height: 40px; font-size: var(--type-small); }
    .new-collection button, .collection-row button, .catalog-state button { height: 38px; font-size: var(--type-body); }
    .catalog-state { font-size: var(--type-small); line-height: 1.5; }
    .collection-row { min-height: 58px; padding: 9px; }
    .collection-row strong { font-size: var(--type-small); }
    .collection-row span { font-size: var(--type-small); }
    .filter-summary { padding: 9px 12px; font-size: var(--type-small); }
    .entry-main { grid-template-columns: 62px minmax(0, 1fr); }
    .entry-main > b { display: none; }
    .entry-row h2 { font-size: var(--type-body); }
    .entry-row p { font-size: var(--type-small); line-height: 1.45; }
    .advanced-grid,.batch-toolbar { grid-template-columns: 1fr 1fr; }.saved-filter-form{align-items:stretch;flex-direction:column}.advanced-catalog{padding:0 12px}.batch-toolbar button,.batch-toolbar input,.batch-toolbar select{min-height:40px;font-size: var(--type-body)}.duplicate-group{align-items:flex-start;flex-direction:column}
    .entry-row { grid-template-columns: auto minmax(0,1fr); }
    .entry-actions,.organize-editor { grid-column: 1 / -1; }
    .organization-badges { flex-wrap: wrap; }
    .organization-badges span { max-width: 150px; font-size: var(--type-caption); }
    .entry-actions { min-height: 48px; grid-template-columns: repeat(3,1fr); border-top: 1px solid var(--line); border-left: 0; }
    .entry-actions button { font-size: var(--type-body); }
    .organize-editor { grid-template-columns: 1fr; padding: 14px 12px; }
    .organize-editor label { font-size: var(--type-small); }
    .organize-editor input, .organize-editor select { height: 42px; font-size: var(--type-small); }
    .organize-actions { justify-content: flex-end; }
    .organize-actions button { height: 40px; font-size: var(--type-body); }
    .export-guidance, .export-notice { padding: 12px; font-size: var(--type-small); }
    .catalog-notice { padding: 12px; font-size: var(--type-small); }
    .inline-error { font-size: var(--type-small); }
  }
</style>
