<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import {
    describeDataFailure,
    getAccessBlockedUsers,
    getMuteSettings,
    setAccessBlock,
    setTagMute,
    setUserMute,
  } from "$lib/pixiv-api";
  import { session, sessionRestoring } from "$lib/session";
  import type { IllustrationAuthor, MuteSettings } from "$lib/types";

  let blockedUsers = $state<IllustrationAuthor[]>([]);
  let nextCursor = $state<string | null>(null);
  let muteSettings = $state<MuteSettings | null>(null);
  let blockedUserId = $state("");
  let mutedUserId = $state("");
  let mutedTag = $state("");
  let loading = $state(false);
  let loadingMore = $state(false);
  let pendingMutation = $state(false);
  let errorMessage = $state("");
  let sessionKey = $state("");

  $effect(() => {
    const nextKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    if (nextKey === sessionKey) return;
    sessionKey = nextKey;
    blockedUsers = [];
    nextCursor = null;
    muteSettings = null;
    errorMessage = "";
    loading = false;
    loadingMore = false;
    pendingMutation = false;
    if (nextKey) void loadAll();
  });

  async function loadAll() {
    loading = true;
    errorMessage = "";
    const requestedSession = sessionKey;
    try {
      const [blocked, muted] = await Promise.all([getAccessBlockedUsers(), getMuteSettings()]);
      if (requestedSession !== sessionKey) return;
      blockedUsers = blocked.users;
      nextCursor = blocked.nextCursor ?? null;
      muteSettings = muted;
    } catch (error) {
      if (requestedSession === sessionKey) errorMessage = describeDataFailure(error);
    } finally {
      if (requestedSession === sessionKey) loading = false;
    }
  }

  async function loadMoreBlocked() {
    if (!nextCursor || loadingMore) return;
    const requestedSession = sessionKey;
    loadingMore = true;
    errorMessage = "";
    try {
      const page = await getAccessBlockedUsers(nextCursor);
      if (requestedSession !== sessionKey) return;
      const known = new Set(blockedUsers.map((user) => user.id));
      blockedUsers = [...blockedUsers, ...page.users.filter((user) => !known.has(user.id))];
      nextCursor = page.nextCursor ?? null;
    } catch (error) {
      if (requestedSession === sessionKey) errorMessage = describeDataFailure(error);
    } finally {
      if (requestedSession === sessionKey) loadingMore = false;
    }
  }

  async function mutate(request: () => Promise<void>, refresh: (expectedSession: string) => Promise<void>) {
    if (pendingMutation || !confirm(m.account_controls_confirm())) return;
    pendingMutation = true;
    errorMessage = "";
    const requestedSession = sessionKey;
    try {
      await request();
      if (requestedSession === sessionKey) await refresh(requestedSession);
    } catch (error) {
      if (requestedSession === sessionKey) errorMessage = describeDataFailure(error);
    } finally {
      if (requestedSession === sessionKey) pendingMutation = false;
    }
  }

  async function reloadBlocked(expectedSession = sessionKey) {
    const page = await getAccessBlockedUsers();
    if (expectedSession !== sessionKey) return;
    blockedUsers = page.users;
    nextCursor = page.nextCursor ?? null;
  }

  async function reloadMutes(expectedSession = sessionKey) {
    const result = await getMuteSettings();
    if (expectedSession === sessionKey) muteSettings = result;
  }

  function addBlock() {
    const userId = blockedUserId.trim();
    if (!/^\d+$/.test(userId) || userId === "0") return;
    void mutate(() => setAccessBlock(userId, true), async (expectedSession) => {
      if (expectedSession === sessionKey) blockedUserId = "";
      await reloadBlocked(expectedSession);
    });
  }

  function addUserMute() {
    const userId = mutedUserId.trim();
    if (!/^\d+$/.test(userId) || userId === "0") return;
    void mutate(() => setUserMute(userId, true), async (expectedSession) => {
      if (expectedSession === sessionKey) mutedUserId = "";
      await reloadMutes(expectedSession);
    });
  }

  function addTagMute() {
    const tag = mutedTag.trim();
    if (!tag) return;
    void mutate(() => setTagMute(tag, true), async (expectedSession) => {
      if (expectedSession === sessionKey) mutedTag = "";
      await reloadMutes(expectedSession);
    });
  }
</script>

