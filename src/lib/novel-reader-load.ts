export const NOVEL_READER_LOAD_ATTEMPTS = 3;
export const NOVEL_READER_RETRY_DELAY_MS = 400;

const NON_RETRYABLE_KINDS = new Set([
  "authentication_required",
  "invalid_identifier",
  "invalid_input",
]);

export function novelReaderFailureKind(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "kind" in error) {
    const kind = (error as { kind?: unknown }).kind;
    return typeof kind === "string" ? kind : undefined;
  }
  return undefined;
}

export function shouldRetryNovelReaderLoad(error: unknown, failedAttempt: number): boolean {
  if (failedAttempt >= NOVEL_READER_LOAD_ATTEMPTS) return false;
  const kind = novelReaderFailureKind(error);
  return kind === undefined || !NON_RETRYABLE_KINDS.has(kind);
}
