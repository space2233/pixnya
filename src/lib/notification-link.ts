export type NotificationLink =
  | { kind: "internal"; href: string }
  | { kind: "external"; href: string };

const numericId = /^\d+$/;

export function classifyNotificationLink(value: string | null | undefined): NotificationLink | null {
  if (!value) return null;
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return null;
  }
  if (
    url.protocol !== "https:" ||
    url.hostname !== "www.pixiv.net" ||
    url.username !== "" ||
    url.password !== ""
  ) {
    return null;
  }

  const parts = url.pathname.split("/").filter(Boolean);
  if (parts.length === 2 && numericId.test(parts[1])) {
    if (parts[0] === "artworks") return { kind: "internal", href: `/artworks/${parts[1]}` };
    if (parts[0] === "novels") return { kind: "internal", href: `/novels/${parts[1]}` };
    if (parts[0] === "users") return { kind: "internal", href: `/users/${parts[1]}` };
  }
  if (url.pathname === "/novel/show.php") {
    const id = url.searchParams.get("id") ?? "";
    if (numericId.test(id)) return { kind: "internal", href: `/novels/${id}` };
  }
  return { kind: "external", href: url.href };
}
