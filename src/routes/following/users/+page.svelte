<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import FollowingTabs from "$lib/components/FollowingTabs.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import UserPreviewCard from "$lib/components/UserPreviewCard.svelte";
  import { m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import { describeDataFailure, getFollowedUsers } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { BookmarkRestrict, UserPreview } from "$lib/types";

  let restrict = $state<BookmarkRestrict>("public");
  let users = $state<UserPreview[]>([]);
  let status = $state<"idle" | "loading" | "ready" | "error">("idle");
  let errorMessage = $state("");
  let nextCursor = $state<string | null>(null);
  let loadingMore = $state(false);
  let paginationError = $state("");
  let requestedKey = $state("");
  let requestSequence = 0;

  type FollowedUsersSnapshot = {
    restrict: BookmarkRestrict;
    users: UserPreview[];
    status: "idle" | "loading" | "ready" | "error";
    errorMessage: string;
    nextCursor: string | null;
    paginationError: string;
    requestedKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<FollowedUsersSnapshot>({
      restrict,
      users,
      status,
      errorMessage,
      nextCursor,
      paginationError,
      requestedKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<FollowedUsersSnapshot>(key);
      if (!value) return;
      requestSequence += 1;
      restrict = value.restrict;
      users = value.users;
      status = value.status === "loading" ? "idle" : value.status;
      errorMessage = value.errorMessage;
      nextCursor = value.nextCursor;
      paginationError = value.paginationError;
      requestedKey = value.status === "loading" ? "" : value.requestedKey;
      loadingMore = false;
    },
  };

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey ? `${sessionKey}:${restrict}` : "";
    if (!key) {
      requestSequence += 1;
      requestedKey = "";
      users = [];
      nextCursor = null;
      loadingMore = false;
      paginationError = "";
      status = "idle";
      errorMessage = "";
      return;
    }
    if (key !== requestedKey) {
      requestedKey = key;
      void loadUsers(key);
    }
  });

  async function loadUsers(key: string) {
    const sequence = ++requestSequence;
    status = "loading";
    errorMessage = "";
    nextCursor = null;
    paginationError = "";
    try {
      const page = await getFollowedUsers(restrict);
      if (sequence !== requestSequence || requestedKey !== key) return;
      users = page.users;
      nextCursor = page.nextCursor ?? null;
      status = "ready";
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      users = [];
      errorMessage = describeDataFailure(error);
      status = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore) return;
    const sequence = ++requestSequence;
    const key = requestedKey;
    loadingMore = true;
    paginationError = "";
    try {
      const page = await getFollowedUsers(restrict, cursor);
      if (sequence !== requestSequence || requestedKey !== key) return;
      const knownIds = new Set(users.map((preview) => preview.user.id));
      users = [...users, ...page.users.filter((preview) => !knownIds.has(preview.user.id))];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      if (sequence !== requestSequence || requestedKey !== key) return;
      paginationError = describeDataFailure(error);
    } finally {
      if (sequence === requestSequence && requestedKey === key) loadingMore = false;
    }
  }
</script>

<svelte:head><title>{m.following_users_title()} · PixNya</title></svelte:head>

