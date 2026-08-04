export const MIN_VIEWER_SCALE = 1;
export const MAX_VIEWER_SCALE = 6;

export interface ViewerPoint {
  x: number;
  y: number;
}

export interface ViewerSize {
  width: number;
  height: number;
}

export interface ViewerTransform extends ViewerPoint {
  scale: number;
}

export const RESET_VIEWER_TRANSFORM: ViewerTransform = Object.freeze({
  scale: MIN_VIEWER_SCALE,
  x: 0,
  y: 0,
});

export function clampViewerScale(scale: number): number {
  if (!Number.isFinite(scale)) return MIN_VIEWER_SCALE;
  return Math.min(MAX_VIEWER_SCALE, Math.max(MIN_VIEWER_SCALE, scale));
}

export function clampViewerTransform(
  transform: ViewerTransform,
  viewport: ViewerSize,
): ViewerTransform {
  const scale = clampViewerScale(transform.scale);
  if (scale === MIN_VIEWER_SCALE) return { ...RESET_VIEWER_TRANSFORM };

  const width = Math.max(0, viewport.width);
  const height = Math.max(0, viewport.height);
  const maxX = (width * (scale - 1)) / 2;
  const maxY = (height * (scale - 1)) / 2;
  return {
    scale,
    x: Math.min(maxX, Math.max(-maxX, transform.x)),
    y: Math.min(maxY, Math.max(-maxY, transform.y)),
  };
}

export function zoomViewerAt(
  transform: ViewerTransform,
  nextScale: number,
  anchor: ViewerPoint,
  viewport: ViewerSize,
): ViewerTransform {
  const scale = clampViewerScale(nextScale);
  if (scale === MIN_VIEWER_SCALE) return { ...RESET_VIEWER_TRANSFORM };
  const ratio = scale / transform.scale;
  return clampViewerTransform(
    {
      scale,
      x: anchor.x - (anchor.x - transform.x) * ratio,
      y: anchor.y - (anchor.y - transform.y) * ratio,
    },
    viewport,
  );
}

export function panViewer(
  transform: ViewerTransform,
  delta: ViewerPoint,
  viewport: ViewerSize,
): ViewerTransform {
  return clampViewerTransform(
    {
      ...transform,
      x: transform.x + delta.x,
      y: transform.y + delta.y,
    },
    viewport,
  );
}

export function pinchViewer(
  transform: ViewerTransform,
  startMidpoint: ViewerPoint,
  currentMidpoint: ViewerPoint,
  scaleRatio: number,
  viewport: ViewerSize,
): ViewerTransform {
  const scale = clampViewerScale(transform.scale * scaleRatio);
  if (scale === MIN_VIEWER_SCALE) return { ...RESET_VIEWER_TRANSFORM };
  const appliedRatio = scale / transform.scale;
  return clampViewerTransform(
    {
      scale,
      x: currentMidpoint.x - appliedRatio * (startMidpoint.x - transform.x),
      y: currentMidpoint.y - appliedRatio * (startMidpoint.y - transform.y),
    },
    viewport,
  );
}

export function adjacentViewerPage(current: number, count: number, delta: -1 | 1): number {
  if (count <= 0) return 0;
  return Math.min(count - 1, Math.max(0, current + delta));
}
