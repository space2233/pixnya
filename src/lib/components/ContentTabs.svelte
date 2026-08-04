<script lang="ts">
  import { page } from "$app/state";
  import {
    contentTabKeys,
    getNavigationItem,
    navigationKeyForPath,
  } from "$lib/navigation";

  let activeKey = $derived(navigationKeyForPath(page.url.pathname));
</script>

<div class="tab-rail">
  <nav aria-label="作品类型">
    {#each contentTabKeys as key}
      {@const item = getNavigationItem(key)}
      <a
        class:active={activeKey === item.key}
        href={item.href}
        aria-current={activeKey === item.key ? "page" : undefined}
      >{item.label}</a>
    {/each}
  </nav>
</div>

<style>
  .tab-rail {
    position: sticky;
    z-index: 20;
    top: var(--topbar-height);
    border-bottom: 1px solid var(--line);
    background: rgba(255, 255, 255, 0.97);
    backdrop-filter: blur(10px);
  }

  nav {
    display: flex;
    width: min(1120px, 100%);
    height: 58px;
    gap: 34px;
    align-items: stretch;
    margin: 0 auto;
    padding: 0 28px;
  }

  a {
    position: relative;
    display: grid;
    min-width: 56px;
    place-items: center;
    padding: 0 8px;
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
    text-decoration: none;
  }

  a:hover {
    color: var(--text);
  }

  a.active {
    color: var(--text);
  }

  a.active::after {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 3px;
    border-radius: 3px 3px 0 0;
    background: var(--pixiv-blue);
    content: "";
  }

  @media (max-width: 959px) {
    .tab-rail {
      top: calc(var(--topbar-height) + env(safe-area-inset-top));
    }

    nav {
      height: 54px;
      gap: 0;
      padding: 0 10px;
    }

    a {
      min-width: 0;
      flex: 1;
      font-size: 12px;
    }
  }
</style>