<AppShell title={m.account_controls_title()}>
  <main class="account-controls">
    <ReturnLink fallback="/settings" label={m.account_controls_back()} />
    <header class="page-heading">
      <h1 class="page-title">{m.account_controls_title()}</h1>
    </header>

    {#if $sessionRestoring || loading}
      <p class="state">{m.account_controls_loading()}</p>
    {:else if !$session.loggedIn}
      <section class="card state"><p>{m.account_controls_login_required()}</p><a href="/login">{m.account_controls_login()}</a></section>
    {:else}
      {#if errorMessage}<p class="error" role="alert">{errorMessage}</p>{/if}

      <section class="card">
        <h2>{m.account_controls_block_title()}</h2>
        <form class="add-row" onsubmit={(event) => { event.preventDefault(); addBlock(); }}>
          <input bind:value={blockedUserId} inputmode="numeric" aria-label={m.account_controls_user_id()} placeholder={m.account_controls_user_id()} />
          <button disabled={pendingMutation}>{m.account_controls_add_block()}</button>
        </form>
        <div class="items">
          {#each blockedUsers as user (user.id)}
            <article class="user-row">
              {#if user.avatarUrl}<PixivImage url={user.avatarUrl} alt="" />{/if}
              <a href={`/users/${user.id}`}><strong>{user.name}</strong><small>@{user.account} · {user.id}</small></a>
              <button class="secondary" disabled={pendingMutation} onclick={() => void mutate(() => setAccessBlock(user.id, false), reloadBlocked)}>{m.account_controls_remove()}</button>
            </article>
          {:else}<p class="empty">{m.account_controls_empty()}</p>{/each}
        </div>
        {#if nextCursor}<button class="wide secondary" disabled={loadingMore} onclick={loadMoreBlocked}>{loadingMore ? m.account_controls_loading() : m.account_controls_load_more()}</button>{/if}
      </section>

      <section class="card">
        <h2>{m.account_controls_mute_title()}</h2>
        {#if muteSettings}<p class="limit">{m.account_controls_limit({ count: muteSettings.limitCount })}</p>{/if}
        <form class="add-row" onsubmit={(event) => { event.preventDefault(); addUserMute(); }}>
          <input bind:value={mutedUserId} inputmode="numeric" aria-label={m.account_controls_user_id()} placeholder={m.account_controls_user_id()} />
          <button disabled={pendingMutation}>{m.account_controls_add_user_mute()}</button>
        </form>
        <form class="add-row" onsubmit={(event) => { event.preventDefault(); addTagMute(); }}>
          <input bind:value={mutedTag} maxlength="100" aria-label={m.account_controls_tag()} placeholder={m.account_controls_tag()} />
          <button disabled={pendingMutation}>{m.account_controls_add_tag_mute()}</button>
        </form>
        <h3>{m.account_controls_muted_users()}</h3>
        <div class="items">
          {#each muteSettings?.users ?? [] as item (item.user.id)}
            <article class="user-row">
              {#if item.user.avatarUrl}<PixivImage url={item.user.avatarUrl} alt="" />{/if}
              <a href={`/users/${item.user.id}`}><strong>{item.user.name}</strong><small>@{item.user.account} · {item.user.id}</small></a>
              <button class="secondary" disabled={pendingMutation} onclick={() => void mutate(() => setUserMute(item.user.id, false), reloadMutes)}>{m.account_controls_remove()}</button>
            </article>
          {:else}<p class="empty">{m.account_controls_empty()}</p>{/each}
        </div>
        <h3>{m.account_controls_muted_tags()}</h3>
        <div class="tags">
          {#each muteSettings?.tags ?? [] as item (item.name)}
            <button class="tag secondary" disabled={pendingMutation} onclick={() => void mutate(() => setTagMute(item.name, false), reloadMutes)}>#{item.name} ×</button>
          {:else}<p class="empty">{m.account_controls_empty()}</p>{/each}
        </div>
      </section>

      <section class="card local-only">
        <h2>{m.account_controls_local_title()}</h2>
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .account-controls{width:min(920px,calc(100% - 32px));margin:0 auto;padding:28px 0 110px}.page-heading{margin:24px 0}.page-heading h1{font-size:var(--type-title);margin:0 0 8px}.card>p,.state,.empty{color:#777;line-height:1.7}.card{background:#fff;border:1px solid #e7e7e7;border-radius:20px;padding:24px;margin:18px 0}.card h2{margin:0 0 6px}.card h3{margin:24px 0 10px}.add-row{display:flex;gap:10px;margin:16px 0}.add-row input{min-width:0;flex:1;border:1px solid #ddd;border-radius:12px;padding:12px 14px;font:inherit}.add-row button,.wide{border:0;border-radius:12px;background:#0096fa;color:#fff;padding:0 18px;font-weight:700}.items{display:grid;gap:8px}.user-row{display:flex;align-items:center;gap:12px;padding:10px 0;border-top:1px solid #eee}.user-row :global(img){width:44px;height:44px;border-radius:50%;object-fit:cover}.user-row a{display:grid;flex:1;color:inherit;text-decoration:none}.user-row small{color:#888}.secondary{border:1px solid #d8d8d8;background:#fff;color:#555;border-radius:999px;padding:9px 14px}.wide{width:100%;margin-top:12px}.tags{display:flex;flex-wrap:wrap;gap:8px}.tag{font-weight:600}.error{background:#fff1f1;color:#b3261e;padding:12px 16px;border-radius:12px}.local-only{background:#f5f7f8}.limit{font-size:var(--type-body)}button:disabled{opacity:.5}@media(max-width:560px){.account-controls{width:min(100% - 24px,920px);padding-top:18px}.card{padding:18px;border-radius:16px}.add-row{align-items:stretch}.add-row button{max-width:42%}.user-row{align-items:flex-start;flex-wrap:wrap}.user-row a{min-width:calc(100% - 64px)}}
</style>
