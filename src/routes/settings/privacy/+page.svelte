<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { clearFrontendLocalData } from "$lib/local-data";
  import { clearDiagnosticLogs, clearLocalData, exportDiagnosticLogs, getBrowsingHistory, getDiagnosticLogSummary, setBrowsingHistoryEnabled } from "$lib/pixiv-api";
  import { applySessionSnapshot } from "$lib/session";
  import type { DiagnosticLogSummary, HistorySnapshot, LocalDataClearFailure } from "$lib/types";

  const localDataFailureLabels: Record<LocalDataClearFailure, () => string> = {
    secure_storage: m.settings_failure_secure_storage,
    session: m.settings_failure_session,
    login_state: m.settings_failure_login_state,
    transport_state: m.settings_failure_transport_state,
    offline_library: m.settings_failure_offline_library,
    media_cache: m.settings_failure_media_cache,
    login_web_view: m.settings_failure_login_webview,
    diagnostic_log: m.settings_failure_diagnostic_log,
    download_queue: m.settings_failure_download_queue,
    storage_settings: m.settings_failure_storage,
    export_settings: m.settings_failure_export,
    update_settings: m.settings_failure_updates,
    local_catalog: m.settings_failure_catalog,
    browsing_history: m.settings_failure_history,
  };

  let history = $state<HistorySnapshot | null>(null);
  let logs = $state<DiagnosticLogSummary | null>(null);
  let busy = $state(false);
  let confirmClear = $state(false);
  let confirmation = $state("");
  let notice = $state("");
  let error = $state(false);
  onMount(() => { void reload(); });
  async function reload(){ const [h,l]=await Promise.allSettled([getBrowsingHistory(),getDiagnosticLogSummary()]); history=h.status==="fulfilled"?h.value:null;logs=l.status==="fulfilled"?l.value:null; }
  async function toggleHistory(){ if(!history)return;await act(async()=>{history=await setBrowsingHistoryEnabled(!history!.enabled)}); }
  async function exportLogs(){await act(async()=>{const result=await exportDiagnosticLogs();notice=result.destination;});}
  async function clearLogs(){await act(async()=>{logs=await clearDiagnosticLogs();});}
  async function clearEverything(){
    if(confirmation!==m.settings_clear_confirmation_word())return;
    await act(async()=>{
      const report=await clearLocalData("CLEAR_LOCAL_DATA");
      clearFrontendLocalData();
      applySessionSnapshot({loggedIn:false});
      if(!report.complete){
        error=true;
        notice=m.settings_local_data_partial({failures:report.failedSteps.map((step)=>localDataFailureLabels[step]()).join(", ")});
        confirmClear=false;
        confirmation="";
        return;
      }
      await goto("/setup/connection",{replaceState:true});
    });
  }
  async function act(action:()=>Promise<void>){if(busy)return;busy=true;notice="";error=false;try{await action()}catch{error=true;notice=m.settings_local_data_failed()}finally{busy=false;}}
</script>

<svelte:head><title>{m.settings_privacy()} · PixNya</title></svelte:head>
<AppShell title={m.settings_privacy()}><div class="page"><ReturnLink fallback="/settings" label={m.common_back()} /><h1 class="page-title">{m.settings_privacy()}</h1><section>
  <label class="row"><strong>{m.settings_history()}</strong><input type="checkbox" role="switch" checked={history?.enabled ?? false} disabled={!history||busy} onchange={toggleHistory}/></label>
  <a class="row" href="/history"><strong>{m.settings_manage_history()}</strong><i>›</i></a>
  <div class="row log-row">
    <strong>{m.settings_diagnostic_log()}</strong>
    <span class="log-count">{logs?.entryCount ?? "—"}</span>
    <div class="log-actions">
      <button class="log-export" type="button" disabled={!logs||busy} onclick={exportLogs}>{m.settings_export_log()}</button>
      <button class="log-clear" type="button" disabled={!logs||busy} onclick={clearLogs}>{m.settings_clear_log()}</button>
    </div>
  </div>
  <div class="row danger"><strong>{m.settings_clear_all()}</strong><button disabled={busy} onclick={()=>(confirmClear=true)}>{m.settings_clear_data()}</button></div>
</section>{#if notice}<p class:error role="status">{notice}</p>{/if}</div></AppShell>
{#if confirmClear}<div class="dialog-layer"><button class="scrim" aria-label={m.common_cancel()} onclick={()=>(confirmClear=false)}></button><div role="alertdialog" aria-modal="true"><h2>{m.settings_local_dialog_title()}</h2><label><span>{m.settings_clear_confirmation_prompt({word:m.settings_clear_confirmation_word()})}</span><input bind:value={confirmation} autocomplete="off"/></label><footer><button onclick={()=>(confirmClear=false)}>{m.common_cancel()}</button><button class="primary" disabled={confirmation!==m.settings_clear_confirmation_word()||busy} onclick={clearEverything}>{m.settings_clear_data()}</button></footer></div></div>{/if}
<style>
  .page { width: min(720px,100%); box-sizing: border-box; margin: auto; padding: 30px 24px 60px; }
  h1 { margin: 24px 0; font-size: var(--type-title); }
  section { overflow: hidden; border: 1px solid var(--line); border-radius: 18px; background: white; }
  .row { display: flex; min-height: 62px; align-items: center; gap: 12px; padding: 0 18px; border-bottom: 1px solid var(--line); color: var(--text); text-decoration: none; }
  .row:last-child { border: 0; }
  .row input, .row span, .row i { margin-left: auto; }
  .row i { font-style: normal; }
  .row button { padding: 8px 12px; border: 1px solid var(--line); border-radius: 16px; background: white; }
  .log-row { display: grid; grid-template-columns: minmax(0,1fr) auto auto; }
  .row .log-count { margin-left: 0; color: var(--muted); }
  .log-actions { display: grid; grid-template-columns: repeat(2,max-content); gap: 8px; }
  .log-actions button { min-height: 44px; padding: 0 14px; font-size: var(--type-body); white-space: nowrap; }
  .log-export { color: var(--pixiv-blue); border-color: #cde7f8 !important; }
  .log-clear { color: var(--danger); }
  .danger button { color: var(--danger); }
  .error { color: var(--danger); }
  .dialog-layer { position: fixed; z-index: 1000; inset: 0; display: grid; place-items: center; padding: 20px; }
  .scrim { position: absolute; inset: 0; border: 0; background: #0008; }
  .dialog-layer > div { position: relative; width: min(420px,100%); box-sizing: border-box; padding: 24px; border-radius: 18px; background: white; }
  .dialog-layer label span, .dialog-layer label input { display: block; width: 100%; box-sizing: border-box; }
  .dialog-layer input { margin-top: 10px; padding: 10px; }
  .dialog-layer footer { display: flex; justify-content: flex-end; gap: 10px; margin-top: 18px; }
  .dialog-layer footer button { padding: 10px 18px; border: 1px solid var(--line); border-radius: 20px; background: white; }
  .dialog-layer footer .primary { background: var(--pixiv-blue); color: white; }
  @media (max-width: 600px) {
    .log-row { grid-template-columns: minmax(0,1fr) auto; padding-top: 12px; padding-bottom: 12px; }
    .log-actions { grid-column: 1 / -1; grid-template-columns: repeat(2,minmax(0,1fr)); width: 100%; margin-top: 10px; }
  }
</style>