<AppShell title={m.following_users_title()}>
  <main class="following-users-page">
    <FollowingTabs />
    <header class="page-heading">
      <div><h1 class="page-title">{m.following_users_title()}</h1></div>
      <nav aria-label={m.following_scope()}>
        <button type="button" class:active={restrict === "public"} aria-pressed={restrict === "public"} onclick={() => (restrict = "public")}>{m.following_public()}</button>
        <button type="button" class:active={restrict === "private"} aria-pressed={restrict === "private"} onclick={() => (restrict = "private")}>{m.following_private()}</button>
      </nav>
    </header>

    {#if $sessionRestoring}
      <section class="state-card"><span class="spinner"></span><div><h2>{m.following_restoring()}</h2><p>{m.following_restoring_description()}</p></div></section>
    {:else if !$session.loggedIn}
      <section class="state-card">
        <Icon name="user" size={25} />
        <div><h2>{m.following_sign_in_title()}</h2><p>{m.following_sign_in_description()}</p></div>
<a href="/login">{m.search_go_to_login()}</a>
      </section>
    {:else if status === "loading"}
      <section class="state-card"><span class="spinner"></span><div><h2>{m.following_loading()}</h2><p>{m.following_loading_description()}</p></div></section>
    {:else if status === "error"}
      <section class="state-card error" role="alert">
        <span>!</span><div><h2>{m.following_load_failed()}</h2><p>{errorMessage}</p></div>
        <button type="button" onclick={() => loadUsers(requestedKey)}>{m.common_retry()}</button>
      </section>
    {:else if users.length > 0}
      <section class="user-grid" aria-live="polite">
        {#each users as preview (preview.user.id)}<UserPreviewCard {preview} />{/each}
      </section>
      {#if paginationError}<p class="pagination-error" role="alert">{paginationError}</p>{/if}
      {#if nextCursor}
        <button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>{loadingMore ? m.common_loading() : m.following_load_more()}</button>
      {/if}
    {:else if status === "ready"}
      <section class="empty-state"><Icon name="user" size={29} /><h2>{restrict === "private" ? m.following_empty_private() : m.following_empty_public()}</h2><p>{m.following_empty_hint()}</p></section>
    {/if}
  </main>
</AppShell>

<style>
  .following-users-page { width: min(1120px, 100%); margin: 0 auto; padding: 0 28px 70px; }
  .page-heading { display: flex; gap: 24px; align-items: end; justify-content: space-between; padding: 27px 0 22px; }
  .page-heading h1 { margin: 5px 0 0; font-size: var(--type-title); }
  .page-heading nav { display: flex; gap: 5px; padding: 4px; border-radius: 20px; background: #eef1f3; }
  .page-heading button { padding: 8px 14px; color: #6d767c; border: 0; border-radius: 16px; background: transparent; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .page-heading button.active { color: white; background: var(--pixiv-blue); }
  .user-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 16px; }
  .state-card { display: grid; grid-template-columns: 42px minmax(0,1fr) auto; min-height: 112px; gap: 14px; align-items: center; padding: 21px; border: 1px solid var(--line); border-radius: 11px; background: white; }
  .state-card h2, .empty-state h2 { margin: 0; font-size: var(--type-body); }
  .state-card p, .empty-state p { margin: 5px 0 0; color: var(--muted); font-size: var(--type-caption); }
  .state-card a, .state-card button, .load-more { padding: 9px 16px; color: white; border: 0; border-radius: 18px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; text-decoration: none; }
  .state-card.error > span { display: grid; width: 34px; height: 34px; place-items: center; color: #a34e5d; border-radius: 50%; background: #fff0f3; }
  .spinner { width: 28px; height: 28px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }
  .empty-state { display: grid; min-height: 230px; gap: 8px; place-items: center; align-content: center; color: var(--muted); border: 1px dashed var(--line); border-radius: 11px; text-align: center; }
  .pagination-error { margin: 16px 0 0; color: #a34e5d; font-size: var(--type-caption); text-align: center; }
  .load-more { display: block; margin: 22px auto 0; }
  .load-more:disabled { cursor: wait; opacity: .65; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 900px) { .user-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 620px) {
    .following-users-page { padding: 0 12px 92px; }
    .page-heading { display: grid; gap: 15px; padding: 20px 2px 17px; }
    .page-heading nav { width: 100%; }
    .page-heading nav button { flex: 1; font-size: var(--type-body); }
    .user-grid { grid-template-columns: 1fr; gap: 11px; }
    .state-card { grid-template-columns: 38px minmax(0,1fr); padding: 17px; }
    .state-card > a, .state-card > button { grid-column: 1 / -1; text-align: center; }
  }
</style>
