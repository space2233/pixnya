const namedEntities: Record<string, string> = {
  amp: "&",
  apos: "'",
  gt: ">",
  lt: "<",
  nbsp: " ",
  quot: '"',
};

export function plainPixivText(value: string): string {
  return value
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&([a-z]+);/gi, (entity, name: string) => namedEntities[name.toLowerCase()] ?? entity)
    .replace(/&#(\d+);/g, (entity, digits: string) => decodeCodePoint(entity, Number(digits)))
    .replace(/&#x([0-9a-f]+);/gi, (entity, digits: string) =>
      decodeCodePoint(entity, Number.parseInt(digits, 16)),
    )
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function decodeCodePoint(fallback: string, codePoint: number): string {
  if (!Number.isInteger(codePoint) || codePoint <= 0 || codePoint > 0x10ffff) return fallback;
  try {
    return String.fromCodePoint(codePoint);
  } catch {
    return fallback;
  }
}
