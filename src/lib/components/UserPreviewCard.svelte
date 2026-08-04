<script lang="ts">
  import ArtworkThumbnail from "$lib/components/ArtworkThumbnail.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { r18DefaultVisible } from "$lib/preferences";
  import type { UserPreview } from "$lib/types";

  let { preview }: { preview: UserPreview } = $props();
  let revealRestricted = $state(false);

  function initial(name: string): string {
    return Array.from(name.trim())[0]?.toUpperCase() ?? "P";
  }
</script>

<article class="user-preview">
  <a class="user-heading" href={`/users/${preview.user.id}`}>
    <span class="avatar">
      <b>{initial(preview.user.name)}</b>
      <PixivImage url={preview.user.avatarUrl} alt="" />
    </span>
    <span class="user-copy">
      <strong>{preview.user.name || preview.user.account}</strong>
      <small>@{preview.user.account}</small>
    </span>
    <i class:active={preview.user.isFollowed}>{preview.user.isFollowed ? "已关注" : "查看主页"}</i>
  </a>

  {#if preview.illustrations.length > 0}
    <div class="preview-works">
      {#each preview.illustrations as illustration, index (illustration.id)}
        <div class="preview-work" class:concealed={illustration.xRestrict > 0 && !$r18DefaultVisible && !revealRestricted}>
          <a href={`/artworks/${illustration.id}`} aria-label={`查看作品：${illustration.title || "无题"}`}></a>
          <ArtworkThumbnail
            url={illustration.thumbnailUrl}
            alt={illustration.xRestrict > 0 && !$r18DefaultVisible && !revealRestricted ? "受限内容缩略图已模糊" : illustration.title || "无题作品"}
            tone={(index % 6) + 1}
          />
          {#if illustration.xRestrict > 0 && !$r18DefaultVisible && !revealRestricted}
            <button type="button" onclick={() => (revealRestricted = true)}>R18</button>
          {/if}
        </div>
      {/each}
    </div>
  {:else}
    <p class="no-preview">没有可预览的公开作品</p>
  {/if}
</article>

<style>
  .user-preview { overflow: hidden; border: 1px solid var(--line); border-radius: 10px; background: white; }
  .user-heading { display: grid; grid-template-columns: 46px minmax(0, 1fr) auto; gap: 11px; align-items: center; padding: 14px; color: var(--text); text-decoration: none; }
  .avatar { position: relative; display: grid; width: 46px; height: 46px; overflow: hidden; place-items: center; color: white; border-radius: 50%; background: var(--pixiv-blue); }
  .avatar :global(img) { position: absolute; z-index: 1; inset: 0; }
  .user-copy { min-width: 0; }
  .user-copy strong, .user-copy small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .user-copy strong { font-size: 11px; }
  .user-copy small { margin-top: 4px; color: var(--muted); font-size: 8px; }
  .user-heading i { color: var(--pixiv-blue); font-size: 8px; font-style: normal; font-weight: 700; }
  .user-heading i.active { color: #78838a; }
  .preview-works { display: grid; grid-template-columns: repeat(3, 1fr); gap: 2px; background: #f1f2f3; }
  .preview-work { position: relative; display: block; overflow: hidden; aspect-ratio: 1; }
  .preview-work > a { position: absolute; z-index: 1; inset: 0; }
  .preview-work.concealed :global(img) { filter: blur(18px) brightness(.7); transform: scale(1.12); }
  .preview-work button { position: absolute; z-index: 2; inset: 0; width: 100%; color: white; border: 0; background: rgba(20,24,28,.38); cursor: pointer; font-size: 8px; font-weight: 700; }
  .no-preview { margin: 0; padding: 24px 14px; color: var(--muted); border-top: 1px solid var(--line); font-size: 9px; text-align: center; }
</style>
