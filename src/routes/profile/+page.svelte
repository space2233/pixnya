<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
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
    preferredConnectionMode = readPreferredConnectionMode();
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
      sessionError = "退出登录失败，安全存储中的会话可能尚未删除。";
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
    if (mode === "ech") return "ECH 直连";
    if (mode === "compatible") return "低安全直连";
    return "标准模式";
  }

  function describeSessionError(error: unknown): string {
    const kind =
      error && typeof error === "object" && "kind" in error
        ? String((error as { kind: unknown }).kind)
        : "";
    const messages: Record<string, string> = {
      oauth_configuration_unavailable: "此构建缺少 OAuth 兼容参数，无法恢复登录。",
      token_client_unavailable: "无法创建 OAuth 客户端，请检查系统时间与 TLS 环境。",
      token_transport_unavailable: "保存的连接模式暂时无法到达 Pixiv OAuth 服务。",
      token_request_failed: "恢复登录时网络请求失败，请稍后重试。",
      token_rejected: "Pixiv 已拒绝保存的登录凭据，请重新登录。",
      invalid_token_response: "Pixiv 返回的会话数据无效，请重新登录。",
      secure_storage_unavailable: "无法读取平台安全存储，请检查系统凭据服务。",
      session_unavailable: "本地会话状态不可用，请重启应用。",
    };
    return messages[kind] ?? "无法恢复登录状态，请稍后重试。";
  }
</script>

<svelte:head>
  <title>个人主页 · PixNya</title>
</svelte:head>

<AppShell title="个人主页">
  <div class="profile-page">
    <section class="profile-card">
      <div class="profile-banner">
        {#if accountDetail?.profile.backgroundImageUrl}
          <PixivImage url={accountDetail.profile.backgroundImageUrl} alt="Pixiv 个人背景图" cacheKind="preview" />
        {:else if accountStatus === "ready"}
          <span class="profile-banner-note"><Icon name="image" size={15} />Pixiv 账户未设置个人背景图</span>
        {:else if $session.loggedIn}
          <span class="profile-banner-note">正在同步个人背景图…</span>
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
            <h1>正在恢复会话…</h1>
            <p>refresh token 仅从平台安全存储读取。</p>
          {:else if $session.loggedIn && $session.user}
            <h1>{accountDetail?.user.name ?? $session.user.name}</h1>
            <p>
              @{accountDetail?.user.account ?? $session.user.account}{accountDetail?.profile.isPremium || $session.user.isPremium
                ? " · Pixiv Premium"
                : ""}
            </p>
            {#if accountComment}<p class="profile-comment">{accountComment}</p>{/if}
          {:else}
            <h1>尚未登录</h1>
            <p>登录后显示头像、昵称、关注状态和公开作品。</p>
          {/if}
        </div>
        {#if !$sessionRestoring}
          {#if $session.loggedIn}
            <button class="login-button logout-button" type="button" disabled={isLoggingOut} onclick={logOut}>
              {isLoggingOut ? "正在退出…" : "退出登录"}
            </button>
          {:else}
            <a class="login-button" href={`/login?mode=${preferredConnectionMode}`}>使用 Pixiv 登录</a>
          {/if}
        {/if}
      </div>

      {#if sessionError}<p class="session-error">{sessionError}</p>{/if}
      {#if accountStatus === "loading"}<p class="account-status">正在同步账户统计…</p>{/if}
      {#if accountStatus === "error"}
        <div class="account-error" role="alert">
          <span>{accountError}</span><button type="button" onclick={retryAccountDetail}>重试</button>
        </div>
      {/if}

      <dl class="profile-stats">
        <div>
          <dt>作品</dt>
          <dd>{accountDetail ? accountDetail.profile.totalIllustrations + accountDetail.profile.totalManga + accountDetail.profile.totalNovels : "—"}</dd>
        </div>
        <div><a class="stat-link" href="/following/users"><dt>关注</dt><dd>{accountDetail?.profile.totalFollowUsers ?? "—"}</dd></a></div>
        <div><dt>好P友</dt><dd>{accountDetail?.profile.totalMypixivUsers ?? "—"}</dd></div>
      </dl>
    </section>

    <div class="profile-columns">
      <section class="quick-section">
        <header><h2>快捷入口</h2><p>快速访问常用账号内容和应用设置</p></header>
        <nav>
          {#if $session.loggedIn && $session.user}
            <a href={`/users/${$session.user.id}`}><span><Icon name="image" size={20} /></span><b>我的公开作品</b><small>查看账户资料、插画与漫画</small><i>›</i></a>
          {/if}
          <a href="/bookmarks"><span><Icon name="heart" size={20} /></span><b>我的收藏</b><small>公开与非公开收藏</small><i>›</i></a>
          <a href="/following"><span><Icon name="user" size={20} /></span><b>关注新作</b><small>查看关注作者的最新投稿</small><i>›</i></a>
          <a href="/settings"><span><Icon name="settings" size={20} /></span><b>应用设置</b><small>连接、界面、存储与隐私</small><i>›</i></a>
        </nav>
      </section>

      <aside class="local-card">
        <span><Icon name="shield" size={22} /></span>
        <div><small>本地隐私</small><h2>凭据不在此页面输入</h2></div>
        <p>登录只使用 Pixiv 官方页面；本页只展示登录完成后的非敏感账户资料。</p>
        <dl>
          <div><dt>离线空间</dt><dd>{offlineStats ? formatBytes(offlineStats.sizeBytes) : "—"}</dd></div>
          <div><dt>离线内容</dt><dd>{offlineStats?.entryCount ?? "—"}</dd></div>
          <div>
            <dt>登录状态</dt>
            <dd>
              {$session.loggedIn
                ? `已建立 · ${connectionModeLabel($session.connectionMode)}`
                : "未建立"}
            </dd>
          </div>
        </dl>
        {#if $session.connectionMode === "compatible"}
          <p class="session-risk">当前会话刷新令牌时会使用已确认的低安全直连。</p>
        {/if}
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

  .quick-section header p {
    margin: 5px 0 0;
    color: var(--muted);
    font-size: 8px;
  }

  .quick-section nav {
    display: grid;
  }

  .quick-section a {
    display: grid;
    min-height: 68px;
    grid-template-columns: 38px minmax(0, 1fr) 16px;
    grid-template-rows: 1fr 1fr;
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
    align-self: end;
    font-size: 10px;
    line-height: 1.35;
  }

  .quick-section small {
    min-width: 0;
    grid-column: 2;
    grid-row: 2;
    align-self: start;
    margin-top: 3px;
    color: var(--muted);
    font-size: 8px;
    line-height: 1.4;
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

  .local-card small {
    color: #4d9871;
    font-size: 8px;
    font-weight: 700;
  }

  .local-card h2 {
    margin-top: 4px;
  }

  .local-card > p {
    margin: 8px 0 17px;
    color: var(--muted);
    font-size: 8px;
    line-height: 1.65;
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

  .session-risk {
    color: #a43e52 !important;
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

    .quick-section small {
      font-size: 10px;
    }
  }

  @media (max-width: 420px) {
    .profile-page {
      padding-right: 12px;
      padding-left: 12px;
    }
  }
</style>
