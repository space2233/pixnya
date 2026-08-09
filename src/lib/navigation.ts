import type { IconName } from "$lib/components/Icon.svelte";
import { m } from "./i18n.ts";

export type NavigationKey =
  | "home"
  | "artworks"
  | "manga"
  | "novels"
  | "following"
  | "discover"
  | "ranking"
  | "bookmarks"
  | "offline"
  | "history"
  | "search"
  | "notifications"
  | "profile"
  | "settings";

export type NavigationItem = {
  key: NavigationKey;
  label: string;
  compactLabel: string;
  href: string;
  icon: IconName;
};

export type NavigationSection = {
  key: "content" | "discovery";
  label: () => string;
  items: readonly NavigationKey[];
};

type NavigationDefinition = Omit<NavigationItem, "label" | "compactLabel"> & {
  label: () => string;
  compactLabel: () => string;
};

const items: Record<NavigationKey, NavigationDefinition> = {
  home: { key: "home", label: m.navigation_home, compactLabel: m.navigation_home, href: "/", icon: "home" },
  artworks: {
    key: "artworks",
    label: m.navigation_artworks,
    compactLabel: m.navigation_artworks,
    href: "/artworks",
    icon: "image",
  },
  manga: { key: "manga", label: m.navigation_manga, compactLabel: m.navigation_manga, href: "/manga", icon: "book" },
  novels: {
    key: "novels",
    label: m.navigation_novels,
    compactLabel: m.navigation_novels,
    href: "/novels",
    icon: "book",
  },
  following: {
    key: "following",
    label: m.navigation_following,
    compactLabel: m.navigation_compact_following,
    href: "/following",
    icon: "user",
  },
  discover: {
    key: "discover",
    label: m.navigation_discover,
    compactLabel: m.navigation_discover,
    href: "/discover",
    icon: "compass",
  },
  ranking: {
    key: "ranking",
    label: m.navigation_ranking,
    compactLabel: m.navigation_compact_ranking,
    href: "/ranking",
    icon: "ranking",
  },
  bookmarks: {
    key: "bookmarks",
    label: m.navigation_bookmarks,
    compactLabel: m.navigation_bookmarks,
    href: "/bookmarks",
    icon: "heart",
  },
  offline: {
    key: "offline",
    label: m.navigation_offline,
    compactLabel: m.navigation_compact_offline,
    href: "/offline",
    icon: "download",
  },
  history: {
    key: "history",
    label: m.navigation_history,
    compactLabel: m.navigation_compact_history,
    href: "/history",
    icon: "history",
  },
  search: {
    key: "search",
    label: m.navigation_search,
    compactLabel: m.navigation_search,
    href: "/search",
    icon: "search",
  },
  notifications: {
    key: "notifications",
    label: m.navigation_notifications,
    compactLabel: m.navigation_notifications,
    href: "/notifications",
    icon: "bell",
  },
  profile: {
    key: "profile",
    label: m.navigation_profile,
    compactLabel: m.navigation_compact_profile,
    href: "/profile",
    icon: "user",
  },
  settings: {
    key: "settings",
    label: m.navigation_settings,
    compactLabel: m.navigation_settings,
    href: "/settings",
    icon: "settings",
  },
};

export const sideNavigationSections: readonly NavigationSection[] = [
  {
    key: "content",
    label: m.navigation_section_content,
    items: ["home", "artworks", "manga", "novels"],
  },
  {
    key: "discovery",
    label: m.navigation_section_discovery,
    items: ["following", "discover", "ranking", "bookmarks", "history", "offline"],
  },
];

export const contentTabKeys: readonly NavigationKey[] = [
  "home",
  "artworks",
  "manga",
  "novels",
];

// The compact "new works" item and the sidebar's followed-user feed share one route.
export const bottomNavigationKeys: readonly NavigationKey[] = [
  "home",
  "search",
  "following",
  "offline",
  "profile",
];

const routeAliases: ReadonlyArray<{ prefix: string; key: NavigationKey }> = [
  { prefix: "/login", key: "profile" },
];

export function getNavigationItem(key: NavigationKey): NavigationItem {
  const item = items[key];
  return {
    ...item,
    label: item.label(),
    compactLabel: item.compactLabel(),
  };
}

export function navigationKeyForPath(pathname: string): NavigationKey | null {
  const normalized = normalizePath(pathname);
  const alias = routeAliases.find(
    ({ prefix }) => normalized === prefix || normalized.startsWith(`${prefix}/`),
  );
  if (alias) return alias.key;

  for (const item of Object.values(items)) {
    if (item.href === "/") {
      if (normalized === "/") return item.key;
      continue;
    }

    if (normalized === item.href || normalized.startsWith(`${item.href}/`)) {
      return item.key;
    }
  }

  return null;
}

function normalizePath(pathname: string): string {
  const withLeadingSlash = pathname.startsWith("/") ? pathname : `/${pathname}`;
  if (withLeadingSlash.length === 1) return withLeadingSlash;
  return withLeadingSlash.replace(/\/+$/, "");
}
