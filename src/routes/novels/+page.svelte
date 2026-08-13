<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import ContentTabs from "$lib/components/ContentTabs.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import { m } from "$lib/i18n";
  import { buildBookmarkBatchUpdate, type BookmarkBatchAction } from "$lib/bookmark-batch";
  import { loadAllBookmarkTags } from "$lib/bookmark-tags";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { batchUpdateBookmarks, describeDataFailure, getBookmarkDetail, getBookmarkTags, getBookmarkedNovels, getFollowedNovels, getRankingNovels, getRecommendedNovels } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { BookmarkRestrict, BookmarkTag, NovelPage, NovelSummary, RankingMode } from "$lib/types";

  const sections = ["recommended", "following", "ranking", "bookmarks"] as const;
  type NovelSection = (typeof sections)[number];
  const sectionLabels: Record<NovelSection, () => string> = {
    recommended: m.novels_section_recommended,
    following: m.novels_section_following,
    ranking: m.novels_section_ranking,
    bookmarks: m.novels_section_bookmarks,
  };

  let novels = $state<NovelSummary[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let requestedSession = $state("");
  let selectedSection = $state<NovelSection>("recommended");
  let bookmarkRestrict = $state<BookmarkRestrict>("public");
  let rankingMode = $state<RankingMode>("day");
  let requestSequence = 0;
  let bookmarkTags = $state<BookmarkTag[]>([]);
  let bookmarkTagsRevision = $state(0);
  let selectedBookmarkTag = $state("");
  let selectionMode = $state(false);
  let selectedNovelIds = $state<string[]>([]);
  let batchTag = $state("");
  let batchBusy = $state(false);
  let batchStatus = $state("");

  type NovelListSnapshot = {
    novels: NovelSummary[];
    nextCursor: string | null;
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    requestedSession: string;
    selectedSection: NovelSection;
    bookmarkRestrict: BookmarkRestrict;
    rankingMode: RankingMode;
    selectedBookmarkTag: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<NovelListSnapshot>({
      novels,
      nextCursor,
      status,
      errorMessage,
      requestedSession,
      selectedSection,
      bookmarkRestrict,
      rankingMode,
      selectedBookmarkTag,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<NovelListSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      novels = value.novels;
      nextCursor = value.nextCursor;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      requestedSession = value.status === "loading" ? "" : value.requestedSession;
      selectedSection = sections.includes(value.selectedSection) ? value.selectedSection : "recommended";
      bookmarkRestrict = value.bookmarkRestrict;
      rankingMode = value.rankingMode;
      selectedBookmarkTag = value.selectedBookmarkTag ?? "";
      loadingMore = false;
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey ? `${sessionKey}:${selectedSection}:${bookmarkRestrict}:${rankingMode}:${selectedSection === "bookmarks" ? selectedBookmarkTag : ""}` : "";
    if (!key) {
      requestSequence += 1;
      requestedSession = "";
      novels = [];
      nextCursor = null;
      status = "idle";
      return;
    }
    if (key !== requestedSession) {
      requestedSession = key;
      void loadNovels(key);
    }
  });

  $effect(() => {
    bookmarkTagsRevision;
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    if (selectedSection !== "bookmarks" || !sessionKey) {
      bookmarkTags = [];
      selectedBookmarkTag = "";
      return;
    }
    let active = true;
    loadAllBookmarkTags((cursor) => getBookmarkTags("novel", bookmarkRestrict, cursor))
      .then((tags) => { if (active) bookmarkTags = tags; })
      .catch(() => { if (active) bookmarkTags = []; });
    return () => { active = false; };
  });

  function requestPage(cursor?: string): Promise<NovelPage> {
    if (selectedSection === "following") return getFollowedNovels(cursor);
    if (selectedSection === "ranking") return getRankingNovels(rankingMode, cursor);
    if (selectedSection === "bookmarks") return getBookmarkedNovels(bookmarkRestrict, cursor, selectedBookmarkTag || undefined);
    return getRecommendedNovels(cursor);
  }

  function toggleNovelSelection(id: string, selected: boolean) {
    const values = new Set(selectedNovelIds);
    if (selected) values.add(id); else values.delete(id);
    selectedNovelIds = [...values].slice(0, 100);
  }

  function changeBookmarkRestrict(event: Event) {
    const value = (event.currentTarget as HTMLSelectElement).value as BookmarkRestrict;
    if (value === bookmarkRestrict) return;
    bookmarkRestrict = value;
    selectedBookmarkTag = "";
    selectedNovelIds = [];
    batchStatus = "";
  }

  async function applyNovelBatch(action: BookmarkBatchAction) {
    if (batchBusy || selectedNovelIds.length === 0) return;
    if (action === "remove" && !window.confirm(m.bookmark_remove_confirm({ count: selectedNovelIds.length }))) return;
    batchBusy = true;
    batchStatus = "";
    try {
      const expectedUserId = $session.user?.id;
      if (!expectedUserId) throw { kind: "authentication_required" };
      const updates = [];
      for (const resourceId of selectedNovelIds) {
        const detail = await getBookmarkDetail("novel", resourceId);
        updates.push(buildBookmarkBatchUpdate("novel", resourceId, detail, action, batchTag));
      }
      const results = await batchUpdateBookmarks(updates, expectedUserId);
      const succeeded = new Set(results.filter((item) => item.succeeded).map((item) => item.resourceId));
      if (action === "remove" || ((action === "public" || action === "private") && action !== bookmarkRestrict)) {
        novels = novels.filter((item) => !succeeded.has(item.id));
      }
      selectedNovelIds = selectedNovelIds.filter((id) => !succeeded.has(id));
      const failed = results.length - succeeded.size;
      batchStatus = failed ? m.bookmark_batch_partial({ success: succeeded.size, failed }) : m.bookmark_batch_success({ count: succeeded.size });
      if (succeeded.size > 0) bookmarkTagsRevision += 1;
      if (action === "add_tag" || action === "remove_tag") {
        selectedNovelIds = [];
        requestedSession = "";
      }
    } catch (error) {
      batchStatus = describeDataFailure(error);
    } finally {
      batchBusy = false;
    }
  }

  async function loadNovels(key: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    try {
      const page = await requestPage();
      if (sequence !== requestSequence || key !== requestedSession) return;
      novels = page.novels;
      nextCursor = page.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || key !== requestedSession) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    loadingMore = true;
    errorMessage = "";
    try {
      const page = await requestPage(cursor);
      const known = new Set(novels.map((novel) => novel.id));
      novels = [...novels, ...page.novels.filter((novel) => !known.has(novel.id))];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      errorMessage = describeDataFailure(error);
    } finally {
      loadingMore = false;
    }
  }
</script>

<svelte:head><title>{m.novels_title()} · PixNya</title></svelte:head>

<AppShell title={m.novels_title()}>
  <ContentTabs />
  <main class="novel-page">
    <header><div><h1>{m.novels_title()}</h1></div></header>
    <div class="novel-toolbar">
      <nav aria-label={m.novels_content_type()}>{#each sections as section}<button type="button" class:active={selectedSection === section} onclick={() => (selectedSection = section)}>{sectionLabels[section]()}</button>{/each}</nav>
      {#if selectedSection === "ranking"}<select bind:value={rankingMode} aria-label={m.novels_ranking_period()}><option value="day">{m.novels_today()}</option><option value="week">{m.novels_this_week()}</option><option value="month">{m.novels_this_month()}</option></select>{/if}
      {#if selectedSection === "bookmarks"}<div class="bookmark-controls"><select value={bookmarkRestrict} onchange={changeBookmarkRestrict} aria-label={m.novels_bookmark_scope()}><option value="public">{m.common_public_bookmarks()}</option><option value="private">{m.common_private_bookmarks()}</option></select><select bind:value={selectedBookmarkTag} aria-label={m.bookmark_filter_tag()}><option value="">{m.bookmark_all_tags()}</option>{#each bookmarkTags as tag}<option value={tag.name}>{tag.name} ({tag.count})</option>{/each}</select><button type="button" onclick={() => { selectionMode = !selectionMode; selectedNovelIds = []; batchStatus = ""; }}>{selectionMode ? m.common_cancel() : m.bookmark_manage()}</button></div>{/if}
    </div>
    {#if selectedSection === "bookmarks" && selectionMode}
      <div class="batch-toolbar"><button onclick={() => (selectedNovelIds = novels.slice(0,100).map((item) => item.id))}>{m.bookmark_select_visible()}</button><span>{m.bookmark_selected_count({ count: selectedNovelIds.length })}</span>{#if selectedNovelIds.length}<button disabled={batchBusy} onclick={() => applyNovelBatch("public")}>{m.filter_public()}</button><button disabled={batchBusy} onclick={() => applyNovelBatch("private")}>{m.filter_private()}</button><input bind:value={batchTag} maxlength="100" placeholder={m.bookmark_tag_name()} /><button disabled={batchBusy || !batchTag.trim()} onclick={() => applyNovelBatch("add_tag")}>{m.bookmark_add_tag()}</button><button disabled={batchBusy || !batchTag.trim()} onclick={() => applyNovelBatch("remove_tag")}>{m.bookmark_remove_tag()}</button><button class="danger" disabled={batchBusy} onclick={() => applyNovelBatch("remove")}>{m.bookmark_remove_selected()}</button>{/if}</div>
      {#if batchStatus}<p class="batch-status" role="status">{batchStatus}</p>{/if}
    {/if}
    {#if !$sessionRestoring && !$session.loggedIn}
<section class="state"><Icon name="user" size={27} /><div><h2>{m.novels_sign_in_title()}</h2><p>{m.novels_sign_in_description()}</p></div><a href="/login">{m.search_go_to_login()}</a></section>
    {:else if status === "loading"}
      <section class="state"><span class="spinner"></span><div><h2>{m.novels_loading_title({ section: sectionLabels[selectedSection]() })}</h2><p>{m.novels_loading_description()}</p></div></section>
    {:else if status === "error"}
      <section class="state error" role="alert"><span>!</span><div><h2>{m.novels_load_failed()}</h2><p>{errorMessage}</p></div><button type="button" onclick={() => loadNovels(requestedSession)}>{m.common_retry()}</button></section>
    {:else if status === "ready"}
      {#if novels.length}<div class="novel-grid">{#each novels as novel (novel.id)}<NovelCard {novel} selectable={selectedSection === "bookmarks" && selectionMode} selected={selectedNovelIds.includes(novel.id)} onSelect={(selected) => toggleNovelSelection(novel.id, selected)} />{/each}</div>{:else}<p class="empty">{m.novels_empty()}</p>{/if}
      {#if errorMessage}<p class="paging-error" role="alert">{errorMessage}</p>{/if}
      {#if nextCursor}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? m.common_loading() : m.novels_load_more()}</button>{/if}
    {/if}
  </main>
</AppShell>

<style>
  .novel-page { width: min(1060px,100%); margin: 0 auto; padding: 26px 28px 64px; }
  header h1 { margin: 0; font-size: 22px; }
  .novel-toolbar { display: flex; gap: 10px; align-items: center; justify-content: space-between; margin-top: 18px; } .novel-toolbar nav { display: flex; gap: 4px; padding: 4px; border-radius: 21px; background: #f3f3f3; } .novel-toolbar button { min-width: 58px; height: 31px; color: #777; border: 0; border-radius: 16px; background: transparent; cursor: pointer; font-size: 9px; font-weight: 700; } .novel-toolbar button.active { color: #333; background: white; box-shadow: 0 1px 4px rgba(0,0,0,.08); } .novel-toolbar select { height: 34px; padding: 0 11px; border: 1px solid var(--line); border-radius: 17px; background: white; font-size: 9px; }
  .bookmark-controls,.batch-toolbar{display:flex;flex-wrap:wrap;align-items:center;gap:8px}.bookmark-controls button,.batch-toolbar button{padding:8px 12px;border:1px solid #cde7f8;border-radius:16px;background:white;color:var(--pixiv-blue);cursor:pointer}.batch-toolbar{margin-top:14px;padding:12px 14px;border:1px solid var(--line);border-radius:12px;background:white}.batch-toolbar input{padding:8px;border:1px solid var(--line);border-radius:9px}.batch-toolbar .danger{color:var(--danger)}.batch-status{color:var(--muted);font-size:10px}
  .novel-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 16px; margin-top: 22px; }
  .state { display: grid; grid-template-columns: 44px minmax(0,1fr) auto; gap: 14px; align-items: center; margin-top: 22px; padding: 20px; border: 1px solid var(--line); border-radius: 11px; background: white; }
  .state h2 { margin: 0; font-size: 15px; } .state p { margin: 5px 0 0; color: var(--muted); font-size: 9px; }
  .state a, .state button, .load-more { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; text-decoration: none; }
  .state.error > span { display: grid; width: 36px; height: 36px; place-items: center; color: #a34e5d; border-radius: 50%; background: #fff0f3; }
  .spinner { width: 29px; height: 29px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .empty { margin-top: 22px; padding: 40px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; text-align: center; }
  .paging-error { color: #a34e5d; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 145px; margin: 24px auto 0; color: #59636a; border: 1px solid var(--line); background: white; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 760px) { .novel-grid { grid-template-columns: 1fr; } }
  @media (max-width: 620px) { .novel-page { padding: 18px 14px 90px; } .novel-toolbar { align-items: stretch; flex-direction: column; } .novel-toolbar nav { width: 100%; } .novel-toolbar nav button { min-width: 0; flex: 1; } .novel-toolbar select { align-self: flex-end; } .state { grid-template-columns: 38px minmax(0,1fr); } .state a, .state button { grid-column: 1 / -1; text-align: center; } }
</style>
