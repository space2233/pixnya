import type { SearchTarget } from "./types.ts";

export const SEARCH_TYPES = ["works", "novels", "users", "tags"] as const;
export type SearchType = (typeof SEARCH_TYPES)[number];
export type SearchMatchTarget = "partial_match_for_tags" | "title_and_caption";

export interface SearchRouteState {
  query: string;
  type: SearchType;
  target: SearchMatchTarget;
}

export function readSearchRouteState(params: URLSearchParams): SearchRouteState {
  const candidateType = params.get("type");
  const type = SEARCH_TYPES.includes(candidateType as SearchType)
    ? candidateType as SearchType
    : "works";
  const candidateTarget = params.get("target");
  const target = supportsMatchTarget(type) && candidateTarget === "title_and_caption"
    ? "title_and_caption"
    : "partial_match_for_tags";
  return { query: params.get("q")?.trim() ?? "", type, target };
}

export function searchRouteHref(state: SearchRouteState): string {
  const params = new URLSearchParams();
  const query = state.query.trim();
  if (query) params.set("q", query);
  if (state.type !== "works") params.set("type", state.type);
  if (supportsMatchTarget(state.type) && state.target === "title_and_caption") {
    params.set("target", "title_and_caption");
  }
  const queryString = params.toString();
  return queryString ? `/search?${queryString}` : "/search";
}

export function effectiveSearchTarget(state: SearchRouteState): SearchTarget | null {
  if (state.type === "users") return null;
  if (state.type === "tags") return "exact_match_for_tags";
  return state.target;
}

export function searchRequestKey(sessionKey: string, state: SearchRouteState): string {
  if (!sessionKey || !state.query) return "";
  return JSON.stringify([
    sessionKey,
    state.type,
    effectiveSearchTarget(state),
    state.query,
  ]);
}

export function supportsMatchTarget(type: SearchType): boolean {
  return type === "works" || type === "novels";
}
