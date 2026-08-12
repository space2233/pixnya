<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";
  import { describeDataFailure, getOfflineStats, getUserDetail } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import { readPreferredConnectionMode } from "$lib/preferences";
  import { initializeSession, logoutSession, session, sessionRestoring } from "$lib/session";
  import type { OfflineStats, UserDetail } from "$lib/types";

  let preferredConnectionMode = $state("standard");
  let isLoggingOut = $state(false);
  let sessionError = $state<string | null>(null);
  let avatarStatus = $state<"loading" | "ready" | "error">("loading");
  let accountDetail = $state<UserDetail | null>(null);
  let accountStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let accountError = $state("");
  let offlineStats = $state<OfflineStats | null>(null);
  let requestedUserId = $state("");
  let accountSequence = 0;
  let avatarUrl = $derived(
    $session.loggedIn ? (accountDetail?.user.avatarUrl ?? $session.user?.avatarUrl ?? null) : null,
  );
  let accountComment = $derived(accountDetail ? plainPixivText(accountDetail.comment) : "");

  $effect(() => {
    const userId = $session.loggedIn ? ($session.user?.id ?? "") : "";
    if (!userId) {
      accountSequence += 1;
      requestedUserId = "";
      accountDetail = null;
      accountStatus = "idle";
      accountError = "";
      return;
    }
    if (userId !== requestedUserId) {
      requestedUserId = userId;
      void loadAccountDetail(userId);
    }
  });

  onMount(() => {
    preferredConnectionMode = readPreferredConnectionMode() ?? "standard";
    void loadOfflineStats();
    void initializeSession().catch((error) => {
      sessionError = describeSessionError(error);
    });
  });

  async function loadOfflineStats() {
    try {
      offlineStats = await getOfflineStats();
    } catch {
      offlineStats = null;
    }
  }

  function formatBytes(value: number): string {
    if (value < 1024) return `${value} B`;
    if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
    return `${(value / 1024 ** 3).toFixed(2)} GiB`;
  }

  async function logOut() {
    if (isLoggingOut) return;
    isLoggingOut = true;
    sessionError = null;
    try {
      await logoutSession();
    } catch {
      sessionError = m.profile_logout_failed();
    } finally {
      isLoggingOut = false;
    }
  }

  async function loadAccountDetail(userId: string) {
    const sequence = ++accountSequence;
    accountStatus = "loading";
    accountError = "";
    try {
      const nextDetail = await getUserDetail(userId);
      if (sequence !== accountSequence || requestedUserId !== userId) return;
      accountDetail = nextDetail;
      accountStatus = "ready";
    } catch (error) {
      if (sequence !== accountSequence || requestedUserId !== userId) return;
      accountError = describeDataFailure(error);
      accountStatus = "error";
    }
  }

  function retryAccountDetail() {
    if (requestedUserId) void loadAccountDetail(requestedUserId);
  }

  function avatarInitial(name: string): string {
    return Array.from(name.trim())[0]?.toUpperCase() ?? "P";
  }

  function connectionModeLabel(mode: typeof $session.connectionMode): string {
    if (mode === "ech") return m.login_mode_ech();
    if (mode === "compatible") return m.login_mode_compatible();
    return m.login_mode_standard();
  }

  function describeSessionError(error: unknown): string {
    const kind =
      error && typeof error === "object" && "kind" in error
        ? String((error as { kind: unknown }).kind)
        : "";
    const messages: Record<string, () => string> = {
      oauth_configuration_unavailable: m.profile_restore_oauth_unavailable,
      token_client_unavailable: m.profile_restore_client_unavailable,
      token_transport_unavailable: m.profile_restore_transport_unavailable,
      token_request_failed: m.profile_restore_request_failed,
      token_rejected: m.profile_restore_token_rejected,
      invalid_token_response: m.profile_restore_invalid_response,
      secure_storage_unavailable: m.profile_restore_storage_unavailable,
      session_unavailable: m.profile_restore_session_unavailable,
    };
    return messages[kind]?.() ?? m.profile_restore_failed();
  }
