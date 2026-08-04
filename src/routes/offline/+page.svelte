<script lang="ts">
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    describeDataFailure,
    createLocalCollection,
    deleteLocalCollection,
    exportOfflineEntry,
    getExportDestinationStatus,
    getLocalCatalogSnapshot,
    getOfflineStats,
    listDownloadTasks,
    listOfflineEntries,
    pauseDownloadTask,
    organizeOfflineEntry,
    removeDownloadTask,
    removeOfflineEntry,
    resumeDownloadTask,
    renameLocalCollection,
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
  let catalog = $state<LocalCatalogSnapshot>({ collections: [], entries: [] });
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
  };

  export const snapshot = {
    capture: () => rememberNavigationView<OfflineLibrarySnapshot>({
      entries, stats, tasks, libraryStatus, queueStatus, libraryError, queueError,
      exportDestination, catalog, catalogStatus, catalogError, libraryQuery,
      kindFilter, collectionFilter, tagFilter, sortOrder,
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
    },
  };

  const availableTags = $derived.by(() => {
    const tags = new Set<string>();
    for (const organization of catalog.entries) {
      for (const tag of organization.tags) tags.add(tag);
    }
    return [...tags].sort((left, right) => left.localeCompare(right, "zh-CN"));
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
      if (!query) return true;
      const collectionName = organization?.collectionId == null
        ? ""
        : collections.get(organization.collectionId) ?? "";
      return [entry.title, entry.author, entry.resourceId, collectionName, ...(organization?.tags ?? [])]
        .some((value) => value.toLocaleLowerCase().includes(query));
    });
    result.sort((left, right) => {
      if (sortOrder === "oldest") return left.storedAtUnixSeconds - right.storedAtUnixSeconds;
      if (sortOrder === "title") return left.title.localeCompare(right.title, "zh-CN");
      if (sortOrder === "size") return right.sizeBytes - left.sizeBytes;
      return right.storedAtUnixSeconds - left.storedAtUnixSeconds;
    });
    return result;
  });

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
        queueError = "无法监听下载进度；可使用刷新按钮读取最新状态。";
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
      showCatalogNotice(`已创建收藏夹“${created.name}”。`);
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
      showCatalogNotice(`收藏夹已更名为“${renamed.name}”。`);
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
      showCatalogNotice("收藏夹已删除；原有标签和离线内容仍保留。 ");
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
      showCatalogNotice(`已更新“${entry.title || entry.resourceId}”的本地整理信息。`);
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
      exportNotice = `已导出 ${result.fileCount} 个文件到 ${result.destination}`;
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
    return kind === "artwork" ? "插画/漫画" : kind === "novel" ? "小说" : "Ugoira";
  }

  const stateLabels: Record<DownloadState, string> = {
    queued: "等待中",
    running: "下载中",
    paused: "已暂停",
    failed: "失败",
    completed: "已完成",
  };

  const failureLabels: Record<DownloadFailure, string> = {
    authentication: "登录状态失效，请重新登录后重试",
    network: "网络请求失败，请检查连接模式后重试",
    invalid_response: "Pixiv 返回的数据无法处理",
    storage: "本机存储写入失败或安全可写空间不足，请在设置中检查存储状态",
    interrupted: "上次退出时下载被中断，已恢复等待",
  };

  function progressPercent(task: DownloadTask): number {
    if (task.state === "completed") return 100;
    if (task.totalItems === 0) return 0;
    return Math.min(100, Math.round((task.completedItems / task.totalItems) * 100));
  }
</script>

<svelte:head><title>离线资料库 · PixNya</title></svelte:head>

