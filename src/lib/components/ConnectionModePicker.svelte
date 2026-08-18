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

<div
  class="probe-state"
  class:checking={state === "checking"}
  class:available={state === "available"}
  class:failed={state === "failed"}
  aria-live="polite"
>
  <span class:checking={state === "checking"}></span>
  <strong>{statusLabels[state]()}</strong>
  {#if message}<em>{message}</em>{/if}
</div>

<style>
  .mode-list {
    display: grid;
    gap: 10px;
  }
  .mode-list button {
    display: flex;
    width: 100%;
    min-height: 60px;
    align-items: center;
    gap: 14px;
    padding: 0 17px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: white;
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition: border-color 150ms ease, background 150ms ease, box-shadow 150ms ease;
  }
  .mode-list button:hover:not(:disabled) { border-color: #d7e8f3; background: #f8fcff; }
  .mode-list button.selected {
    border-color: var(--pixiv-blue);
    background: #eef8ff;
    box-shadow: 0 0 0 1px rgba(0, 150, 250, 0.08);
  }
  .mode-list button:disabled { cursor: default; opacity: 0.65; }
  .radio {
    width: 21px;
    height: 21px;
    box-sizing: border-box;
    flex: 0 0 auto;
    border: 2px solid #9aa0a6;
    border-radius: 50%;
    background: white;
    transition: border-width 150ms ease, border-color 150ms ease;
  }
  .selected .radio {
    border: 6px solid var(--pixiv-blue);
  }
  strong { font-size: 14px; }
  .probe-state {
    display: flex;
    min-height: 42px;
    align-items: center;
    gap: 9px;
    margin-top: 12px;
    padding: 0 14px;
    color: var(--muted);
    border-radius: 12px;
    background: var(--soft-surface);
  }
  .probe-state > span {
    width: 8px;
    height: 8px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: #aeb4ba;
  }
  .probe-state > span.checking {
    background: var(--pixiv-blue);
    animation: pulse 0.9s ease-in-out infinite alternate;
  }
  .probe-state.checking { color: var(--pixiv-blue); }
  .probe-state.available { color: var(--success); }
  .probe-state.available > span { background: var(--success); }
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
