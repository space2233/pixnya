<script lang="ts">
  import { tick } from "svelte";
  import {
    adjacentViewerPage,
    panViewer,
    pinchViewer,
    RESET_VIEWER_TRANSFORM,
    zoomViewerAt,
    type ViewerPoint,
    type ViewerTransform,
  } from "$lib/image-viewer";
  import OfflineImage from "$lib/components/OfflineImage.svelte";
  import PixivImage from "$lib/components/PixivImage.svelte";
  import { m } from "$lib/i18n";

  export interface ArtworkViewerPage {
    pageIndex: number;
    alt: string;
    previewUrl?: string | null;
    originalUrl?: string | null;
    entryKey?: string;
    assetNames?: string[];
  }

  type Gesture =
    | { kind: "pan"; pointerId: number; start: ViewerPoint; transform: ViewerTransform }
    | { kind: "pinch"; startA: ViewerPoint; startB: ViewerPoint; transform: ViewerTransform };

  let {
    pages,
    title,
    concealed = false,
  }: {
    pages: ArtworkViewerPage[];
    title: string;
    concealed?: boolean;
  } = $props();

  let open = $state(false);
  let currentIndex = $state(0);
  let transform = $state<ViewerTransform>({ ...RESET_VIEWER_TRANSFORM });
  let dialog = $state<HTMLDivElement | null>(null);
  let viewport = $state<HTMLDivElement | null>(null);
  const activePointers = new Map<number, ViewerPoint>();
  let gesture: Gesture | null = null;
  let ownsHistoryEntry = false;
  let lastTouchTap: { at: number; point: ViewerPoint } | null = null;
  let pointerMoved = false;
  let currentPage = $derived(pages[currentIndex]);
  let zoomPercent = $derived(Math.round(transform.scale * 100));

  $effect(() => {
    if (!open) return;
    const previousOverflow = document.body.style.overflow;
    const handleHistoryBack = () => {
      ownsHistoryEntry = false;
      open = false;
    };
    document.body.style.overflow = "hidden";
    window.addEventListener("popstate", handleHistoryBack);
    void tick().then(() => dialog?.focus());
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("popstate", handleHistoryBack);
      activePointers.clear();
      gesture = null;
    };
  });

  $effect(() => {
    if (currentIndex >= pages.length) currentIndex = Math.max(0, pages.length - 1);
  });

  function viewportSize() {
    const bounds = viewport?.getBoundingClientRect();
    return { width: bounds?.width ?? 0, height: bounds?.height ?? 0 };
  }

  function pointInViewport(clientX: number, clientY: number): ViewerPoint {
    const bounds = viewport?.getBoundingClientRect();
    return {
      x: clientX - (bounds?.left ?? 0) - (bounds?.width ?? 0) / 2,
      y: clientY - (bounds?.top ?? 0) - (bounds?.height ?? 0) / 2,
    };
  }

  function openViewer(index: number) {
    if (concealed || !pages[index]) return;
    currentIndex = index;
    transform = { ...RESET_VIEWER_TRANSFORM };
    window.history.pushState({ ...window.history.state, pixivImageViewer: true }, "", window.location.href);
    ownsHistoryEntry = true;
    open = true;
  }

  function closeViewer() {
    if (ownsHistoryEntry) {
      window.history.back();
    } else {
      open = false;
    }
  }

  function changePage(delta: -1 | 1) {
    const next = adjacentViewerPage(currentIndex, pages.length, delta);
    if (next === currentIndex) return;
    currentIndex = next;
    transform = { ...RESET_VIEWER_TRANSFORM };
  }

  function setZoom(scale: number, anchor: ViewerPoint = { x: 0, y: 0 }) {
    transform = zoomViewerAt(transform, scale, anchor, viewportSize());
  }

  function toggleZoom(anchor: ViewerPoint) {
    setZoom(transform.scale > 1 ? 1 : 2.5, anchor);
  }

  function handleWheel(event: WheelEvent) {
    event.preventDefault();
    const factor = event.deltaY < 0 ? 1.16 : 1 / 1.16;
    setZoom(transform.scale * factor, pointInViewport(event.clientX, event.clientY));
  }

  function beginGesture(event: PointerEvent) {
    if (event.button !== 0) return;
    viewport?.setPointerCapture(event.pointerId);
    const point = pointInViewport(event.clientX, event.clientY);
    activePointers.set(event.pointerId, point);
    pointerMoved = false;
    const points = [...activePointers.entries()];
    if (points.length >= 2) {
      gesture = { kind: "pinch", startA: points[0][1], startB: points[1][1], transform: { ...transform } };
    } else {
      gesture = { kind: "pan", pointerId: event.pointerId, start: point, transform: { ...transform } };
    }
  }

  function moveGesture(event: PointerEvent) {
    if (!activePointers.has(event.pointerId) || !gesture) return;
    const point = pointInViewport(event.clientX, event.clientY);
    const previous = activePointers.get(event.pointerId);
    activePointers.set(event.pointerId, point);
    if (previous && Math.hypot(point.x - previous.x, point.y - previous.y) > 2) pointerMoved = true;

    if (gesture.kind === "pan") {
      if (gesture.pointerId !== event.pointerId || transform.scale <= 1) return;
      transform = panViewer(
        gesture.transform,
        { x: point.x - gesture.start.x, y: point.y - gesture.start.y },
        viewportSize(),
      );
      return;
    }

    const points = [...activePointers.values()];
    if (points.length < 2) return;
    const startDistance = Math.hypot(
      gesture.startB.x - gesture.startA.x,
      gesture.startB.y - gesture.startA.y,
    );
    const currentDistance = Math.hypot(points[1].x - points[0].x, points[1].y - points[0].y);
    if (startDistance < 1) return;
    const startMidpoint = {
      x: (gesture.startA.x + gesture.startB.x) / 2,
      y: (gesture.startA.y + gesture.startB.y) / 2,
    };
    const currentMidpoint = {
      x: (points[0].x + points[1].x) / 2,
      y: (points[0].y + points[1].y) / 2,
    };
    transform = pinchViewer(
      gesture.transform,
      startMidpoint,
      currentMidpoint,
      currentDistance / startDistance,
      viewportSize(),
    );
  }

  function endGesture(event: PointerEvent) {
    if (!activePointers.has(event.pointerId)) return;
    const point = activePointers.get(event.pointerId) ?? { x: 0, y: 0 };
    activePointers.delete(event.pointerId);
    if (event.pointerType !== "mouse" && !pointerMoved) {
      const now = Date.now();
      if (lastTouchTap && now - lastTouchTap.at < 320 && Math.hypot(point.x - lastTouchTap.point.x, point.y - lastTouchTap.point.y) < 28) {
        toggleZoom(point);
        lastTouchTap = null;
      } else {
        lastTouchTap = { at: now, point };
      }
    }
    const remaining = [...activePointers.entries()];
    gesture = remaining.length === 1
      ? { kind: "pan", pointerId: remaining[0][0], start: remaining[0][1], transform: { ...transform } }
      : null;
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") closeViewer();
    else if (event.key === "ArrowLeft") changePage(-1);
    else if (event.key === "ArrowRight") changePage(1);
    else if (event.key === "+" || event.key === "=") setZoom(transform.scale * 1.25);
    else if (event.key === "-") setZoom(transform.scale / 1.25);
    else if (event.key === "0") transform = { ...RESET_VIEWER_TRANSFORM };
    else return;
    event.preventDefault();
  }
