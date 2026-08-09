<script lang="ts">
  import { tick } from "svelte";
  import { COMMENT_EMOJIS, insertCommentEmoji, type CommentEmojiToken } from "$lib/comment-emoji";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";

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
    onsubmit: (text: string) => Promise<boolean>;
  } = $props();

  let text = $state("");
  let pickerOpen = $state(false);
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
    if (!value || submitting) return;
    if (await onsubmit(value)) {
      text = "";
      pickerOpen = false;
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
    <button
      class:active={pickerOpen}
      class="emoji-toggle"
      type="button"
      aria-expanded={pickerOpen}
      onclick={() => (pickerOpen = !pickerOpen)}
    >{m.comment_emoji()}</button>
    <span>{Array.from(text).length} / 140</span>
    <button class="submit" type="submit" disabled={submitting || !text.trim()}>{submitting ? m.comment_publishing() : submitLabel}</button>
  </div>
  {#if pickerOpen}
    <div class="emoji-picker" aria-label={m.comment_emoji_label()}>
      {#each COMMENT_EMOJIS as emoji (emoji.token)}
        <button type="button" title={emoji.token} aria-label={emoji.token} onclick={() => insertEmoji(emoji.token)}>
          <PixivImage url={emoji.url} alt="" fit="contain" />
        </button>
      {/each}
    </div>
  {/if}
  {#if errorMessage}<p class="inline-error" role="alert">{errorMessage}</p>{/if}
</form>

<style>
  .composer { padding: 15px; border: 1px solid var(--line); border-radius: 10px; background: white; }
  textarea { box-sizing: border-box; width: 100%; resize: vertical; padding: 11px; color: var(--text); border: 1px solid #dedede; border-radius: 7px; outline: none; font: inherit; font-size: 10px; line-height: 1.6; }
  textarea:focus { border-color: #75c7f5; box-shadow: 0 0 0 3px rgba(0,150,250,.1); }
  .composer-foot { display: grid; grid-template-columns: auto 1fr auto; gap: 10px; align-items: center; margin-top: 9px; color: var(--muted); font-size: 8px; }
  .composer-foot > span { justify-self: end; }
  button { font: inherit; }
  .emoji-toggle { height: 30px; padding: 0 13px; color: var(--pixiv-blue); border: 1px solid #bde1f7; border-radius: 15px; background: #f5fbff; cursor: pointer; font-size: 9px; font-weight: 700; }
  .emoji-toggle.active { color: white; border-color: var(--pixiv-blue); background: var(--pixiv-blue); }
  .submit { min-width: 76px; height: 34px; color: white; border: 0; border-radius: 17px; background: var(--pixiv-blue); cursor: pointer; font-size: 9px; font-weight: 700; }
  .submit:disabled { cursor: wait; opacity: .58; }
  .emoji-picker { display: grid; grid-template-columns: repeat(auto-fill, minmax(38px, 1fr)); gap: 5px; max-height: 196px; overflow-y: auto; margin-top: 11px; padding: 9px; border: 1px solid var(--line); border-radius: 8px; background: #fafafa; }
  .emoji-picker button { display: grid; width: 38px; height: 38px; padding: 5px; place-self: center; place-items: center; border: 0; border-radius: 7px; background: transparent; cursor: pointer; }
  .emoji-picker button:hover, .emoji-picker button:focus-visible { background: #e8f6ff; outline: none; }
  .inline-error { margin: 8px 0 0; color: #a44f5e; font-size: 8px; }
  @media (max-width: 620px) { .composer { padding: 12px; } }
</style>
