<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkCard from "$lib/components/ArtworkCard.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import NovelCard from "$lib/components/NovelCard.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { currentAppLocale, m } from "$lib/i18n";
  import { recallNavigationView, rememberNavigationView } from "$lib/navigation-view-memory";
  import {
    describeDataFailure,
    getUserDetail,
    getUserIllustrations,
    getUserNovels,
    recordBrowsingHistory,
    setUserFollow,
  } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { session, sessionRestoring } from "$lib/session";
  import type {
    IllustrationSummary,
    NovelSummary,
    UserDetail,
    UserWorkKind,
  } from "$lib/types";

  type ProfileWorkKind = UserWorkKind | "novel";

  let detail = $state<UserDetail | null>(null);
  let works = $state<IllustrationSummary[]>([]);
  let novels = $state<NovelSummary[]>([]);
  let workKind = $state<ProfileWorkKind>("illust");
  let profileStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let worksStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let profileError = $state("");
  let worksError = $state("");
  let nextCursor = $state<string | null>(null);
  let loadingMore = $state(false);
  let followed = $state(false);
  let followPending = $state(false);
  let followError = $state("");
  let requestedProfileKey = $state("");
  let requestedWorksKey = $state("");
  let profileSequence = 0;
  let worksSequence = 0;
  let userId = $derived(page.params.id ?? "");
  let biography = $derived(detail ? plainPixivText(detail.comment) : "");
  let isOwnProfile = $derived(Boolean($session.user?.id && $session.user.id === userId));

  type UserDetailSnapshot = {
    detail: UserDetail | null;
    works: IllustrationSummary[];
    novels: NovelSummary[];
    workKind: ProfileWorkKind;
    profileStatus: "idle" | "loading" | "ready" | "error";
    worksStatus: "idle" | "loading" | "ready" | "error";
    profileError: string;
    worksError: string;
    nextCursor: string | null;
    followed: boolean;
    followError: string;
    requestedProfileKey: string;
    requestedWorksKey: string;
  };

  export const snapshot = {
    capture: () => rememberNavigationView<UserDetailSnapshot>({
      detail, works, novels, workKind, profileStatus, worksStatus, profileError,
      worksError, nextCursor, followed, followError, requestedProfileKey, requestedWorksKey,
    }),
    restore: (key: unknown) => {
      const value = recallNavigationView<UserDetailSnapshot>(key);
      if (!value) return;
      profileSequence += 1;
      worksSequence += 1;
      detail = value.detail;
      works = value.works;
      novels = value.novels;
      workKind = value.workKind;
      profileStatus = value.profileStatus === "loading" ? "idle" : value.profileStatus;
      worksStatus = value.worksStatus === "loading" ? "idle" : value.worksStatus;
      profileError = value.profileError;
      worksError = value.worksError;
      nextCursor = value.nextCursor;
      followed = value.followed;
      followError = value.followError;
      requestedProfileKey = value.profileStatus === "loading" ? "" : value.requestedProfileKey;
      requestedWorksKey = value.worksStatus === "loading" ? "" : value.requestedWorksKey;
      loadingMore = false;
      followPending = false;
    },
  };

  $effect(() => {
    followed = detail?.user.isFollowed ?? false;
    followError = "";
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && userId ? `${sessionKey}:${userId}` : "";
    if (!key) {
      profileSequence += 1;
      requestedProfileKey = "";
      detail = null;
      profileStatus = "idle";
      return;
    }
    if (key !== requestedProfileKey) {
      requestedProfileKey = key;
      void loadProfile(key, userId);
    }
  });

  $effect(() => {
    const sessionKey = $session.loggedIn ? ($session.user?.id ?? "logged-in") : "";
    const key = sessionKey && userId ? `${sessionKey}:${userId}:${workKind}` : "";
    if (!key) {
      worksSequence += 1;
      requestedWorksKey = "";
      works = [];
      novels = [];
      nextCursor = null;
      worksStatus = "idle";
      return;
    }
    if (key !== requestedWorksKey) {
      requestedWorksKey = key;
      void loadWorks(key, userId, workKind);
    }
  });

  async function loadProfile(key: string, id: string) {
    const sequence = ++profileSequence;
    profileStatus = "loading";
    profileError = "";
    try {
      const nextDetail = await getUserDetail(id);
      if (sequence !== profileSequence || key !== requestedProfileKey) return;
      detail = nextDetail;
      void recordBrowsingHistory({
        kind: "user",
        resourceId: nextDetail.user.id,
        title: nextDetail.user.name || m.user_pixiv_user(),
        subtitle: nextDetail.user.account ? `@${nextDetail.user.account}` : m.user_pixiv_author(),
        thumbnailUrl: nextDetail.user.avatarUrl,
      }).catch(() => undefined);
      profileStatus = "ready";
    } catch (error) {
      if (sequence !== profileSequence || key !== requestedProfileKey) return;
      profileError = describeDataFailure(error);
      profileStatus = "error";
    }
  }

  async function loadWorks(key: string, id: string, kind: ProfileWorkKind) {
    const sequence = ++worksSequence;
    worksStatus = "loading";
    worksError = "";
    works = [];
    novels = [];
    nextCursor = null;
    try {
      if (kind === "novel") {
        const nextPage = await getUserNovels(id);
        if (sequence !== worksSequence || key !== requestedWorksKey) return;
        novels = nextPage.novels;
        nextCursor = nextPage.nextCursor ?? null;
      } else {
        const nextPage = await getUserIllustrations(id, kind);
        if (sequence !== worksSequence || key !== requestedWorksKey) return;
        works = nextPage.illustrations;
        nextCursor = nextPage.nextCursor ?? null;
      }
      worksStatus = "ready";
    } catch (error) {
      if (sequence !== worksSequence || key !== requestedWorksKey) return;
      worksError = describeDataFailure(error);
      worksStatus = "error";
    }
  }

  async function loadMore() {
    const cursor = nextCursor;
    if (!cursor || loadingMore || !userId) return;
    const sequence = worksSequence;
    const key = requestedWorksKey;
    loadingMore = true;
    worksError = "";
    try {
      if (workKind === "novel") {
        const nextPage = await getUserNovels(userId, cursor);
        if (sequence !== worksSequence || key !== requestedWorksKey) return;
        const knownIds = new Set(novels.map((novel) => novel.id));
        novels = [...novels, ...nextPage.novels.filter((novel) => !knownIds.has(novel.id))];
        nextCursor = nextPage.nextCursor ?? null;
      } else {
        const nextPage = await getUserIllustrations(userId, workKind, cursor);
        if (sequence !== worksSequence || key !== requestedWorksKey) return;
        const knownIds = new Set(works.map((work) => work.id));
        works = [...works, ...nextPage.illustrations.filter((work) => !knownIds.has(work.id))];
        nextCursor = nextPage.nextCursor ?? null;
      }
    } catch (error) {
      if (sequence === worksSequence && key === requestedWorksKey) {
        worksError = describeDataFailure(error);
      }
    } finally {
      loadingMore = false;
    }
  }

  function retryProfile() {
    if (requestedProfileKey && userId) void loadProfile(requestedProfileKey, userId);
  }

  function retryWorks() {
    if (requestedWorksKey && userId) void loadWorks(requestedWorksKey, userId, workKind);
  }

  async function toggleFollow() {
    if (!detail || followPending || isOwnProfile) return;
    const previous = followed;
    followed = !previous;
    followPending = true;
    followError = "";
    try {
      await setUserFollow(detail.user.id, followed);
      detail.user.isFollowed = followed;
    } catch (error) {
      followed = previous;
      followError = describeDataFailure(error);
    } finally {
      followPending = false;
    }
  }

  function avatarInitial(name: string): string {
    return Array.from(name.trim())[0]?.toUpperCase() ?? "P";
  }

  function formatCount(value: number): string {
    return new Intl.NumberFormat(currentAppLocale()).format(value);
  }
