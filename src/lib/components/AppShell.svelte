<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";
  import {
    bottomNavigationKeys,
    getNavigationItem,
    navigationKeyForPath,
    sideNavigationSections,
  } from "$lib/navigation";
  import {
    PREFERENCES_CHANGED_EVENT,
    readDesktopSidebarExpanded,
    syncR18DefaultVisible,
    writeDesktopSidebarExpanded,
  } from "$lib/preferences";
  import { initializeSession, session, sessionRestoring } from "$lib/session";
  import { recordSearchHistory } from "$lib/search-history";
  import { m } from "$lib/i18n";
  import Icon from "./Icon.svelte";
  import PixivImage from "./PixivImage.svelte";

  let { children, title }: { children: Snippet; title: string } = $props();

  let isDesktop = $state(false);
  let sidebarVisible = $state(true);
  let drawerOpen = $state(false);
  let searchQuery = $state("");
  let avatarStatus = $state<"loading" | "ready" | "error">("loading");
  let activeKey = $derived(navigationKeyForPath(page.url.pathname));
  let activeItem = $derived(activeKey ? getNavigationItem(activeKey) : null);
  let mobileTitle = $derived(
    activeItem &&
      (page.url.pathname === activeItem.href || page.url.pathname === `${activeItem.href}/`)
      ? activeItem.label
      : title,
  );
  let avatarUrl = $derived($session.loggedIn ? ($session.user?.avatarUrl ?? null) : null);
  let settingsItem = $derived(getNavigationItem("settings"));

  $effect(() => {
    if (activeKey === "search") {
      searchQuery = page.url.searchParams.get("q") ?? "";
    }
  });

  onMount(() => {
    void initializeSession().catch(() => {});
    const desktopMedia = window.matchMedia("(min-width: 960px)");
    const syncSidebarPreference = () => {
      sidebarVisible = readDesktopSidebarExpanded();
      syncR18DefaultVisible();
    };
    syncSidebarPreference();

    const syncViewport = () => {
      isDesktop = desktopMedia.matches;
      if (isDesktop) drawerOpen = false;
    };

    syncViewport();
    desktopMedia.addEventListener("change", syncViewport);
    window.addEventListener(PREFERENCES_CHANGED_EVENT, syncSidebarPreference);
    return () => {
      desktopMedia.removeEventListener("change", syncViewport);
      window.removeEventListener(PREFERENCES_CHANGED_EVENT, syncSidebarPreference);
    };
  });

  function toggleNavigation() {
    if (isDesktop) {
      sidebarVisible = !sidebarVisible;
      writeDesktopSidebarExpanded(sidebarVisible);
      return;
    }

    drawerOpen = !drawerOpen;
  }

  function closeDrawer() {
    drawerOpen = false;
  }

  function submitSearch(event: SubmitEvent) {
    event.preventDefault();
    const query = searchQuery.trim();
    if (query) recordSearchHistory(query);
    void goto(query ? `/search?q=${encodeURIComponent(query)}` : "/search");
  }

  function avatarInitial(name: string): string {
    return Array.from(name.trim())[0]?.toUpperCase() ?? "P";
  }

</script>

<div
  class="app-frame"
  class:sidebar-hidden={isDesktop && !sidebarVisible}
  data-session-restoring={$sessionRestoring ? "true" : "false"}
>
  <aside class="side-panel" class:drawer-open={drawerOpen} aria-label={m.shell_main_navigation()}>
    <div class="side-brand">
      <a href="/" aria-label={m.shell_pixnya_home()} onclick={closeDrawer}>
        <strong>PixNya</strong>
      </a>
      <small>UNOFFICIAL</small>
    </div>

    <nav class="side-nav">
      {#each sideNavigationSections as section, sectionIndex}
        {#if sectionIndex > 0}<div class="nav-divider"></div>{/if}
        <div class="nav-group" aria-label={section.label()}>
          {#each section.items as key}
            {@const item = getNavigationItem(key)}
            <a
              class:active={activeKey === item.key}
              href={item.href}
              aria-current={activeKey === item.key ? "page" : undefined}
              onclick={closeDrawer}
            >
              <Icon name={item.icon} size={20} /><span>{item.label}</span>
            </a>
          {/each}
        </div>
      {/each}
    </nav>

    <div class="side-footer">
      <a
        class:active={activeKey === settingsItem.key}
        href={settingsItem.href}
        aria-current={activeKey === settingsItem.key ? "page" : undefined}
        onclick={closeDrawer}
      >
        <Icon name={settingsItem.icon} size={20} /><span>{settingsItem.label}</span>
      </a>
    </div>
  </aside>

  {#if drawerOpen}
    <button class="drawer-scrim" type="button" aria-label={m.shell_close_navigation()} onclick={closeDrawer}></button>
  {/if}

  <div class="app-column">
    <header class="app-topbar">
      <button class="icon-button menu-button" type="button" aria-label={m.shell_toggle_navigation()} onclick={toggleNavigation}>
        <Icon name="menu" size={24} />
      </button>

      <div class="mobile-title">{mobileTitle}</div>

      <form class="search-box" role="search" onsubmit={submitSearch}>
        <Icon name="search" size={18} />
        <input bind:value={searchQuery} type="search" maxlength="100" placeholder={m.shell_search_placeholder()} aria-label={m.navigation_search()} />
      </form>

      <div class="top-actions">
        <button class="text-action" type="button" disabled title={m.shell_post_unavailable()}>{m.shell_post()}</button>
        <a
          class="icon-button"
          class:active={activeKey === "notifications"}
          href={getNavigationItem("notifications").href}
          aria-label={m.navigation_notifications()}
          title={m.navigation_notifications()}
          aria-current={activeKey === "notifications" ? "page" : undefined}
        >
          <Icon name="bell" size={20} />
        </a>
        <a
          class="login-avatar"
          class:active={activeKey === "profile"}
          href={getNavigationItem("profile").href}
          aria-label={$session.loggedIn && $session.user
            ? m.shell_user_profile({ name: $session.user.name })
            : m.navigation_profile()}
          aria-current={activeKey === "profile" ? "page" : undefined}
        >
          {#if avatarUrl}
            <PixivImage url={avatarUrl} alt="" onstatus={(status) => (avatarStatus = status)} />
          {/if}
          {#if $session.loggedIn && $session.user && (!avatarUrl || avatarStatus !== "ready")}
            <b>{avatarInitial($session.user.name)}</b>
          {:else if !$session.loggedIn || !$session.user}
            <Icon name="user" size={19} />
          {/if}
        </a>
      </div>
    </header>

    <main class="app-content">
      {@render children()}
    </main>
  </div>

  <nav class="mobile-bottom-nav" aria-label={m.shell_mobile_navigation()}>
    {#each bottomNavigationKeys as key}
      {@const item = getNavigationItem(key)}
      <a
        class:active={activeKey === item.key}
        href={item.href}
        aria-current={activeKey === item.key ? "page" : undefined}
      >
        <Icon name={item.icon} size={23} /><span>{item.compactLabel}</span>
      </a>
    {/each}
  </nav>

</div>