</script>

<div class="artwork-gallery-preview" class:concealed aria-hidden={concealed}>
  {#each pages as image, index (image.pageIndex)}
    <figure>
      <button type="button" aria-label={m.viewer_open_original_label({ alt: image.alt })} onclick={() => openViewer(index)} disabled={concealed}>
        {#if image.entryKey && image.assetNames}
          <OfflineImage entryKey={image.entryKey} assetNames={image.assetNames} alt={image.alt} />
        {:else}
          <PixivImage url={image.previewUrl ?? image.originalUrl} alt={image.alt} fit="contain" cacheKind="preview" />
        {/if}
        <span class="open-hint">{m.viewer_open_original()}</span>
        {#if pages.length > 1}<span class="page-count">{index + 1} / {pages.length}</span>{/if}
      </button>
    </figure>
  {/each}
</div>

{#if open && currentPage}
  <div
    class="viewer"
    role="dialog"
    aria-modal="true"
    aria-label={m.viewer_dialog_label({ title })}
    tabindex="-1"
    bind:this={dialog}
    onkeydown={handleKeydown}
  >
    <header>
      <div>
        <strong>{title || m.common_untitled()}</strong>
        <span aria-live="polite">{currentIndex + 1} / {pages.length} · {zoomPercent}%</span>
      </div>
      <button type="button" aria-label={m.viewer_close()} onclick={closeViewer}>×</button>
    </header>

    <div
      class="viewport"
      class:zoomed={transform.scale > 1}
      role="group"
      aria-label={m.viewer_zoomable_region()}
      bind:this={viewport}
      onwheel={handleWheel}
      onpointerdown={beginGesture}
      onpointermove={moveGesture}
      onpointerup={endGesture}
      onpointercancel={endGesture}
      ondblclick={(event) => toggleZoom(pointInViewport(event.clientX, event.clientY))}
    >
      <div class="image-stage" style:transform={`translate3d(${transform.x}px, ${transform.y}px, 0) scale(${transform.scale})`}>
        {#if currentPage.entryKey && currentPage.assetNames}
          <OfflineImage entryKey={currentPage.entryKey} assetNames={currentPage.assetNames} alt={currentPage.alt} />
        {:else}
          <PixivImage
            url={currentPage.originalUrl ?? currentPage.previewUrl}
            alt={currentPage.alt}
            fit="contain"
            cacheKind="original"
          />
        {/if}
      </div>
    </div>

    {#if pages.length > 1}
      <button class="page-nav previous" type="button" aria-label={m.viewer_previous()} disabled={currentIndex === 0} onclick={() => changePage(-1)}>‹</button>
      <button class="page-nav next" type="button" aria-label={m.viewer_next()} disabled={currentIndex === pages.length - 1} onclick={() => changePage(1)}>›</button>
    {/if}

    <footer aria-label={m.viewer_zoom_controls()}>
      <button type="button" aria-label={m.viewer_zoom_out()} disabled={transform.scale <= 1} onclick={() => setZoom(transform.scale / 1.25)}>−</button>
      <button class="zoom-value" type="button" aria-label={m.viewer_reset_zoom()} onclick={() => (transform = { ...RESET_VIEWER_TRANSFORM })}>{zoomPercent}%</button>
      <button type="button" aria-label={m.viewer_zoom_in()} disabled={transform.scale >= 6} onclick={() => setZoom(transform.scale * 1.25)}>＋</button>
    </footer>
  </div>
{/if}

<style>
  .artwork-gallery-preview { display: grid; gap: 16px; }
  figure { min-width: 0; margin: 0; }
  figure > button {
    position: relative; display: block; width: 100%; min-height: 360px; overflow: hidden;
    padding: 0; border: 0; border-radius: 10px; background: #f3f4f5; cursor: zoom-in;
  }
  figure > button :global(img) { max-height: 82vh; }
  .open-hint, .page-count {
    position: absolute; bottom: 10px; padding: 5px 9px; color: white; border-radius: 13px;
    background: rgba(20, 22, 24, .66); font-size: 9px; font-weight: 700;
  }
  .open-hint { left: 10px; opacity: 0; transform: translateY(4px); transition: opacity .16s ease, transform .16s ease; }
  .page-count { right: 10px; }
  figure > button:hover .open-hint, figure > button:focus-visible .open-hint { opacity: 1; transform: none; }
  .concealed { filter: blur(24px) brightness(.68); pointer-events: none; }

  .viewer {
    position: fixed; z-index: 300; inset: 0; overflow: hidden; color: white; outline: none;
    background: rgba(12, 14, 16, .96); overscroll-behavior: contain;
  }
  .viewer header {
    position: absolute; z-index: 4; top: 0; right: 0; left: 0; display: flex; min-height: 64px;
    align-items: center; justify-content: space-between; padding: max(10px, env(safe-area-inset-top)) 18px 10px;
    background: linear-gradient(rgba(0, 0, 0, .64), transparent);
  }
  .viewer header div { min-width: 0; }
  .viewer header strong, .viewer header span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .viewer header strong { max-width: min(70vw, 720px); font-size: 12px; }
  .viewer header span { margin-top: 4px; color: #c5c9cc; font-size: 9px; }
  .viewer header button, .viewer footer button, .page-nav {
    display: grid; place-items: center; color: white; border: 1px solid rgba(255, 255, 255, .18);
    background: rgba(30, 33, 36, .72); cursor: pointer;
  }
  .viewer header button { width: 42px; height: 42px; border-radius: 50%; font-size: 27px; line-height: 1; }
  .viewport { position: absolute; inset: 0; overflow: hidden; cursor: zoom-in; touch-action: none; user-select: none; }
  .viewport.zoomed { cursor: grab; }
  .viewport.zoomed:active { cursor: grabbing; }
  .image-stage { width: 100%; height: 100%; transform-origin: center; will-change: transform; }
  .image-stage :global(img) { width: 100%; height: 100%; object-fit: contain; }
  .page-nav { position: absolute; z-index: 4; top: 50%; width: 48px; height: 64px; border-radius: 9px; font-size: 36px; transform: translateY(-50%); }
  .page-nav.previous { left: 16px; }
  .page-nav.next { right: 16px; }
  .page-nav:disabled, .viewer footer button:disabled { cursor: default; opacity: .28; }
  .viewer footer {
    position: absolute; z-index: 4; right: 50%; bottom: max(18px, env(safe-area-inset-bottom)); display: flex;
    gap: 6px; padding: 6px; border-radius: 24px; background: rgba(0, 0, 0, .42); transform: translateX(50%);
  }
  .viewer footer button { width: 38px; height: 38px; border-radius: 50%; font-size: 20px; }
  .viewer footer .zoom-value { width: 66px; border-radius: 19px; font-size: 10px; }

  @media (max-width: 620px) {
    figure > button { min-height: 260px; border-radius: 7px; }
    .open-hint { opacity: 1; transform: none; }
    .viewer header { min-height: 58px; padding-right: 12px; padding-left: 12px; }
    .viewer header strong { max-width: 68vw; font-size: 11px; }
    .page-nav { top: auto; bottom: max(78px, calc(64px + env(safe-area-inset-bottom))); width: 42px; height: 42px; border-radius: 50%; font-size: 28px; transform: none; }
    .page-nav.previous { left: 12px; }
    .page-nav.next { right: 12px; }
  }

  @media (prefers-reduced-motion: reduce) {
    .open-hint { transition: none; }
  }
</style>
