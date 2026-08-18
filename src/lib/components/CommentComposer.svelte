<script lang="ts">
  import { tick } from "svelte";
  import { COMMENT_EMOJIS, insertCommentEmoji, type CommentEmojiToken } from "$lib/comment-emoji";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";
  import { describeDataFailure, getCommentStamps } from "$lib/pixiv-api";
  import type { CommentSubmission, PixivCommentStamp } from "$lib/types";

  let {
    placeholder = m.comment_write_placeholder(),
    submitLabel = m.comment_publish(),
    submitting = false,
    errorMessage = "",
    autofocus = false,
    onsubmit,
  }: {
    placeholder?: string;
    submitLabel?: string;
    submitting?: boolean;
    errorMessage?: string;
    autofocus?: boolean;
    onsubmit: (submission: CommentSubmission) => Promise<boolean>;
  } = $props();

  let text = $state("");
  let pickerOpen = $state(false);
  let pickerKind = $state<"emoji" | "stamp">("emoji");
  let stamps = $state<PixivCommentStamp[]>([]);
  let stampsStatus = $state<"idle" | "loading" | "ready" | "error">("idle");
  let stampsError = $state("");
  let selectedStampId = $state<string | null>(null);
  let textarea = $state<HTMLTextAreaElement | null>(null);
  let focusedInitially = false;

  $effect(() => {
    if (autofocus && textarea && !focusedInitially) {
      focusedInitially = true;
      void tick().then(() => textarea?.focus());
    }
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const value = text.trim();
    if ((!value && !selectedStampId) || submitting) return;
    if (await onsubmit({ text: value, stampId: selectedStampId })) {
      text = "";
      selectedStampId = null;
      pickerOpen = false;
    }
  }

  async function openStampPicker() {
    pickerOpen = true;
    pickerKind = "stamp";
    if (stampsStatus !== "idle") return;
    stampsStatus = "loading";
    stampsError = "";
    try {
      stamps = await getCommentStamps();
      stampsStatus = "ready";
    } catch (error) {
      stampsError = describeDataFailure(error);
      stampsStatus = "error";
    }
  }

  async function insertEmoji(token: CommentEmojiToken) {
    const start = textarea?.selectionStart ?? text.length;
    const end = textarea?.selectionEnd ?? start;
    const inserted = insertCommentEmoji(text, token, start, end);
    text = inserted.value;
    await tick();
    textarea?.focus();
    textarea?.setSelectionRange(inserted.cursor, inserted.cursor);
  }
</script>

<form class="composer" onsubmit={submit}>
  <textarea
    bind:this={textarea}
    bind:value={text}
    maxlength="140"
    rows="3"
    {placeholder}
    aria-label={m.comment_content_label()}
  ></textarea>
  <div class="composer-foot">
    <div class="picker-buttons"><button
      class:active={pickerOpen && pickerKind === "emoji"}
      class="emoji-toggle"
      type="button"
      aria-expanded={pickerOpen}
      onclick={() => { pickerOpen = !(pickerOpen && pickerKind === "emoji"); pickerKind = "emoji"; }}
    >{m.comment_emoji()}</button><button
      class:active={pickerOpen && pickerKind === "stamp"}
      class="emoji-toggle"
      type="button"
      aria-expanded={pickerOpen && pickerKind === "stamp"}
      onclick={openStampPicker}
    >{m.comment_official_stamps()}</button></div>
    <span>{Array.from(text).length} / 140</span>
    <button class="submit" type="submit" disabled={submitting || (!text.trim() && !selectedStampId)}>{submitting ? m.comment_publishing() : submitLabel}</button>
  </div>
  {#if selectedStampId}
    <div class="selected-stamp">
      <PixivImage url={stamps.find((stamp) => stamp.id === selectedStampId)?.url} alt={m.comment_stamp_label({ id: selectedStampId })} fit="contain" />
      <button type="button" onclick={() => (selectedStampId = null)}>{m.common_remove()}</button>
    </div>
  {/if}
  {#if pickerOpen && pickerKind === "emoji"}
    <div class="emoji-picker" aria-label={m.comment_emoji_label()}>
      {#each COMMENT_EMOJIS as emoji (emoji.token)}
        <button type="button" title={emoji.token} aria-label={emoji.token} onclick={() => insertEmoji(emoji.token)}>
          <PixivImage url={emoji.url} alt="" fit="contain" />
        </button>
      {/each}
    </div>
  {:else if pickerOpen}
    <div class="emoji-picker" aria-label={m.comment_official_stamps()}>
      {#if stampsStatus === "loading"}<p>{m.common_loading()}</p>
      {:else if stampsStatus === "error"}<p class="inline-error">{stampsError}</p>
      {:else}
        {#each stamps as stamp (stamp.id)}
          <button class:selected={selectedStampId === stamp.id} type="button" aria-label={m.comment_stamp_label({ id: stamp.id })} onclick={() => (selectedStampId = selectedStampId === stamp.id ? null : stamp.id)}>
            <PixivImage url={stamp.url} alt="" fit="contain" />
          </button>
        {/each}
      {/if}
    </div>
  {/if}
  {#if errorMessage}<p class="inline-error" role="alert">{errorMessage}</p>{/if}
</form>

<style>
  .composer { padding: 15px; border: 1px solid var(--line); border-radius: 10px; background: white; }
  textarea { box-sizing: border-box; width: 100%; resize: vertical; padding: 11px; color: var(--text); border: 1px solid #dedede; border-radius: 7px; outline: none; font: inherit; font-size: var(--type-small); line-height: 1.6; }
  textarea:focus { border-color: #75c7f5; box-shadow: 0 0 0 3px rgba(0,150,250,.1); }
  .composer-foot { display: grid; grid-template-columns: auto 1fr auto; gap: 10px; align-items: center; margin-top: 9px; color: var(--muted); font-size: var(--type-caption); }
  .picker-buttons { display: flex; gap: 6px; flex-wrap: wrap; }
  .composer-foot > span { justify-self: end; }
  button { font: inherit; }
  .emoji-toggle { height: 30px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #bde1f7; border-radius: 15px; background: #f5fbff; cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .emoji-toggle.active { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .submit { min-width: 76px; height: 34px; color: white; border: 0; border-radius: 17px; background: var(--pixiv-blue); cursor: pointer; font-size: var(--type-body); font-weight: 700; }
  .submit:disabled { cursor: wait; opacity: .58; }
  .emoji-picker { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); gap: 5px; max-height: 196px; overflow-y: auto; margin-top: 11px; padding: 9px; border: 1px solid var(--line); border-radius: 8px; background: #fafafa; }
  .emoji-picker button { display: grid; width: 38px; height: 38px; padding: 5px; place-self: center; place-items: center; border: 0; border-radius: 7px; background: transparent; cursor: pointer; }
  .emoji-picker button:hover, .emoji-picker button:focus-visible { background: #e8f6ff; outline: none; }
  .emoji-picker button.selected { background: #dff3ff; box-shadow: inset 0 0 0 2px var(--pixiv-blue); }
  .selected-stamp { display: flex; gap: 10px; align-items: center; margin-top: 10px; }
  .selected-stamp :global(img) { width: 52px; height: 52px; }
  .selected-stamp button { color: #667078; border: 0; background: transparent; cursor: pointer; font-size: var(--type-body); }
  .inline-error { margin: 8px 0 0; color: #a44f5e; font-size: var(--type-caption); }
  @media (max-width: 620px) { .composer { padding: 12px; } }
</style>
