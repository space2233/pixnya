<script lang="ts">
  import AppShell from "$lib/components/AppShell.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import {
    commitLocalBackupRestore,
    acknowledgeLocalBackupFrontendRecovery,
    createLocalBackup,
    describeDataFailure,
    rollbackLocalBackupRestore,
    selectLocalBackup,
    startLocalBackupRestore,
  } from "$lib/pixiv-api";
  import { collectFrontendBackupState, restoreFrontendBackupState } from "$lib/local-backup";
  import type { BackupRestoreStrategy, BackupSelectionResult } from "$lib/types";

  let includeOffline = $state(false);
  let selected = $state<BackupSelectionResult | null>(null);
  let strategy = $state<BackupRestoreStrategy>("merge");
  let busy = $state(false);
  let confirmRestore = $state(false);
  let status = $state("");
  let failed = $state(false);

  async function createBackup() {
    await perform(async () => {
      const result = await createLocalBackup(collectFrontendBackupState(), includeOffline);
      status = m.backup_created({ destination: result.destination });
    });
  }

  async function chooseBackup() {
    await perform(async () => {
      const result = await selectLocalBackup();
      if (!result.cancelled) selected = result;
    }, false);
  }

  async function restoreBackup() {
    confirmRestore = false;
    await perform(async () => {
      const previousFrontend = collectFrontendBackupState();
      const result = await startLocalBackupRestore(strategy, previousFrontend);
      try {
        restoreFrontendBackupState(result.frontend);
        await commitLocalBackupRestore(result.transactionId);
      } catch (error) {
        await rollbackLocalBackupRestore(result.transactionId).catch(() => {
          throw { kind: "backup_rollback_failed" };
        });
        restoreFrontendBackupState(previousFrontend);
        await acknowledgeLocalBackupFrontendRecovery(result.transactionId).catch(() => {});
        throw error;
      }
      selected = null;
      status = m.backup_restored();
      window.location.reload();
    });
  }

  async function perform(action: () => Promise<void>, clearStatus = true) {
    if (busy) return;
    busy = true;
    failed = false;
    if (clearStatus) status = "";
    try { await action(); }
    catch (error) { failed = true; status = describeDataFailure(error); }
    finally { busy = false; }
  }

  function bytes(value: number): string {
    if (value < 1024 ** 2) return `${(value / 1024).toFixed(1)} KiB`;
    if (value < 1024 ** 3) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
    return `${(value / 1024 ** 3).toFixed(2)} GiB`;
  }
</script>

<svelte:head><title>{m.settings_data_backup()} · PixNya</title></svelte:head>
<AppShell title={m.settings_data_backup()}>
  <main class="page">
    <ReturnLink fallback="/settings" label={m.common_back()} />
    <h1>{m.settings_data_backup()}</h1>
    <h2>{m.backup_create_title()}</h2>
    <section>
      <a class="row" href="/settings/storage"><strong>{m.settings_export_directory()}</strong><span>{m.backup_export_directory_hint()}</span><i>›</i></a>
      <label class="row"><strong>{m.backup_include_offline()}</strong><input type="checkbox" role="switch" bind:checked={includeOffline} disabled={busy} /></label>
      <button class="action" type="button" disabled={busy} onclick={createBackup}>{busy ? m.settings_reading() : m.backup_create_action()}</button>
    </section>
    <h2>{m.backup_restore_title()}</h2>
    <section>
      <button class="row picker" type="button" disabled={busy} onclick={chooseBackup}><strong>{m.backup_choose_file()}</strong><span>{selected?.label ?? "—"}</span><i>›</i></button>
      {#if selected?.preview}
        <div class="summary"><span>PixNya {selected.preview.applicationVersion}</span><span>{bytes(selected.preview.totalBytes)}</span><span>{selected.preview.offlineIncluded ? m.backup_offline_count({ count: selected.preview.offlineFileCount }) : m.backup_without_offline()}</span></div>
        <label class="row"><strong>{m.backup_restore_mode()}</strong><select bind:value={strategy} disabled={busy}><option value="merge">{m.backup_restore_merge()}</option><option value="replace">{m.backup_restore_replace()}</option></select></label>
        <button class="action danger" type="button" disabled={busy} onclick={() => (confirmRestore = true)}>{m.backup_restore_action()}</button>
      {/if}
    </section>
    {#if status}<p class:failed role="status">{status}</p>{/if}
  </main>
</AppShell>

{#if confirmRestore}
  <div class="dialog-layer"><button class="scrim" aria-label={m.common_cancel()} onclick={() => (confirmRestore = false)}></button><div role="alertdialog" aria-modal="true"><h2>{m.backup_confirm_title()}</h2><p>{strategy === "replace" ? m.backup_confirm_replace() : m.backup_confirm_merge()}</p><footer><button onclick={() => (confirmRestore = false)}>{m.common_cancel()}</button><button class="primary" disabled={busy} onclick={restoreBackup}>{m.backup_restore_action()}</button></footer></div></div>
{/if}

<style>
  .page{width:min(760px,100%);box-sizing:border-box;margin:auto;padding:30px 24px 60px}h1{margin:24px 0;font-size:28px}h2{margin:24px 4px 10px;color:var(--muted);font-size:13px}section{overflow:hidden;border:1px solid var(--line);border-radius:18px;background:#fff}.row{display:flex;width:100%;min-height:62px;box-sizing:border-box;align-items:center;gap:12px;padding:0 18px;border:0;border-bottom:1px solid var(--line);background:#fff;color:var(--ink);text-align:left;text-decoration:none}.row span{overflow:hidden;margin-left:auto;color:var(--muted);text-overflow:ellipsis;white-space:nowrap}.row i{color:#aeb4ba;font-size:24px;font-style:normal}.picker{cursor:pointer}input,select{margin-left:auto;accent-color:var(--brand)}select{padding:8px;border:1px solid var(--line);border-radius:10px;background:#fff}.action{width:100%;min-height:54px;border:0;background:#fff;color:var(--brand);font-weight:700}.danger{color:var(--danger)}.summary{display:flex;flex-wrap:wrap;gap:8px 18px;padding:14px 18px;border-bottom:1px solid var(--line);color:var(--muted);font-size:13px}.failed{color:var(--danger)}.dialog-layer{position:fixed;z-index:1000;inset:0;display:grid;place-items:center;padding:20px}.scrim{position:absolute;inset:0;border:0;background:#0008}.dialog-layer>div{position:relative;width:min(420px,100%);box-sizing:border-box;padding:24px;border-radius:18px;background:#fff}.dialog-layer footer{display:flex;justify-content:flex-end;gap:10px}.dialog-layer footer button{padding:10px 18px;border:1px solid var(--line);border-radius:20px;background:#fff}.dialog-layer footer .primary{border-color:var(--brand);background:var(--brand);color:#fff}
</style>
