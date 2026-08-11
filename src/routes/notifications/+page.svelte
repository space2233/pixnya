<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import { classifyNotificationLink } from "$lib/notification-link";
  import {
    describeDataFailure,
    getNotifications,
    getNotificationViewMore,
    openPixivUrl,
  } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { PixivNotification } from "$lib/types";

  let notifications = $state<PixivNotification[]>([]);
  let nextCursor = $state<string | null>(null);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let loadingMore = $state(false);
  let groupLoading = $state<string | null>(null);
  let groupCursors = $state<Record<string, string | null | undefined>>({});
  let groupChildIds = $state<Record<string, string[]>>({});
  let sessionKey = $state("");
  let requestSequence = 0;

  $effect(() => {
    const key = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    if (key === sessionKey) return;
    sessionKey = key;
    requestSequence += 1;
    notifications = [];
    nextCursor = null;
    groupCursors = {};
    groupChildIds = {};
    loadingMore = false;
    groupLoading = null;
    errorMessage = "";
    status = "idle";
    if (key) void loadInitial(key);
  });

  async function loadInitial(expectedSession = sessionKey) {
    if (!expectedSession) return;
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    notifications = [];
    nextCursor = null;
    groupCursors = {};
    groupChildIds = {};
    try {
      const page = await getNotifications();
      if (sequence !== requestSequence || expectedSession !== sessionKey) return;
      notifications = page.notifications;
      nextCursor = page.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || expectedSession !== sessionKey) return;
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMore() {
    if (!nextCursor || loadingMore) return;
    const expectedSession = sessionKey;
    const sequence = requestSequence;
    const cursor = nextCursor;
    loadingMore = true;
    errorMessage = "";
    try {
      const page = await getNotifications(cursor);
      if (sequence !== requestSequence || expectedSession !== sessionKey) return;
      const known = new Set(notifications.map((item) => item.id));
      notifications = [...notifications, ...page.notifications.filter((item) => !known.has(item.id))];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      if (sequence === requestSequence && expectedSession === sessionKey) errorMessage = describeDataFailure(error);
    } finally {
      if (sequence === requestSequence && expectedSession === sessionKey) loadingMore = false;
    }
  }

  async function expandGroup(item: PixivNotification) {
    const cursor = groupCursors[item.id];
    if (groupLoading || cursor === null) return;
    groupLoading = item.id;
    errorMessage = "";
    const expectedSession = sessionKey;
    const sequence = requestSequence;
    try {
      const page = await getNotificationViewMore(item.id, cursor ?? undefined);
      if (sequence !== requestSequence || expectedSession !== sessionKey) return;
      const existingChildIds = groupChildIds[item.id] ?? [];
      const known = new Set(notifications.map((candidate) => candidate.id));
      const additions = page.notifications.filter((candidate) => !known.has(candidate.id));
      const lastChildId = existingChildIds[existingChildIds.length - 1];
      const insertAfterId = lastChildId ?? item.id;
      const index = notifications.findIndex((candidate) => candidate.id === insertAfterId);
      if (index >= 0 && additions.length > 0) notifications = [
        ...notifications.slice(0, index + 1),
        ...additions,
        ...notifications.slice(index + 1),
      ];
      groupChildIds = {
        ...groupChildIds,
        [item.id]: [...existingChildIds, ...additions.map((candidate) => candidate.id)],
      };
      groupCursors = { ...groupCursors, [item.id]: page.nextCursor ?? null };
    } catch (error) {
      if (sequence === requestSequence && expectedSession === sessionKey) errorMessage = describeDataFailure(error);
    } finally {
      if (sequence === requestSequence && expectedSession === sessionKey) groupLoading = null;
    }
  }

  async function openTarget(item: PixivNotification) {
    const link = classifyNotificationLink(item.targetUrl);
    if (link?.kind !== "external") return;
    try {
      await openPixivUrl(link.href);
    } catch (error) {
      errorMessage = describeDataFailure(error);
    }
  }

  function displayDate(value: string): string {
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : date.toLocaleString(currentAppLocale(), { dateStyle: "medium", timeStyle: "short" });
  }

  function mediaUrl(item: PixivNotification): string | null {
    return item.content.leftImage ?? item.content.rightImage ?? item.content.leftIcon ?? item.content.rightIcon ?? null;
  }
</script>

<svelte:head><title>{m.notifications_title()} · PixNya</title></svelte:head>

<AppShell title={m.notifications_title()}>
  <main class="notification-page">
    <header>
      <div><small>{m.notifications_read_only()}</small><h1>{m.notifications_title()}</h1><p>{m.notifications_read_only_description()}</p></div>
      <button type="button" disabled={status === "loading"} onclick={() => loadInitial()}>{m.common_refresh()}</button>
    </header>

    {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state"><Icon name="user" size={28} /><div><h2>{m.notifications_login_title()}</h2><p>{m.notifications_login_description()}</p></div><a href="/login?mode=standard">{m.common_go_to_login()}</a></section>
    {:else if status === "loading"}
      <section class="state"><span class="spinner"></span><p>{m.notifications_loading()}</p></section>
    {:else if status === "error" && notifications.length === 0}
      <section class="state error" role="alert"><Icon name="bell" size={28} /><div><h2>{m.notifications_load_failed()}</h2><p>{errorMessage}</p></div><button type="button" onclick={() => loadInitial()}>{m.common_retry()}</button></section>
    {:else if notifications.length === 0}
      <section class="state"><Icon name="bell" size={28} /><div><h2>{m.notifications_empty()}</h2><p>{m.notifications_empty_description()}</p></div></section>
    {:else}
      <section class="notification-list" aria-live="polite">
        {#each notifications as item (item.id)}
          {@const link = classifyNotificationLink(item.targetUrl)}
          <article class:unread={!item.isRead}>
            <span class="media"><PixivImage url={mediaUrl(item)} alt="" fit="cover" /></span>
            <div class="body"><p>{item.content.text}</p><time>{displayDate(item.createdDatetime)}</time>
              <div class="actions">
                {#if link?.kind === "internal"}<a href={link.href}>{m.notifications_open()}</a>
                {:else if link?.kind === "external"}<button type="button" onclick={() => openTarget(item)}>{m.notifications_open_browser()}</button>{/if}
                {#if item.viewMore && groupCursors[item.id] !== null}<button type="button" disabled={groupLoading === item.id} onclick={() => expandGroup(item)}>{groupLoading === item.id ? m.common_loading() : (groupChildIds[item.id]?.length ? m.notifications_load_more() : item.viewMore.title || m.notifications_expand())}</button>{/if}
              </div>
            </div>
          </article>
        {/each}
      </section>
      {#if errorMessage}<p class="inline-error" role="alert">{errorMessage}</p>{/if}
      {#if nextCursor}<button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? m.common_loading() : m.notifications_load_more()}</button>{/if}
    {/if}
  </main>
</AppShell>

<style>
  .notification-page { box-sizing: border-box; width: min(840px,100%); margin: 0 auto; padding: 28px 24px 100px; }
  header { display: flex; gap: 20px; align-items: end; justify-content: space-between; margin-bottom: 20px; }
  header small { color: var(--pixiv-blue); font-size: 8px; font-weight: 800; }
  h1 { margin: 6px 0 0; font-size: 24px; } header p { margin: 7px 0 0; color: var(--muted); font-size: 9px; }
  header button, .load-more, .state button { height: 36px; padding: 0 16px; border: 1px solid var(--line); border-radius: 18px; background: white; cursor: pointer; }
  .notification-list { overflow: hidden; border: 1px solid var(--line); border-radius: 13px; background: white; }
  article { display: grid; grid-template-columns: 52px minmax(0,1fr); gap: 13px; padding: 16px; }
  article + article { border-top: 1px solid var(--line); } article.unread { background: #f5fbff; }
  .media { display: grid; overflow: hidden; width: 52px; height: 52px; place-items: center; color: #9eb4c0; border-radius: 9px; background: #edf3f6; }
  .body p { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 10px; line-height: 1.65; }
  time { display: block; margin-top: 5px; color: var(--soft-muted); font-size: 8px; }
  .actions { display: flex; gap: 9px; flex-wrap: wrap; margin-top: 10px; }
  .actions a, .actions button { color: var(--pixiv-blue); border: 0; background: transparent; cursor: pointer; font-size: 8px; text-decoration: none; }
  .state { display: flex; gap: 14px; align-items: center; min-height: 100px; padding: 20px; color: var(--muted); border: 1px solid var(--line); border-radius: 12px; background: white; }
  .state h2, .state p { margin: 0; } .state h2 { color: var(--text); font-size: 15px; } .state p { margin-top: 5px; font-size: 9px; }
  .state a { margin-left: auto; padding: 10px 15px; color: white; border-radius: 18px; background: var(--pixiv-blue); font-size: 9px; text-decoration: none; }
  .state.error { color: #ad5360; } .state button { margin-left: auto; }
  .spinner { width: 26px; height: 26px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .inline-error { color: #ad5360; font-size: 9px; text-align: center; }
  .load-more { display: block; min-width: 140px; margin: 18px auto 0; }
  button:disabled { cursor: wait; opacity: .58; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 620px) { .notification-page { padding: 18px 12px 92px; } header { align-items: start; } article { grid-template-columns: 44px minmax(0,1fr); padding: 13px; } .media { width: 44px; height: 44px; } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
