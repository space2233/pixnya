<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/i18n";
  import {
    readPreferredConnectionMode,
    reconcilePreferredConnectionMode,
  } from "$lib/preferences";
  import { initializeSession, session } from "$lib/session";
  import type { ConnectionMode } from "$lib/types";

  let preferredConnectionMode = $state<ConnectionMode>("standard");
  let connectionMode = $derived<ConnectionMode>(
    $session.loggedIn && $session.connectionMode
      ? $session.connectionMode
      : preferredConnectionMode,
  );
  onMount(() => {
    preferredConnectionMode = readPreferredConnectionMode() ?? "standard";
    void initializeSession()
      .then((snapshot) => {
        preferredConnectionMode =
          reconcilePreferredConnectionMode(snapshot) ?? preferredConnectionMode;
      })
      .catch(() => {});
  });

  const connectionLabels: Record<ConnectionMode, () => string> = {
    standard: m.login_mode_standard,
    ech: m.login_mode_ech,
    compatible: m.login_mode_compatible,
  };

  const groups = [
    {
      title: m.settings_account,
      rows: [
        ["/settings/network", "shield", m.settings_connection, () => connectionLabels[connectionMode]()],
        ["/settings/account-controls", "user", m.settings_account, () => $session.loggedIn ? ($session.user?.name ?? m.settings_logged_in()) : m.settings_logged_out()],
      ],
    },
    {
      title: m.settings_interface,
      rows: [
        ["/settings/interface", "settings", m.settings_interface, () => ""],
        ["/settings/storage", "download", m.settings_storage, () => ""],
        ["/settings/data", "history", m.settings_data_backup, () => ""],
        ["/settings/updates", "download", m.settings_updates, () => ""],
        ["/settings/privacy", "shield", m.settings_privacy, () => ""],
      ],
    },
  ] as const;
</script>

<svelte:head><title>{m.settings_title()} · PixNya</title></svelte:head>

<AppShell title={m.settings_title()}>
  <div class="settings-page">
    <h1>{m.settings_title()}</h1>
    {#each groups as group}
      <h2>{group.title()}</h2>
      <section class="settings-list">
        {#each group.rows as section}
          <a href={section[0]}>
            <span class="icon"><Icon name={section[1]} size={21} /></span>
            <strong>{section[2]()}</strong>
            {#if section[3]()}<em>{section[3]()}</em>{/if}
            <i aria-hidden="true">›</i>
          </a>
        {/each}
      </section>
    {/each}
  </div>
</AppShell>

<style>
  .settings-page { width: min(760px, 100%); box-sizing: border-box; margin: 0 auto; padding: 34px 24px 60px; }
  h1 { margin: 0 0 24px; font-size: 28px; }
  h2 { margin: 24px 4px 10px; color: var(--muted); font-size: 13px; font-weight: 650; }
  .settings-list { overflow: hidden; border: 1px solid var(--line); border-radius: 18px; background: white; }
  a { display: flex; min-height: 64px; align-items: center; gap: 14px; padding: 0 18px; border-bottom: 1px solid var(--line); color: var(--ink); text-decoration: none; }
  a:last-child { border-bottom: 0; }
  .icon { display: grid; width: 38px; height: 38px; place-items: center; color: var(--brand); border-radius: 50%; background: #eef8ff; }
  strong { font-size: 15px; }
  em { overflow: hidden; margin-left: auto; color: var(--muted); font-size: 13px; font-style: normal; text-overflow: ellipsis; white-space: nowrap; }
  i { margin-left: auto; color: #aeb4ba; font-size: 26px; font-style: normal; }
  em + i { margin-left: 2px; }
  @media (max-width: 600px) { .settings-page { padding: 26px 18px 50px; } }
</style>
