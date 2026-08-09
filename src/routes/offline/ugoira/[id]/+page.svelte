<script lang="ts">
  import { page } from "$app/state";
  import AppShell from "$lib/components/AppShell.svelte";
  import OfflineUgoiraPlayer from "$lib/components/OfflineUgoiraPlayer.svelte";
  import ReturnLink from "$lib/components/ReturnLink.svelte";
  import { m } from "$lib/i18n";
  import { readOfflineText } from "$lib/pixiv-api";
  import type { UgoiraMetadata } from "$lib/types";
  let metadata=$state<UgoiraMetadata|null>(null);let errorMessage=$state("");let id=$derived(page.params.id??"");let key=$derived(`ugoira-${id}`);
  $effect(()=>{if(id)void load(id)});async function load(expectedId:string){metadata=null;errorMessage="";try{const parsed=JSON.parse(await readOfflineText(`ugoira-${expectedId}`,"metadata.json")) as UgoiraMetadata;if(!Array.isArray(parsed?.frames)||!parsed.frames.length)throw new Error("invalid");metadata=parsed}catch{errorMessage=m.offline_ugoira_error()}}
</script>
<svelte:head><title>{m.offline_ugoira_title()} · PixNya</title></svelte:head><AppShell title={m.offline_ugoira_title()}><main><ReturnLink fallback="/offline" label={m.offline_back_library()} />{#if errorMessage}<section role="alert">{errorMessage}</section>{:else if !metadata}<section>{m.offline_ugoira_loading()}</section>{:else}<OfflineUgoiraPlayer entryKey={key} {metadata} title={`Ugoira ${id}`} />{/if}</main></AppShell><style>main{width:min(900px,100%);margin:0 auto;padding:24px 28px 70px}section{display:grid;min-height:180px;margin-top:18px;place-items:center;color:var(--muted);border:1px dashed var(--line);border-radius:10px}@media(max-width:620px){main{padding:16px 10px 90px}}</style>