</script>

<svelte:head>
  <title>{detail?.user.name || m.user_detail()} · PixNya</title>
</svelte:head>

<AppShell title={m.user_detail()}>
  <main class="user-page">
    <ReturnLink fallback="/" label={m.user_return_source()} />

  {#if !$sessionRestoring && !$session.loggedIn}
      <section class="state-card">
        <Icon name="user" size={28} />
        <div><h1>{m.user_login_title()}</h1><p>{m.user_login_description()}</p></div>
<a href="/login">{m.common_go_to_login()}</a>
      </section>
    {:else if profileStatus === "loading"}
      <section class="state-card"><span class="spinner"></span><div><h1>{m.user_loading_title()}</h1><p>{m.user_loading_description()}</p></div></section>
    {:else if profileStatus === "error"}
      <section class="state-card error" role="alert">
        <span>!</span><div><h1>{m.user_load_failed()}</h1><p>{profileError}</p></div>
        <button type="button" onclick={retryProfile}>{m.common_retry()}</button>
      </section>
    {:else if detail}
      <section class="profile-card">
        <div class="profile-banner">
          {#if detail.profile.backgroundImageUrl}
            <PixivImage url={detail.profile.backgroundImageUrl} alt="" cacheKind="preview" />
          {/if}
        </div>
        <div class="profile-main">
          <span class="avatar">
            <b>{avatarInitial(detail.user.name)}</b>
            <PixivImage url={detail.user.avatarUrl} alt="" />
          </span>
          <div class="profile-copy">
            <div class="name-row">
              <h1>{detail.user.name}</h1>
              {#if detail.profile.isPremium}<span>Premium</span>{/if}
            </div>
            <p>@{detail.user.account}</p>
            {#if biography}<p class="bio">{biography}</p>{/if}
            <div class="profile-meta">
              {#if detail.profile.region}<span>{detail.profile.region}</span>{/if}
              {#if detail.profile.job}<span>{detail.profile.job}</span>{/if}
              {#if detail.profile.twitterAccount}<span>𝕏 @{detail.profile.twitterAccount}</span>{/if}
            </div>
          </div>
          {#if isOwnProfile}
            <span class="follow-state active">{m.common_current_user()}</span>
          {:else}
            <button
              type="button"
              class="follow-state"
              class:active={followed}
              disabled={followPending}
              onclick={toggleFollow}
            >{followPending ? m.common_processing() : followed ? m.common_following() : m.common_follow()}</button>
          {/if}
          {#if followError}<small class="follow-error" role="alert">{followError}</small>{/if}
        </div>
        <dl class="profile-stats">
          <div><dt>{m.common_illustrations()}</dt><dd>{formatCount(detail.profile.totalIllustrations)}</dd></div>
          <div><dt>{m.common_manga()}</dt><dd>{formatCount(detail.profile.totalManga)}</dd></div>
          <div><dt>{m.common_novels()}</dt><dd>{formatCount(detail.profile.totalNovels)}</dd></div>
          <div><dt>{m.common_follow()}</dt><dd>{formatCount(detail.profile.totalFollowUsers)}</dd></div>
          <div><dt>{m.user_mypixiv()}</dt><dd>{formatCount(detail.profile.totalMypixivUsers)}</dd></div>
          <div><dt>{m.common_bookmark_count()}</dt><dd>{formatCount(detail.profile.totalIllustrationBookmarks)}</dd></div>
        </dl>
      </section>

      <section class="works-section">
        <header>
          <div><h2>{m.user_public_works()}</h2></div>
          <nav aria-label={m.user_work_type()}>
            <button type="button" class:active={workKind === "illust"} onclick={() => (workKind = "illust")}>{m.common_illustrations()}</button>
            <button type="button" class:active={workKind === "manga"} onclick={() => (workKind = "manga")}>{m.common_manga()}</button>
            <button type="button" class:active={workKind === "novel"} onclick={() => (workKind = "novel")}>{m.common_novels()}</button>
          </nav>
        </header>

        {#if worksStatus === "loading"}
          <div class="works-loading"><span class="spinner"></span><p>{m.user_works_loading()}</p></div>
        {:else if worksStatus === "error"}
          <div class="works-error" role="alert"><p>{worksError}</p><button type="button" onclick={retryWorks}>{m.common_retry()}</button></div>
        {:else if workKind === "novel" && novels.length > 0}
          <div class="novel-grid">{#each novels as novel (novel.id)}<NovelCard {novel} />{/each}</div>
        {:else if works.length > 0}
          <div class="works-grid">
            {#each works as illustration, index (illustration.id)}
              <ArtworkCard {illustration} tone={(index % 6) + 1} />
            {/each}
          </div>
        {:else if worksStatus === "ready"}
          <p class="empty">{m.user_empty_works({ kind: workKind === "illust" ? m.common_illustrations() : workKind === "manga" ? m.common_manga() : m.common_novels() })}</p>
        {/if}

        {#if worksError && worksStatus === "ready"}<p class="pagination-error" role="alert">{worksError}</p>{/if}
        {#if nextCursor && worksStatus === "ready"}
          <button class="load-more" type="button" disabled={loadingMore} onclick={loadMore}>
            {loadingMore ? m.common_loading() : m.user_load_more_works()}
          </button>
        {/if}
      </section>
    {/if}
  </main>
</AppShell>

<style>
  .user-page { width: min(1080px, 100%); margin: 0 auto; padding: 24px 28px 56px; }
  .state-card { display: grid; grid-template-columns: 46px minmax(0, 1fr) auto; gap: 16px; align-items: center; margin-top: 22px; padding: 22px; border: 1px solid var(--line); border-radius: 12px; background: white; }
  .state-card h1 { margin: 0; font-size: var(--type-label); }
  .state-card p { margin: 5px 0 0; color: var(--muted); font-size: var(--type-small); }
  .state-card a, .state-card button { padding: 10px 17px; color: white; border: 0; border-radius: 20px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; text-decoration: none; }
  .state-card.error > span { display: grid; width: 40px; height: 40px; place-items: center; color: #b65364; border-radius: 50%; background: #fff0f3; font-weight: 800; }
  .spinner { width: 32px; height: 32px; border: 3px solid #dceefb; border-top-color: var(--pixiv-blue); border-radius: 50%; animation: spin .8s linear infinite; }

  .profile-card { overflow: hidden; margin-top: 20px; border: 1px solid var(--line); border-radius: 13px; background: white; }
  .profile-banner { position: relative; height: 190px; overflow: hidden; background: linear-gradient(120deg, #b9e4fb, #dbeef8 48%, #e8dbf2); }
  .profile-banner :global(img) { position: absolute; inset: 0; object-fit: cover !important; }
  .profile-main { display: grid; grid-template-columns: 104px minmax(0, 1fr) auto; gap: 20px; align-items: center; padding: 0 28px 24px; }
  .avatar { position: relative; display: grid; width: 104px; height: 104px; overflow: hidden; place-items: center; margin-top: -45px; color: white; border: 5px solid white; border-radius: 50%; background: var(--pixiv-blue); box-shadow: 0 3px 14px rgba(0,0,0,.12); }
  .avatar b { font-size: var(--type-title); }
  .avatar :global(img) { position: absolute; z-index: 1; inset: 0; }
  .profile-copy { padding-top: 18px; }
  .name-row { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
  .name-row h1 { margin: 0; font-size: var(--type-title); }
  .name-row span { padding: 4px 7px; color: #a06c1f; border-radius: 4px; background: #fff4d8; font-size: var(--type-caption); font-weight: 750; }
  .profile-copy > p { margin: 5px 0 0; color: var(--muted); font-size: var(--type-small); }
  .profile-copy .bio { max-width: 650px; margin-top: 12px; color: #4f5559; line-height: 1.65; white-space: pre-line; }
  .profile-meta { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 11px; }
  .profile-meta span { padding: 5px 8px; color: #66757e; border-radius: 4px; background: #f2f5f7; font-size: var(--type-caption); }
  .follow-state { min-width: 82px; padding: 10px 16px; color: var(--pixiv-blue); border: 1px solid #b9def5; border-radius: 19px; background: white; cursor: pointer; font-size: var(--type-body); font-weight: 750; text-align: center; }
  .follow-state.active { color: #66727a; border-color: var(--line); background: #f7f7f7; }
  .follow-state:disabled { cursor: wait; opacity: .65; }
  .follow-error { grid-column: 3; color: #a44f5e; font-size: var(--type-caption); text-align: right; }
  .profile-stats { display: grid; grid-template-columns: repeat(6, 1fr); margin: 0; border-top: 1px solid var(--line); }
  .profile-stats div { padding: 15px 8px; text-align: center; }
  .profile-stats div + div { border-left: 1px solid var(--line); }
  .profile-stats dt { color: var(--muted); font-size: var(--type-caption); }
  .profile-stats dd { margin: 5px 0 0; font-size: var(--type-small); font-weight: 750; }

  .works-section { margin-top: 36px; }
  .works-section > header { display: flex; gap: 18px; align-items: flex-end; justify-content: space-between; margin-bottom: 16px; }
  .works-section h2 { margin: 0; font-size: var(--type-section); }
  .works-section nav { display: flex; gap: 4px; padding: 4px; border-radius: 20px; background: #f3f3f3; }
  .works-section nav button { min-width: 62px; height: 31px; color: #777; border: 0; border-radius: 16px; background: transparent; cursor: pointer; font-size: var(--type-body); }
  .works-section nav button.active { color: #333; background: white; box-shadow: 0 1px 4px rgba(0,0,0,.08); font-weight: 700; }
  .works-grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 22px 14px; }
  .novel-grid { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); gap: 14px; }
  .works-loading, .works-error { display: flex; gap: 14px; align-items: center; justify-content: center; min-height: 150px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: var(--type-small); }
  .works-error { flex-direction: column; color: #9b5964; }
  .works-error p { margin: 0; }
  .works-error button, .load-more { min-width: 110px; height: 36px; color: #59636a; border: 1px solid var(--line); border-radius: 18px; background: white; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .empty { padding: 38px; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; font-size: var(--type-small); text-align: center; }
  .pagination-error { color: #a65865; font-size: var(--type-caption); text-align: center; }
  .load-more { display: block; margin: 26px auto 0; }
  .load-more:disabled { cursor: wait; opacity: .65; }

  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 860px) {
    .profile-stats { grid-template-columns: repeat(3, 1fr); }
    .profile-stats div:nth-child(4) { border-left: 0; border-top: 1px solid var(--line); }
    .profile-stats div:nth-child(n+5) { border-top: 1px solid var(--line); }
    .works-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }
  @media (max-width: 620px) {
    .user-page { padding: 18px 12px 44px; }
    .state-card { grid-template-columns: 40px minmax(0, 1fr); padding: 17px; }
    .state-card a, .state-card button { grid-column: 1 / -1; text-align: center; }
    .profile-banner { height: 124px; }
    .profile-main { grid-template-columns: 78px minmax(0, 1fr); gap: 13px; padding: 0 16px 18px; }
    .avatar { width: 78px; height: 78px; margin-top: -31px; }
    .profile-copy { padding-top: 12px; }
    .name-row h1 { font-size: var(--type-section); }
    .follow-state { grid-column: 1 / -1; width: 100%; }
    .follow-error { grid-column: 1 / -1; text-align: center; }
    .profile-stats { grid-template-columns: repeat(3, 1fr); }
    .works-section > header { align-items: stretch; flex-direction: column; }
    .works-section nav { align-self: flex-start; }
    .works-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 18px 11px; }
    .novel-grid { grid-template-columns: 1fr; }
  }
  @media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
</style>
