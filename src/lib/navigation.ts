import type { IconName } from "$lib/components/Icon.svelte";

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
  label: string;
  items: readonly NavigationKey[];
};

const items: Record<NavigationKey, NavigationItem> = {
  home: { key: "home", label: "首页", compactLabel: "首页", href: "/", icon: "home" },
  artworks: {
    key: "artworks",
    label: "插画",
    compactLabel: "插画",
    href: "/artworks",
    icon: "image",
  },
  manga: { key: "manga", label: "漫画", compactLabel: "漫画", href: "/manga", icon: "book" },
  novels: {
    key: "novels",
    label: "小说",
    compactLabel: "小说",
    href: "/novels",
    icon: "book",
  },
  following: {
    key: "following",
    label: "关注用户的新作",
    compactLabel: "新作",
    href: "/following",
    icon: "user",
  },
  discover: {
    key: "discover",
    label: "发现",
    compactLabel: "发现",
    href: "/discover",
    icon: "compass",
  },
  ranking: {
    key: "ranking",
    label: "排行榜",
    compactLabel: "排行",
    href: "/ranking",
    icon: "ranking",
  },
  bookmarks: {
    key: "bookmarks",
    label: "收藏",
    compactLabel: "收藏",
    href: "/bookmarks",
    icon: "heart",
  },
  offline: {
    key: "offline",
    label: "离线资料库",
    compactLabel: "离线",
    href: "/offline",
    icon: "download",
  },
  history: {
    key: "history",
    label: "浏览历史",
    compactLabel: "历史",
    href: "/history",
    icon: "history",
  },
  search: {
    key: "search",
    label: "搜索",
    compactLabel: "搜索",
    href: "/search",
    icon: "search",
  },
  notifications: {
    key: "notifications",
    label: "通知",
    compactLabel: "通知",
    href: "/notifications",
    icon: "bell",
  },
  profile: {
    key: "profile",
    label: "个人主页",
    compactLabel: "我的",
    href: "/profile",
    icon: "user",
  },
  settings: {
    key: "settings",
    label: "设置",
    compactLabel: "设置",
    href: "/settings",
    icon: "settings",
  },
};

export const sideNavigationSections: readonly NavigationSection[] = [
  {
    key: "content",
    label: "浏览作品",
    items: ["home", "artworks", "manga", "novels"],
  },
  {
    key: "discovery",
    label: "发现与收藏",
    items: ["following", "discover", "ranking", "bookmarks", "history", "offline"],
  },
];

export const contentTabKeys: readonly NavigationKey[] = [
  "home",
  "artworks",
  "manga",
  "novels",
];

// “新作”与侧栏的“关注用户的新作”是同一个入口，故复用 following。
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
  return items[key];
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