<AppShell title="离线资料库">
  <main class="offline-page">
    <header class="page-header">
      <div>
        <h1>离线资料库</h1>
        <p>下载任务断点保存在本机；已完成内容无需登录或网络即可阅读。</p>
      </div>
      <div class="stats"><strong>{stats.entryCount}</strong><span>项 · {formatBytes(stats.sizeBytes)}</span></div>
    </header>

    <section class="content-section queue-section" aria-labelledby="queue-title">
      <div class="section-heading">
        <div><span class="heading-icon"><Icon name="download" size={18} /></span><div><h2 id="queue-title">下载队列</h2><p>任务串行执行；退出应用后仍会保留，重新登录后自动继续。</p></div></div>
        <button class="section-refresh" type="button" disabled={queueStatus === "loading"} onclick={() => refreshQueue(true)}>刷新</button>
      </div>

      {#if queueStatus === "loading"}
        <div class="compact-state"><span class="spinner"></span><p>正在读取下载队列…</p></div>
      {:else if queueStatus === "error"}
        <div class="compact-state error" role="alert"><p>{queueError}</p><button type="button" onclick={() => refreshQueue(true)}>重试</button></div>
      {:else if tasks.length === 0}
        <div class="queue-empty"><p>暂无下载任务</p><span>可在作品或小说详情页点击“离线保存”。</span></div>
      {:else}
        <div class="task-list">
          {#each tasks as task (task.id)}
            <article class="task-row">
              <span class="task-kind">{kindLabel(task.kind)}</span>
              <div class="task-main">
                <div class="task-title-line">
                  <h3>{task.title || `作品 ${task.resourceId}`}</h3>
                  <span class:failed={task.state === "failed"} class:done={task.state === "completed"} class="state-badge">{stateLabels[task.state]}</span>
                </div>
                <p>{task.author || "未知作者"}{task.attemptCount > 1 ? ` · 第 ${task.attemptCount} 次尝试` : ""}</p>
                <div class="progress-line">
                  <div class="progress-track" role="progressbar" aria-label={`${task.title || task.resourceId} 下载进度`} aria-valuemin="0" aria-valuemax="100" aria-valuenow={progressPercent(task)}>
                    <span style={`width: ${progressPercent(task)}%`}></span>
                  </div>
                  <small>{task.totalItems > 0 ? `${task.completedItems}/${task.totalItems}` : stateLabels[task.state]} · {formatBytes(task.downloadedBytes)}</small>
                </div>
                {#if task.failure}<p class="failure-note">{failureLabels[task.failure]}</p>{/if}
              </div>
              <div class="task-actions">
                {#if task.state === "completed"}
                  <a href={taskHref(task)}>打开</a>
                {:else}
                  <button type="button" disabled={taskActionId !== null} onclick={() => changeTaskState(task)}>
                    {taskActionId === task.id ? "处理中…" : task.state === "queued" || task.state === "running" ? "暂停" : task.state === "failed" ? "重试" : "继续"}
                  </button>
                {/if}
                <button class:confirm={confirmingTaskId === task.id} type="button" disabled={taskActionId !== null || task.state === "running"} title={task.state === "running" ? "请先暂停任务" : "仅移除队列记录，不删除已下载内容"} onclick={() => removeTask(task)}>
                  {taskActionId === task.id ? "处理中…" : confirmingTaskId === task.id ? "确认移除" : "移除"}
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
        <div><span class="heading-icon library"><Icon name="book" size={18} /></span><div><h2 id="library-title">已下载内容</h2><p>删除队列记录不会删除这里的文件。</p></div></div>
        <button class="section-refresh" type="button" disabled={libraryStatus === "loading"} onclick={() => refreshLibrary(true)}>刷新</button>
      </div>

      {#if exportDestination && !exportDestination.configured}
        <p class="export-guidance">需要普通文件夹副本时，请先在<a href="/settings#storage">设置</a>中选择导出目录；应用私有离线内容不受影响。</p>
      {:else if exportDestination?.configured}
        <p class="export-guidance ready">当前导出目录：{exportDestination.label}{exportDestination.autoExport ? " · 新下载会自动导出" : " · 仅手动导出"}</p>
      {/if}
      {#if exportNotice}<p class="export-notice" class:error={exportNoticeIsError} role="status">{exportNotice}</p>{/if}
      {#if catalogNotice}<p class="catalog-notice" class:error={catalogNoticeIsError} role="status">{catalogNotice}</p>{/if}

      {#if entries.length > 0}
        <div class="catalog-tools" aria-label="离线资料库筛选">
          <label class="library-search"><span>搜索本地内容</span><input bind:value={libraryQuery} type="search" maxlength="120" placeholder="标题、作者、编号、收藏夹或标签" /></label>
          <label><span>类型</span><select bind:value={kindFilter}><option value="all">全部类型</option><option value="artwork">插画/漫画</option><option value="novel">小说</option><option value="ugoira">Ugoira</option></select></label>
          <label><span>收藏夹</span><select bind:value={collectionFilter} disabled={catalogStatus !== "ready"}><option value="all">全部收藏夹</option><option value="unfiled">未分类</option>{#each catalog.collections as collection (collection.id)}<option value={String(collection.id)}>{collection.name}（{collection.entryCount}）</option>{/each}</select></label>
          <label><span>标签</span><select bind:value={tagFilter} disabled={catalogStatus !== "ready"}><option value="all">全部标签</option>{#each availableTags as tag (tag)}<option value={tag}>{tag}</option>{/each}</select></label>
          <label><span>排序</span><select bind:value={sortOrder}><option value="newest">最近下载</option><option value="oldest">最早下载</option><option value="title">标题</option><option value="size">占用空间</option></select></label>
          <button type="button" onclick={resetLibraryFilters}>重置</button>
        </div>
      {/if}

      <details class="collection-manager">
          <summary><span>管理本地收藏夹</span><small>{catalog.collections.length} 个收藏夹 · 整理信息只保存在本机</small></summary>
          <div class="collection-manager-body">
            <form class="new-collection" onsubmit={createCollection}>
              <label for="new-collection-name">新建收藏夹</label>
              <input id="new-collection-name" bind:value={newCollectionName} maxlength="128" placeholder="例如：绘画参考" />
              <button type="submit" disabled={!!catalogAction || catalogStatus !== "ready"}>{catalogAction === "create" ? "创建中…" : "创建"}</button>
            </form>
            {#if catalogStatus === "loading"}
              <p class="catalog-state">正在读取本地收藏夹…</p>
            {:else if catalogStatus === "error"}
              <p class="catalog-state error" role="alert">{catalogError}<button type="button" onclick={() => refreshCatalog(true)}>重试</button></p>
            {:else if catalog.collections.length === 0}
              <p class="catalog-state">尚未创建收藏夹；也可以直接为内容添加标签。</p>
            {:else}
              <div class="collection-list">
                {#each catalog.collections as collection (collection.id)}
                  <div class="collection-row">
                    {#if renamingCollectionId === collection.id}
                      <form onsubmit={saveCollectionRename}>
                        <input bind:value={renameCollectionName} maxlength="128" aria-label={`重命名 ${collection.name}`} />
                        <button type="submit" disabled={!!catalogAction}>{catalogAction === `rename-${collection.id}` ? "保存中…" : "保存"}</button>
                        <button type="button" disabled={!!catalogAction} onclick={() => renamingCollectionId = null}>取消</button>
                      </form>
                    {:else}
                      <div><strong>{collection.name}</strong><span>{collection.entryCount} 项内容</span></div>
                      <div class="collection-actions">
                        <button type="button" disabled={!!catalogAction} onclick={() => beginRenameCollection(collection.id, collection.name)}>重命名</button>
                        <button class:confirm={confirmingCollectionId === collection.id} type="button" disabled={!!catalogAction} onclick={() => removeCollection(collection.id)}>{catalogAction === `delete-${collection.id}` ? "删除中…" : confirmingCollectionId === collection.id ? "确认删除" : "删除"}</button>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
      </details>

      {#if libraryStatus === "loading"}
        <div class="state"><span class="spinner"></span><p>正在读取本地清单…</p></div>
      {:else if libraryStatus === "error"}
        <div class="state error" role="alert"><p>{libraryError}</p><button type="button" onclick={() => refreshLibrary(true)}>重试</button></div>
      {:else if entries.length === 0}
        <div class="empty"><Icon name="download" size={30} /><h2>还没有离线内容</h2><p>完成的下载会自动出现在这里。</p></div>
      {:else if filteredEntries.length === 0}
        <div class="empty filter-empty"><Icon name="search" size={27} /><h2>没有符合条件的内容</h2><p>调整筛选条件，或点击“重置”查看全部 {entries.length} 项。</p><button type="button" onclick={resetLibraryFilters}>重置筛选</button></div>
      {:else}
        <p class="filter-summary">显示 {filteredEntries.length} / {entries.length} 项</p>
        <div class="entries">
          {#each filteredEntries as entry (entry.key)}
            {@const organization = organizationFor(entry.key)}
            <article class="entry-row">
              <a class="entry-main" href={entryHref(entry)}>
                <span class="kind">{kindLabel(entry.kind)}</span>
                <div class="entry-copy">
                  <h2>{entry.title || `作品 ${entry.resourceId}`}</h2>
                  <p>{entry.author || "未知作者"} · {entry.assetCount} 个文件 · {formatBytes(entry.sizeBytes)}</p>
                  {#if organization?.collectionId != null || organization?.tags.length}
                    <div class="organization-badges">
                      {#if organization.collectionId != null}<span class="collection-badge">{collectionName(organization.collectionId)}</span>{/if}
                      {#each organization.tags as tag (tag)}<span>#{tag}</span>{/each}
                    </div>
                  {/if}
                </div>
                <b>打开 ›</b>
              </a>
              <div class="entry-actions">
                <button type="button" disabled={catalogStatus !== "ready" || !!catalogAction || !!exporting || !!removing} onclick={() => organizingKey === entry.key ? closeOrganizationEditor() : openOrganizationEditor(entry)}>{organizingKey === entry.key ? "收起" : "整理"}</button>
                <button type="button" disabled={!exportDestination?.configured || !!exporting || !!removing} onclick={() => exportEntry(entry)}>
                  {exporting === entry.key ? "导出中…" : "导出"}
                </button>
                <button type="button" class:confirm={confirming === entry.key} disabled={!!exporting || removing === entry.key} onclick={() => requestRemoval(entry.key)}>{removing === entry.key ? "删除中…" : confirming === entry.key ? "确认删除" : "删除"}</button>
              </div>
              {#if organizingKey === entry.key}
                <form class="organize-editor" onsubmit={(event) => saveOrganization(event, entry)}>
                  <div>
                    <label for={`collection-${entry.key}`}>本地收藏夹</label>
                    <select id={`collection-${entry.key}`} bind:value={organizationCollectionId}><option value="">不放入收藏夹</option>{#each catalog.collections as collection (collection.id)}<option value={String(collection.id)}>{collection.name}</option>{/each}</select>
                  </div>
                  <div class="tag-editor">
                    <label for={`tags-${entry.key}`}>本地标签</label>
                    <input id={`tags-${entry.key}`} bind:value={organizationTags} maxlength="768" placeholder="用逗号分隔，最多 16 个" />
                    <small>标签仅保存在本机，不会修改 Pixiv 收藏标签。</small>
                  </div>
                  <div class="organize-actions"><button type="button" disabled={!!catalogAction} onclick={closeOrganizationEditor}>取消</button><button type="submit" disabled={!!catalogAction}>{catalogAction === `organize-${entry.key}` ? "保存中…" : "保存整理"}</button></div>
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
  .page-header h1 { margin: 0; font-size: 22px; }
  .page-header p { margin: 7px 0 0; color: var(--muted); font-size: 10px; }
  .stats { min-width: 120px; padding: 12px 16px; border: 1px solid var(--line); border-radius: 10px; background: white; text-align: right; }
  .stats strong, .stats span { display: block; }
  .stats strong { font-size: 18px; }
  .stats span { margin-top: 3px; color: var(--muted); font-size: 8px; }
  .content-section { overflow: hidden; margin-top: 22px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .section-heading { display: flex; min-height: 68px; gap: 14px; align-items: center; justify-content: space-between; padding: 14px 16px; border-bottom: 1px solid var(--line); }
  .section-heading > div { display: flex; min-width: 0; gap: 11px; align-items: center; }
  .section-heading h2, .section-heading p { margin: 0; }
  .section-heading h2 { font-size: 14px; }
  .section-heading p { margin-top: 4px; color: var(--muted); font-size: 8px; }
  .section-heading button, .compact-state button, .state button { padding: 8px 15px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 18px; background: #f5fbff; cursor: pointer; font-size: 8px; }
  .section-refresh { min-width: 68px; flex: 0 0 auto; white-space: nowrap; word-break: keep-all; }
  .section-heading button:disabled { cursor: default; opacity: .55; }
  .heading-icon { display: grid; width: 38px; height: 38px; flex: 0 0 auto; place-items: center; color: var(--pixiv-blue); border-radius: 50%; background: #eaf7ff; }
  .heading-icon.library { color: #4b8b6b; background: #edf9f2; }
  .task-list { display: grid; }
  .task-row { display: grid; grid-template-columns: 82px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 14px 16px; border-bottom: 1px solid var(--line); }
  .task-row:last-child { border-bottom: 0; }
  .task-kind { display: grid; height: 46px; place-items: center; color: #4e7b94; border-radius: 7px; background: #edf7fc; font-size: 8px; font-weight: 700; }
  .task-main { min-width: 0; }
  .task-title-line { display: flex; min-width: 0; gap: 8px; align-items: center; }
  .task-title-line h3 { overflow: hidden; margin: 0; font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
  .task-main > p { margin: 4px 0 0; color: var(--muted); font-size: 8px; }
  .state-badge { flex: 0 0 auto; padding: 3px 7px; color: #54788c; border-radius: 10px; background: #edf7fc; font-size: 7px; font-weight: 700; }
  .state-badge.failed { color: #a24e5c; background: #fff0f3; }
  .state-badge.done { color: #3b7a58; background: #eaf8f0; }
  .progress-line { display: flex; gap: 9px; align-items: center; margin-top: 8px; }
  .progress-track { overflow: hidden; height: 5px; flex: 1 1 auto; border-radius: 4px; background: #e9eef2; }
  .progress-track span { display: block; height: 100%; border-radius: inherit; background: var(--pixiv-blue); transition: width .18s ease; }
  .progress-line small { flex: 0 0 auto; color: var(--muted); font-size: 7px; }
  .task-main .failure-note { color: #a24e5c; }
  .task-actions { display: flex; gap: 7px; }
  .task-actions button, .task-actions a { min-width: 52px; padding: 8px 11px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 17px; background: white; cursor: pointer; font-size: 8px; text-align: center; text-decoration: none; }
  .task-actions button:last-child { color: #8b6570; border-color: #eadbe0; }
  .task-actions button.confirm { color: white; border-color: #b24d5e; background: #b24d5e; }
  .task-actions button:disabled { cursor: default; opacity: .48; }
  .compact-state, .queue-empty { display: grid; min-height: 92px; gap: 7px; place-items: center; color: var(--muted); text-align: center; }
  .compact-state p, .queue-empty p, .queue-empty span { margin: 0; font-size: 8px; }
  .queue-empty p { color: var(--text); font-size: 11px; font-weight: 700; }
  .catalog-tools { display: grid; grid-template-columns: minmax(190px, 1.7fr) repeat(4, minmax(108px, 1fr)) auto; gap: 9px; align-items: end; padding: 14px 16px; border-bottom: 1px solid var(--line); background: #fbfdff; }
  .catalog-tools label { display: grid; min-width: 0; gap: 5px; color: var(--muted); font-size: 7px; font-weight: 700; }
  .catalog-tools input, .catalog-tools select, .new-collection input, .collection-row input, .organize-editor input, .organize-editor select { width: 100%; height: 34px; min-width: 0; padding: 0 10px; color: var(--text); border: 1px solid #dce4ea; border-radius: 7px; outline: none; background: white; font: inherit; font-size: 9px; }
  .catalog-tools input:focus, .catalog-tools select:focus, .new-collection input:focus, .collection-row input:focus, .organize-editor input:focus, .organize-editor select:focus { border-color: var(--pixiv-blue); box-shadow: 0 0 0 2px rgba(0,150,250,.1); }
  .catalog-tools select:disabled { opacity: .55; }
  .catalog-tools > button, .filter-empty button { height: 34px; padding: 0 13px; color: #60727d; border: 1px solid #dce4ea; border-radius: 17px; background: white; cursor: pointer; font-size: 8px; }
  .collection-manager { border-bottom: 1px solid var(--line); background: #fff; }
  .collection-manager summary { display: flex; min-height: 43px; gap: 9px; align-items: center; padding: 10px 16px; cursor: pointer; list-style-position: inside; }
  .collection-manager summary span { font-size: 9px; font-weight: 750; }
  .collection-manager summary small { color: var(--muted); font-size: 7px; font-weight: 400; }
  .collection-manager-body { padding: 0 16px 14px; }
  .new-collection { display: grid; grid-template-columns: auto minmax(150px, 1fr) auto; gap: 9px; align-items: center; padding: 11px; border-radius: 8px; background: #f6f9fb; }
  .new-collection label { color: #586974; font-size: 8px; font-weight: 700; }
  .new-collection button, .collection-row button, .catalog-state button, .organize-actions button { height: 32px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #cde8f9; border-radius: 16px; background: white; cursor: pointer; font-size: 8px; }
  .new-collection button:disabled, .collection-row button:disabled, .organize-actions button:disabled { cursor: default; opacity: .5; }
  .catalog-state { margin: 11px 0 0; color: var(--muted); font-size: 8px; text-align: center; }
  .catalog-state.error { color: #a24e5c; }
  .catalog-state button { margin-left: 8px; }
  .collection-list { display: grid; margin-top: 8px; border: 1px solid var(--line); border-radius: 8px; }
  .collection-row { display: flex; min-height: 48px; gap: 12px; align-items: center; justify-content: space-between; padding: 8px 11px; border-bottom: 1px solid var(--line); }
  .collection-row:last-child { border-bottom: 0; }
  .collection-row > div:first-child strong, .collection-row > div:first-child span { display: block; }
  .collection-row strong { font-size: 9px; }
  .collection-row span { margin-top: 3px; color: var(--muted); font-size: 7px; }
  .collection-row > form { display: grid; width: 100%; grid-template-columns: minmax(0,1fr) auto auto; gap: 7px; }
  .collection-actions { display: flex; gap: 7px; }
  .collection-actions button:last-child { color: #a24e5c; border-color: #eadbe0; }
  .collection-actions button.confirm { color: white; border-color: #b24d5e; background: #b24d5e; }
  .filter-summary { margin: 0; padding: 8px 16px; color: var(--muted); border-bottom: 1px solid var(--line); background: #fbfdff; font-size: 7px; text-align: right; }
  .entries { display: grid; }
  .entry-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; border-bottom: 1px solid var(--line); }
  .entry-row:last-child { border-bottom: 0; }
  .entry-main { display: grid; grid-template-columns: 78px minmax(0, 1fr) auto; gap: 14px; align-items: center; padding: 16px; color: var(--text); text-decoration: none; }
  .entry-copy { min-width: 0; }
  .kind { display: grid; height: 46px; place-items: center; color: #4e7b94; border-radius: 7px; background: #edf7fc; font-size: 8px; font-weight: 700; }
  .entry-row h2 { overflow: hidden; margin: 0; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; }
  .entry-row p { margin: 5px 0 0; color: var(--muted); font-size: 8px; }
  .entry-main > b { color: var(--pixiv-blue); font-size: 9px; }
  .organization-badges { display: flex; overflow: hidden; gap: 5px; align-items: center; margin-top: 7px; }
  .organization-badges span { overflow: hidden; max-width: 120px; padding: 3px 6px; color: #526f82; border-radius: 10px; background: #edf6fb; font-size: 7px; text-overflow: ellipsis; white-space: nowrap; }
  .organization-badges span.collection-badge { color: #397457; background: #eaf8f0; font-weight: 700; }
  .entry-actions { display: grid; align-self: stretch; grid-template-columns: repeat(3, minmax(58px, auto)); border-left: 1px solid var(--line); }
  .entry-actions button { min-width: 62px; padding: 0 12px; color: var(--pixiv-blue); border: 0; background: #f8fcff; cursor: pointer; font-size: 8px; }
  .entry-actions button + button { border-left: 1px solid var(--line); }
  .entry-actions button:last-child { color: #a24e5c; background: #fffafb; }
  .entry-actions button.confirm { color: white; background: #b24d5e; }
  .entry-actions button:disabled { cursor: default; opacity: .48; }
  .organize-editor { display: grid; grid-column: 1 / -1; grid-template-columns: minmax(150px,.7fr) minmax(260px,1.6fr) auto; gap: 12px; align-items: end; padding: 14px 16px; border-top: 1px solid var(--line); background: #f8fbfd; }
  .organize-editor > div { display: grid; min-width: 0; gap: 5px; }
  .organize-editor label { color: #53656f; font-size: 8px; font-weight: 700; }
  .organize-editor small { color: var(--muted); font-size: 7px; }
  .organize-actions { display: flex !important; gap: 7px !important; }
  .organize-actions button:last-child { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .export-guidance, .export-notice { margin: 0; padding: 10px 16px; color: #7a6542; border-bottom: 1px solid #f0e5cf; background: #fffaf1; font-size: 8px; line-height: 1.55; }
  .export-guidance.ready { color: #397457; border-color: #d8ebdf; background: #f3fbf6; }
  .export-guidance a { margin: 0 3px; color: var(--pixiv-blue); font-weight: 700; }
  .export-notice { color: #397457; border-color: #d8ebdf; background: #f3fbf6; }
  .export-notice.error { color: #a24e5c; border-color: #f2dce2; background: #fff9fa; }
  .catalog-notice { margin: 0; padding: 10px 16px; color: #397457; border-bottom: 1px solid #d8ebdf; background: #f3fbf6; font-size: 8px; line-height: 1.55; }
  .catalog-notice.error { color: #a24e5c; border-color: #f2dce2; background: #fff9fa; }
  .state, .empty { display: grid; min-height: 180px; gap: 10px; place-items: center; color: var(--muted); text-align: center; }
  .empty h2, .empty p, .state p { margin: 0; }
  .empty h2 { color: var(--text); font-size: 15px; }
  .empty p, .state p { font-size: 9px; }
  .filter-empty button { margin-top: 2px; }
  .spinner { width: 28px; height: 28px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .compact-state .spinner { width: 20px; height: 20px; border-width: 2px; }
  .inline-error { margin: 0; padding: 9px 16px; color: #a24e5c; border-top: 1px solid #f2dce2; background: #fff9fa; font-size: 8px; text-align: center; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 620px) {
    .offline-page { padding: 18px 12px 90px; }
    .page-header { align-items: stretch; flex-direction: column; }
    .page-header h1 { font-size: 21px; }
    .page-header p { font-size: 12px; line-height: 1.6; }
    .stats { text-align: left; }
    .stats span { font-size: 11px; }
    .section-heading { align-items: flex-start; }
    .section-heading h2 { font-size: 16px; }
    .section-heading p { font-size: 11px; line-height: 1.5; }
    .section-heading button { margin-top: 3px; font-size: 11px; }
    .task-row { grid-template-columns: 58px minmax(0, 1fr); gap: 10px; padding: 14px 12px; }
    .task-kind { height: 42px; font-size: 10px; }
    .task-title-line h3 { font-size: 14px; }
    .state-badge { font-size: 9px; }
    .task-main > p, .task-main .failure-note { font-size: 11px; line-height: 1.45; }
    .progress-line { align-items: flex-start; flex-direction: column; }
    .progress-track { width: 100%; flex: none; }
    .progress-line small { font-size: 10px; }
    .task-actions { grid-column: 1 / -1; justify-content: flex-end; }
    .task-actions button, .task-actions a { min-width: 70px; font-size: 11px; }
    .compact-state p, .queue-empty span { font-size: 11px; }
    .queue-empty p { font-size: 14px; }
    .catalog-tools { grid-template-columns: 1fr 1fr; gap: 10px; padding: 12px; }
    .catalog-tools .library-search { grid-column: 1 / -1; }
    .catalog-tools label { font-size: 10px; }
    .catalog-tools input, .catalog-tools select { height: 40px; font-size: 12px; }
    .catalog-tools > button { height: 40px; align-self: end; font-size: 11px; }
    .collection-manager summary { align-items: flex-start; flex-direction: column; padding: 12px; }
    .collection-manager summary span { font-size: 12px; }
    .collection-manager summary small { font-size: 10px; }
    .collection-manager-body { padding: 0 12px 12px; }
    .new-collection { grid-template-columns: 1fr auto; }
    .new-collection label { grid-column: 1 / -1; font-size: 11px; }
    .new-collection input, .collection-row input { height: 40px; font-size: 12px; }
    .new-collection button, .collection-row button, .catalog-state button { height: 38px; font-size: 11px; }
    .catalog-state { font-size: 11px; line-height: 1.5; }
    .collection-row { min-height: 58px; padding: 9px; }
    .collection-row strong { font-size: 12px; }
    .collection-row span { font-size: 10px; }
    .filter-summary { padding: 9px 12px; font-size: 10px; }
    .entry-main { grid-template-columns: 62px minmax(0, 1fr); }
    .entry-main > b { display: none; }
    .entry-row h2 { font-size: 14px; }
    .entry-row p { font-size: 11px; line-height: 1.45; }
    .entry-row { grid-template-columns: 1fr; }
    .organization-badges { flex-wrap: wrap; }
    .organization-badges span { max-width: 150px; font-size: 9px; }
    .entry-actions { min-height: 48px; grid-template-columns: repeat(3,1fr); border-top: 1px solid var(--line); border-left: 0; }
    .entry-actions button { font-size: 11px; }
    .organize-editor { grid-template-columns: 1fr; padding: 14px 12px; }
    .organize-editor label { font-size: 11px; }
    .organize-editor input, .organize-editor select { height: 42px; font-size: 12px; }
    .organize-editor small { font-size: 10px; line-height: 1.45; }
    .organize-actions { justify-content: flex-end; }
    .organize-actions button { height: 40px; font-size: 11px; }
    .export-guidance, .export-notice { padding: 12px; font-size: 11px; }
    .catalog-notice { padding: 12px; font-size: 11px; }
    .inline-error { font-size: 11px; }
  }
</style>
