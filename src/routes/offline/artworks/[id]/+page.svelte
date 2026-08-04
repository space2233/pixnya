<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import ArtworkImageViewer from "$lib/components/ArtworkImageViewer.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { readOfflineText } from "$lib/pixiv-api";
  import { plainPixivText } from "$lib/pixiv-text";
  import type { IllustrationDetail } from "$lib/types";

  let detail = $state<IllustrationDetail | null>(null);
  let errorMessage = $state("");
  let id = $derived(page.params.id ?? "");
  let key = $derived(`artwork-${id}`);
  let caption = $derived(detail ? plainPixivText(detail.caption) : "");
  let viewerPages = $derived((detail?.pages ?? []).map((image) => ({
    pageIndex: image.pageIndex,
    alt: `${detail?.illustration.title || "无题"} 第 ${image.pageIndex + 1} 页`,
    entryKey: key,
    assetNames: candidates(image.pageIndex),
  })));
  $effect(() => { if (id) void load(id); });
  async function load(expectedId: string) {
    detail = null; errorMessage = "";
    try { const parsed = JSON.parse(await readOfflineText(`artwork-${expectedId}`, "detail.json")) as IllustrationDetail; if (parsed?.illustration?.id !== expectedId || !Array.isArray(parsed.pages)) throw new Error("invalid"); detail = parsed; }
    catch { errorMessage = "无法读取离线作品，缓存可能不完整。"; }
  }
  function candidates(index: number): string[] { const base = `page-${String(index + 1).padStart(4,"0")}`; return ["jpg","jpeg","png","webp","gif","avif"].map((extension) => `${base}.${extension}`); }
</script>
<svelte:head><title>{detail?.illustration.title || "离线作品"} · PixNya</title></svelte:head>
<AppShell title="离线作品"><main class="offline-work"><ReturnLink fallback="/offline" label="返回离线资料库" />{#if errorMessage}<section class="state" role="alert">{errorMessage}</section>{:else if !detail}<section class="state">正在读取本地作品…</section>{:else}<header><div><span>离线作品</span><h1>{detail.illustration.title || "无题"}</h1><p>{detail.illustration.author.name}</p>{#if caption}<div>{caption}</div>{/if}</div></header><section class="pages"><ArtworkImageViewer pages={viewerPages} title={detail.illustration.title || "无题"} /></section>{/if}</main></AppShell>
<style>
  .offline-work { width: min(980px,100%); margin: 0 auto; padding: 24px 28px 70px; } header { margin-top: 18px; padding: 20px; border: 1px solid var(--line); border-radius: 10px; background: white; } header span { color: var(--pixiv-blue); font-size: 8px; font-weight: 700; } h1 { margin: 8px 0 0; font-size: 21px; } header p { margin: 6px 0 0; color: var(--muted); font-size: 9px; } header div div { margin-top: 12px; color: #5d6367; font-size: 10px; line-height: 1.7; white-space: pre-line; } .pages { margin-top: 18px; } .state { display: grid; min-height: 180px; margin-top: 18px; place-items: center; color: var(--muted); border: 1px dashed var(--line); border-radius: 10px; } @media (max-width:620px) { .offline-work { padding: 16px 10px 90px; } }
</style>
