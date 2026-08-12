const SETUP_PATH = "/setup/connection";

export function connectionSetupUrl(returnTarget: string): string {
  const safeTarget = safeConnectionReturnTarget(returnTarget);
  return `${SETUP_PATH}?returnTo=${encodeURIComponent(safeTarget)}`;
}

export function safeConnectionReturnTarget(value: string | null | undefined): string {
  if (!value || !value.startsWith("/") || value.startsWith("//")) return "/";
  try {
    const parsed = new URL(value, "https://pixnya.local");
    if (parsed.origin !== "https://pixnya.local" || parsed.pathname === SETUP_PATH) return "/";
    return `${parsed.pathname}${parsed.search}${parsed.hash}`;
  } catch {
    return "/";
  }
}
