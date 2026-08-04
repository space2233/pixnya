export const MEDIA_FALLBACK_REQUIRED_EVENT = "pixiv-client:media-fallback-required";
export const MEDIA_RETRY_EVENT = "pixiv-client:media-retry";

let fallbackPromptRequested = false;

export function requestInsecureMediaFallback(): void {
  if (typeof window === "undefined" || fallbackPromptRequested) return;
  fallbackPromptRequested = true;
  window.dispatchEvent(new CustomEvent(MEDIA_FALLBACK_REQUIRED_EVENT));
}

export function retryPixivMedia(): void {
  if (typeof window === "undefined") return;
  fallbackPromptRequested = false;
  window.dispatchEvent(new CustomEvent(MEDIA_RETRY_EVENT));
}

export function resetMediaFallbackPrompt(): void {
  fallbackPromptRequested = false;
}

export function commandFailureKind(error: unknown): string {
  return error && typeof error === "object" && "kind" in error
    ? String((error as { kind: unknown }).kind)
    : "";
}
