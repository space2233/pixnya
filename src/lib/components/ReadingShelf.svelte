<script lang="ts">
  import { onMount } from "svelte";
  import { m } from "$lib/i18n";
  import { PREFERENCES_CHANGED_EVENT, r18DefaultVisible } from "$lib/preferences";
  import { readReadingShelf, recentReading, readNovelProgress, removeFromReadingShelf,
    setReadingStatus, READING_SHELF_CHANGED_EVENT, type ReadingShelfEntry, type ReadingStatus } from "$lib/reading-shelf";
  let { compact = false }: { compact?: boolean } = $props();
  let entries = $state<ReadingShelfEntry[]>([]);
  let filter = $state<ReadingStatus | "all">("all");
  let error = $state("");
  let visible = $derived(compact ? recentReading(entries) : entries.filter((entry) => filter === "all" || entry.status === filter));
  const statuses: ReadingStatus[] = ["planned", "reading", "finished"];
  const labels = { planned: m.reading_planned, reading: m.reading_in_progress, finished: m.reading_finished };
  onMount(() => {
    const refresh = () => { entries = readReadingShelf(); };
    refresh();
    window.addEventListener(READING_SHELF_CHANGED_EVENT, refresh);
    window.addEventListener(PREFERENCES_CHANGED_EVENT, refresh);
    window.addEventListener("storage", refresh);
    return () => {
      window.removeEventListener(READING_SHELF_CHANGED_EVENT, refresh);
      window.removeEventListener(PREFERENCES_CHANGED_EVENT, refresh);
      window.removeEventListener("storage", refresh);
    };
  });
  function update(action: () => void) {
    try { action(); error = ""; } catch { error = m.reading_save_failed(); }
  }
</script>

<section class="shelf" class:compact>
  <header><h2>{compact ? m.reading_continue() : m.reading_shelf_title()}</h2>{#if compact}<a href="/novels/shelf">{m.reading_shelf_title()} ›</a>{/if}</header>
  {#if !compact}
    <nav aria-label={m.reading_shelf_title()}>
      <button class:active={filter === "all"} onclick={() => { filter = "all"; }}>{m.reading_all()}</button>
      {#each statuses as status}<button class:active={filter === status} onclick={() => { filter = status; }}>{labels[status]()}</button>{/each}
    </nav>
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
  {#if visible.length === 0}<p class="empty">{compact ? m.reading_recent_empty() : m.reading_shelf_empty()}</p>{/if}
  <div class="entries">
    {#each visible as entry (entry.id)}
      <article>
        <a class="book" href={`/novels/${entry.id}/read`}>
          <strong>{entry.xRestrict > 0 && !$r18DefaultVisible ? m.novel_reader_restricted_title() : entry.title || m.common_untitled()}</strong>
          <span>{entry.xRestrict > 0 && !$r18DefaultVisible ? "R18" : entry.author}</span>
          <span class="progress">{m.reading_progress({ percent: String(Math.round(readNovelProgress(entry.id) * 100)) })}</span>
        </a>
        {#if !compact}
          <div class="actions">
            <select aria-label={m.reading_status()} value={entry.status} onchange={(event) => update(() => setReadingStatus(entry.id, event.currentTarget.value as ReadingStatus))}>
              {#each statuses as status}<option value={status}>{labels[status]()}</option>{/each}
            </select>
            <button onclick={() => update(() => removeFromReadingShelf(entry.id))}>{m.reading_remove()}</button>
          </div>
        {/if}
      </article>
    {/each}
  </div>
</section>

<style>
  .shelf { margin: 0 0 24px; }
  header { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  h2 { margin: 12px 0; font-size: var(--type-section); }
  header a { color: var(--pixiv-blue); font-size: var(--type-small); text-decoration: none; }
  nav { display: flex; flex-wrap: wrap; gap: 8px; margin: 8px 0 18px; }
  button, select { padding: 9px 13px; border: 1px solid var(--line); border-radius: 16px; background: var(--surface); color: var(--text); font-size: var(--type-body); }
  button.active { background: var(--pixiv-blue); color: white; border-color: var(--pixiv-blue); }
  .entries { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
  article { min-width: 0; border: 1px solid var(--line); border-radius: 14px; background: var(--soft-surface); }
  .book { display: grid; gap: 8px; padding: 16px; color: var(--text); text-decoration: none; }
  strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: var(--type-body); }
  span, .empty { font-size: var(--type-small); color: var(--muted); }
  .progress { color: var(--pixiv-blue); }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; padding: 0 16px 16px; }
  .compact .entries { display: flex; overflow-x: auto; padding-bottom: 6px; }
  .compact article { flex: 0 0 210px; }
  @media (max-width: 600px) { .entries { grid-template-columns: 1fr; } .compact article { flex-basis: 180px; } }
</style>
