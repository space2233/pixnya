<script lang="ts">
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m, readLanguagePreference, setLanguagePreference, type LanguagePreference } from "$lib/i18n";
  import {
    readDesktopSidebarExpanded,
    readReducedMotion,
    readR18DefaultVisible,
    writeDesktopSidebarExpanded,
    writeReducedMotion,
    writeR18DefaultVisible,
  } from "$lib/preferences";

  let language = $state<LanguagePreference>("system");
  let sidebar = $state(true);
  let reducedMotion = $state(false);
  let showR18 = $state(false);
  onMount(() => {
    language = readLanguagePreference();
    sidebar = readDesktopSidebarExpanded();
    reducedMotion = readReducedMotion();
    showR18 = readR18DefaultVisible();
  });
</script>

<svelte:head><title>{m.settings_interface()} · PixNya</title></svelte:head>
<AppShell title={m.settings_interface()}>
  <div class="page">
    <ReturnLink fallback="/settings" label={m.common_back()} />
    <h1>{m.settings_interface()}</h1>
    <section>
      <label><strong>{m.language_settings_title()}</strong><select value={language} onchange={(event) => { language = (event.currentTarget as HTMLSelectElement).value as LanguagePreference; setLanguagePreference(language); }}><option value="system">{m.language_system()}</option><option value="zh-CN">{m.language_simplified_chinese()}</option><option value="zh-TW">{m.language_traditional_chinese()}</option><option value="en-US">{m.language_english()}</option></select></label>
      <label><strong>{m.settings_sidebar()}</strong><input type="checkbox" role="switch" bind:checked={sidebar} onchange={() => writeDesktopSidebarExpanded(sidebar)} /></label>
      <label><strong>{m.settings_reduced_motion()}</strong><input type="checkbox" role="switch" bind:checked={reducedMotion} onchange={() => writeReducedMotion(reducedMotion)} /></label>
      <label><strong>{m.settings_r18()}</strong><input type="checkbox" role="switch" bind:checked={showR18} onchange={() => writeR18DefaultVisible(showR18)} /></label>
    </section>
  </div>
</AppShell>

<style>
  .page { width: min(720px,100%); box-sizing:border-box; margin:auto; padding:30px 24px 60px; } h1{margin:24px 0;font-size:28px} section{overflow:hidden;border:1px solid var(--line);border-radius:18px;background:white} label{display:flex;min-height:62px;align-items:center;justify-content:space-between;gap:20px;padding:0 18px;border-bottom:1px solid var(--line)} label:last-child{border:0} select{max-width:55%;padding:8px 10px;border:1px solid var(--line);border-radius:10px;background:white} input{width:20px;height:20px;accent-color:var(--brand)}
</style>
