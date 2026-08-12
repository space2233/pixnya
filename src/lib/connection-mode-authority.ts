import type { SessionSnapshot } from "$lib/types";

export function sameConnectionModeAuthority(
  before: SessionSnapshot,
  after: SessionSnapshot,
): boolean {
  if (before.loggedIn !== after.loggedIn) return false;
  if (!before.loggedIn) return true;
  return (
    before.user?.id === after.user?.id &&
    before.connectionMode === after.connectionMode
  );
}