</script>

<svelte:head>
  <title>{m.profile_title()} · PixNya</title>
</svelte:head>

<AppShell title={m.profile_title()}>
  <div class="profile-page">
    <section class="profile-card">
      <div class="profile-banner">
        {#if accountDetail?.profile.backgroundImageUrl}
          <PixivImage url={accountDetail.profile.backgroundImageUrl} alt={m.profile_background_alt()} cacheKind="preview" />
        {:else if accountStatus === "ready"}
          <span class="profile-banner-note"><Icon name="image" size={15} />{m.profile_background_none()}</span>
        {:else if $session.loggedIn}
          <span class="profile-banner-note">{m.profile_background_syncing()}</span>
        {/if}
      </div>
      <div class="profile-main">
        <span class="profile-avatar">
          {#if avatarUrl}
            <PixivImage url={avatarUrl} alt="" onstatus={(status) => (avatarStatus = status)} />
          {/if}
          {#if $session.loggedIn && $session.user && (!avatarUrl || avatarStatus !== "ready")}
            <b>{avatarInitial($session.user.name)}</b>
          {:else if !$sessionRestoring && (!$session.loggedIn || !$session.user)}
            <Icon name="user" size={34} />
          {/if}
        </span>
        <div class="profile-copy">
          {#if $sessionRestoring}
            <h1>{m.profile_restoring()}</h1>
            <p>{m.profile_secure_refresh_note()}</p>
          {:else if $session.loggedIn && $session.user}
            <h1>{accountDetail?.user.name ?? $session.user.name}</h1>
            <p>
              @{accountDetail?.user.account ?? $session.user.account}{accountDetail?.profile.isPremium || $session.user.isPremium
                ? " · Pixiv Premium"
                : ""}
            </p>
            {#if accountComment}<p class="profile-comment">{accountComment}</p>{/if}
          {:else}
            <h1>{m.profile_not_signed_in()}</h1>
          {/if}
        </div>
        {#if !$sessionRestoring}
          {#if $session.loggedIn}
            <button class="login-button logout-button" type="button" disabled={isLoggingOut} onclick={logOut}>
              {isLoggingOut ? m.profile_logging_out() : m.profile_logout()}
            </button>
          {:else}
            <a class="login-button" href={`/login?mode=${preferredConnectionMode}`}>{m.profile_login_pixiv()}</a>
          {/if}
        {/if}
      </div>

      {#if sessionError}<p class="session-error">{sessionError}</p>{/if}
      {#if accountStatus === "loading"}<p class="account-status">{m.profile_syncing_stats()}</p>{/if}
      {#if accountStatus === "error"}
        <div class="account-error" role="alert">
          <span>{accountError}</span><button type="button" onclick={retryAccountDetail}>{m.common_retry()}</button>
        </div>
      {/if}

      <dl class="profile-stats">
        <div>
          <dt>{m.profile_stat_works()}</dt>
          <dd>{accountDetail ? accountDetail.profile.totalIllustrations + accountDetail.profile.totalManga + accountDetail.profile.totalNovels : "—"}</dd>
        </div>
        <div><a class="stat-link" href="/following/users"><dt>{m.profile_stat_following()}</dt><dd>{accountDetail?.profile.totalFollowUsers ?? "—"}</dd></a></div>
        <div><dt>{m.profile_stat_friends()}</dt><dd>{accountDetail?.profile.totalMypixivUsers ?? "—"}</dd></div>
      </dl>
    </section>

    <div class="profile-columns">
      <section class="quick-section">
        <header><h2>{m.profile_quick_links()}</h2></header>
        <nav>
          {#if $session.loggedIn && $session.user}
            <a href={`/users/${$session.user.id}`}><span><Icon name="image" size={20} /></span><b>{m.profile_public_works()}</b><i>›</i></a>
          {/if}
          <a href="/bookmarks"><span><Icon name="heart" size={20} /></span><b>{m.profile_bookmarks()}</b><i>›</i></a>
          <a href="/following"><span><Icon name="user" size={20} /></span><b>{m.profile_following_new()}</b><i>›</i></a>
          <a href="/settings"><span><Icon name="settings" size={20} /></span><b>{m.profile_settings()}</b><i>›</i></a>
        </nav>
      </section>

      <aside class="local-card">
        <span><Icon name="shield" size={22} /></span>
        <div><h2>{m.profile_credentials_title()}</h2></div>
        <dl>
          <div><dt>{m.profile_offline_space()}</dt><dd>{offlineStats ? formatBytes(offlineStats.sizeBytes) : "—"}</dd></div>
          <div><dt>{m.profile_offline_content()}</dt><dd>{offlineStats?.entryCount ?? "—"}</dd></div>
          <div>
            <dt>{m.profile_login_status()}</dt>
            <dd>
              {$session.loggedIn
                ? m.profile_session_ready({ mode: connectionModeLabel($session.connectionMode) })
                : m.profile_session_missing()}
            </dd>
          </div>
        </dl>
      </aside>
    </div>
  </div>
</AppShell>

<style>
  .profile-page {
    width: min(940px, 100%);
    margin: 0 auto;
    padding: 34px 28px 52px;
  }

  .profile-card {
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: white;
  }

  .profile-banner {
    position: relative;
    height: 116px;
    overflow: hidden;
    background: linear-gradient(120deg, #b9e4fb, #dbeef8 48%, #e8dbf2);
  }

  .profile-banner :global(img) {
    position: absolute;
    inset: 0;
    object-fit: cover !important;
  }

  .profile-banner-note {
    position: absolute;
    right: 16px;
    bottom: 12px;
    display: flex;
    gap: 6px;
    align-items: center;
    padding: 6px 10px;
    color: #5b7180;
    border: 1px solid rgba(255, 255, 255, 0.66);
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.58);
    font-size: 8px;
    backdrop-filter: blur(8px);
  }

  .profile-main {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr) auto;
    gap: 18px;
    align-items: center;
    padding: 20px 24px;
  }

  .profile-avatar {
    display: grid;
    width: 88px;
    height: 88px;
    place-items: center;
    margin-top: 0;
    color: var(--pixiv-blue);
    border: 5px solid white;
    border-radius: 50%;
    background: #eff8fd;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.08);
    overflow: hidden;
  }

  .profile-avatar b {
    font-size: 28px;
  }

  .profile-copy {
    min-width: 0;
  }

  .profile-copy h1 {
    margin: 4px 0 0;
    font-size: 21px;
  }

  .profile-copy p {
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 9px;
  }

  .profile-copy .profile-comment {
    max-width: 520px;
    margin-top: 8px;
    color: #555;
    line-height: 1.55;
    white-space: pre-line;
  }

  .login-button {
    display: grid;
    min-width: 154px;
    height: 40px;
    place-items: center;
    color: white;
    border-radius: 20px;
    background: var(--pixiv-blue);
    font-size: 10px;
    font-weight: 700;
    text-decoration: none;
  }

  .logout-button {
    border: 1px solid var(--line);
    color: var(--text);
    background: white;
    cursor: pointer;
  }

  .logout-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .session-error {
    margin: 0;
    padding: 10px 24px 14px;
    color: #a43e52;
    font-size: 9px;
  }

  .account-status {
    margin: 0;
    padding: 10px 24px 14px;
    color: var(--muted);
    font-size: 9px;
  }

  .account-error {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    padding: 10px 24px 14px;
    color: #a43e52;
    font-size: 9px;
  }

  .account-error button {
    flex: 0 0 auto;
    padding: 6px 12px;
    color: var(--pixiv-blue);
    border: 1px solid #b9def5;
    border-radius: 15px;
    background: white;
    cursor: pointer;
    font-size: 9px;
    font-weight: 700;
  }

  .profile-stats {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin: 0;
    border-top: 1px solid var(--line);
  }

  .profile-stats div {
    padding: 15px;
    text-align: center;
  }

  .stat-link { display: block; color: inherit; text-decoration: none; }
  .stat-link:hover dt, .stat-link:hover dd { color: var(--pixiv-blue); }

  .profile-stats div + div {
    border-left: 1px solid var(--line);
  }

  .profile-stats dt {
    color: var(--muted);
    font-size: 8px;
  }

  .profile-stats dd {
    margin: 4px 0 0;
    font-size: 13px;
    font-weight: 700;
  }

  .profile-columns {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(250px, 0.75fr);
    gap: 18px;
    margin-top: 20px;
  }

  .quick-section,
  .local-card {
    border: 1px solid var(--line);
    border-radius: 11px;
    background: white;
  }

  .quick-section header {
    padding: 19px 20px 15px;
    border-bottom: 1px solid var(--line);
  }

  h2 {
    margin: 0;
    font-size: 15px;
  }

  .quick-section nav {
    display: grid;
  }

  .quick-section a {
    display: grid;
    min-height: 68px;
    grid-template-columns: 38px minmax(0, 1fr) 16px;
    grid-template-rows: 1fr;
    gap: 0 11px;
    align-items: center;
    padding: 12px 17px;
    color: var(--text);
    text-decoration: none;
  }

  .quick-section a + a {
    border-top: 1px solid var(--line);
  }

  .quick-section a > span {
    display: grid;
    width: 38px;
    height: 38px;
    grid-column: 1;
    grid-row: 1 / -1;
    place-items: center;
    color: var(--pixiv-blue);
    border-radius: 50%;
    background: #edf8ff;
  }

  .quick-section b {
    min-width: 0;
    grid-column: 2;
    grid-row: 1;
    align-self: center;
    font-size: 10px;
    line-height: 1.35;
  }

  .quick-section i {
    grid-column: 3;
    grid-row: 1 / -1;
    align-self: center;
    justify-self: end;
    color: #aaa;
    font-size: 19px;
    font-style: normal;
  }

  .local-card {
    padding: 20px;
  }

  .local-card > span {
    display: grid;
    width: 42px;
    height: 42px;
    place-items: center;
    color: #4d9871;
    border-radius: 50%;
    background: #edf8f2;
  }

  .local-card > div {
    margin-top: 15px;
  }

  .local-card h2 {
    margin-top: 4px;
  }

  .local-card dl {
    margin: 0;
    border-top: 1px solid var(--line);
  }

  .local-card dl div {
    display: flex;
    justify-content: space-between;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
    font-size: 8px;
  }

  .local-card dt {
    color: var(--muted);
  }

  .local-card dd {
    margin: 0;
    font-weight: 700;
  }

  @media (max-width: 720px) {
    .profile-page {
      padding: 24px 16px 42px;
    }

    .profile-main {
      grid-template-columns: 74px minmax(0, 1fr);
      padding: 18px;
    }

    .profile-avatar {
      width: 74px;
      height: 74px;
    }

    .login-button {
      width: 100%;
      grid-column: 1 / -1;
    }

    .profile-columns {
      grid-template-columns: 1fr;
    }

    .quick-section a {
      min-height: 72px;
      grid-template-columns: 40px minmax(0, 1fr) 16px;
      gap: 0 12px;
      padding: 12px 16px;
    }

    .quick-section a > span {
      width: 40px;
      height: 40px;
    }

    .quick-section b {
      font-size: 12px;
    }

  }

  @media (max-width: 420px) {
    .profile-page {
      padding-right: 12px;
      padding-left: 12px;
    }
  }
</style>
