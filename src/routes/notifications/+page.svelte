<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
</script>

<svelte:head><title>通知能力 · PixNya</title></svelte:head>

<AppShell title="通知">
  <main class="capability-page">
    <section>
      <span class="capability-icon"><Icon name="bell" size={31} /></span>
      <small>明确的不支持项</small>
      <h1>当前版本不提供 Pixiv 站内通知</h1>
      <p>Pixiv App API 没有为第三方客户端提供稳定、可验证的完整通知接口。为避免显示虚构数据、漏报消息或依赖网页登录 Cookie，本客户端不会伪造通知列表。</p>
      <div class="boundary">
        <Icon name="shield" size={20} />
        <div><strong>不会用网页会话绕过此限制</strong><span>官方登录 WebView 仅用于授权；Cookie 不会交给普通页面或数据接口。</span></div>
      </div>
      <nav>
        <a href="/following">查看关注新作</a>
        <a class="secondary" href="/offline">打开离线资料库</a>
      </nav>
    </section>
  </main>
</AppShell>

<style>
  .capability-page { display: grid; min-height: calc(100vh - var(--topbar-height)); padding: 32px 20px 70px; place-items: center; }
  section { width: min(570px,100%); padding: 34px; border: 1px solid var(--line); border-radius: 14px; background: white; box-shadow: 0 12px 36px rgba(0,0,0,.04); text-align: center; }
  .capability-icon { display: grid; width: 68px; height: 68px; margin: 0 auto 17px; place-items: center; color: #78909e; border-radius: 50%; background: #eef4f7; }
  section > small { color: #9a6a30; font-size: 8px; font-weight: 800; letter-spacing: .08em; }
  h1 { margin: 8px 0 0; font-size: 21px; }
  section > p { margin: 13px auto 0; color: var(--muted); font-size: 10px; line-height: 1.75; }
  .boundary { display: grid; grid-template-columns: 34px minmax(0,1fr); gap: 10px; align-items: center; margin-top: 23px; padding: 14px; color: #39765a; border-radius: 9px; background: #edf8f2; text-align: left; }
  .boundary strong, .boundary span { display: block; } .boundary strong { font-size: 10px; } .boundary span { margin-top: 4px; color: #63806f; font-size: 8px; line-height: 1.55; }
  nav { display: flex; gap: 9px; justify-content: center; margin-top: 23px; }
  nav a { display: grid; min-width: 132px; height: 38px; place-items: center; color: white; border: 1px solid var(--pixiv-blue); border-radius: 19px; background: var(--pixiv-blue); font-size: 9px; font-weight: 700; text-decoration: none; }
  nav a.secondary { color: #53636d; border-color: var(--line); background: white; }
  @media (max-width: 620px) { .capability-page { min-height: calc(100vh - var(--topbar-height) - var(--bottom-nav-height)); padding: 18px 12px 90px; } section { padding: 27px 20px; } nav { align-items: stretch; flex-direction: column; } nav a { width: 100%; } }
</style>
