export type NovelBlock =
  | { kind: "paragraph"; text: string }
  | { kind: "chapter"; text: string }
  | { kind: "page_break" }
  | { kind: "artwork_link"; id: string }
  | { kind: "uploaded_image"; id: string }
  | { kind: "external_link"; label: string; url: string };

const resourcePattern = /^[1-9]\d{0,19}$/;

export function parseNovelText(source: string, defaultChapter = "Chapter"): NovelBlock[] {
  const normalized = source.replaceAll("\r\n", "\n").replaceAll("\r", "\n");
  const blocks: NovelBlock[] = [];
  let paragraph: string[] = [];

  function flushParagraph() {
    const text = paragraph.join("\n").trim();
    if (text) blocks.push({ kind: "paragraph", text });
    paragraph = [];
  }

  for (const line of normalized.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) {
      flushParagraph();
      continue;
    }
    if (trimmed === "[newpage]") {
      flushParagraph();
      blocks.push({ kind: "page_break" });
      continue;
    }
    const chapter = trimmed.match(/^\[chapter:(.*)]$/);
    if (chapter) {
      flushParagraph();
      blocks.push({ kind: "chapter", text: chapter[1].trim() || defaultChapter });
      continue;
    }
    const artwork = trimmed.match(/^\[pixivimage:(\d+)]$/);
    if (artwork && resourcePattern.test(artwork[1])) {
      flushParagraph();
      blocks.push({ kind: "artwork_link", id: artwork[1] });
      continue;
    }
    const uploaded = trimmed.match(/^\[uploadedimage:(\d+)]$/);
    if (uploaded && resourcePattern.test(uploaded[1])) {
      flushParagraph();
      blocks.push({ kind: "uploaded_image", id: uploaded[1] });
      continue;
    }
    const jump = trimmed.match(/^\[\[jumpuri:(.*?)\s*>\s*(https?:\/\/[^\]\s]+)]]$/);
    if (jump) {
      flushParagraph();
      blocks.push({ kind: "external_link", label: jump[1].trim() || jump[2], url: jump[2] });
      continue;
    }
    paragraph.push(line);
  }
  flushParagraph();
  return blocks;
}
