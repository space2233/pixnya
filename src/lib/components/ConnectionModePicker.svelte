<script lang="ts">
  import { m } from "$lib/i18n";
  import type { ConnectionMode } from "$lib/types";

  type ProbeState = "idle" | "checking" | "available" | "failed";

  let {
    selected,
    state = "idle",
    message = "",
    disabled = false,
    onselect,
  }: {
    selected: ConnectionMode | null;
    state?: ProbeState;
    message?: string;
    disabled?: boolean;
    onselect: (mode: ConnectionMode) => void;
  } = $props();

  const modes: Array<{ id: ConnectionMode; label: () => string }> = [
    { id: "standard", label: m.login_mode_standard },
    { id: "ech", label: m.login_mode_ech },
    { id: "compatible", label: m.login_mode_compatible },
  ];

  const statusLabels: Record<ProbeState, () => string> = {
    idle: m.connection_status_idle,
    checking: m.connection_status_checking,
    available: m.connection_status_available,
    failed: m.connection_status_failed,
  };
</script>

<div class="mode-list" role="radiogroup" aria-label={m.network_connection_method()}>
  {#each modes as mode}
    <button
      type="button"
      role="radio"
      aria-checked={selected === mode.id}
      class:selected={selected === mode.id}
      disabled={disabled}
      onclick={() => onselect(mode.id)}
    >
      <span class="radio" aria-hidden="true"></span>
      <strong>{mode.label()}</strong>
    </button>
  {/each}
</div>

<div class="probe-state" class:failed={state === "failed"} aria-live="polite">
  <span class:checking={state === "checking"}></span>
  <strong>{statusLabels[state]()}</strong>
  {#if message}<em>{message}</em>{/if}
</div>

<style>
  .mode-list {
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: white;
  }
  .mode-list button {
    display: flex;
    width: 100%;
    min-height: 64px;
    align-items: center;
    gap: 16px;
    padding: 0 20px;
    border: 0;
    border-bottom: 1px solid var(--line);
    background: transparent;
    color: var(--ink);
    text-align: left;
    cursor: pointer;
  }
  .mode-list button:last-child { border-bottom: 0; }
  .mode-list button.selected { background: #f2f9ff; }
  .radio {
    width: 21px;
    height: 21px;
    box-sizing: border-box;
    border: 2px solid #9aa0a6;
    border-radius: 50%;
  }
  .selected .radio {
    border: 6px solid var(--brand);
  }
  strong { font-size: 15px; }
  .probe-state {
    display: flex;
    min-height: 28px;
    align-items: center;
    gap: 9px;
    margin-top: 14px;
    color: var(--muted);
  }
  .probe-state > span {
    width: 8px;
    height: 8px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: #aeb4ba;
  }
  .probe-state > span.checking {
    background: var(--brand);
    animation: pulse 0.9s ease-in-out infinite alternate;
  }
  .probe-state.failed { color: var(--danger); }
  .probe-state.failed > span { background: var(--danger); }
  .probe-state em {
    overflow: hidden;
    margin-left: auto;
    font-size: 12px;
    font-style: normal;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  @keyframes pulse { to { opacity: 0.3; } }
</style>
