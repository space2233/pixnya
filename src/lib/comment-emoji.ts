export type CommentTextSegment =
  | { kind: "text"; value: string }
  | { kind: "emoji"; token: string; url: string };

const COMMENT_EMOJI_FILES = {
  "(normal)": "101.png",
  "(surprise)": "102.png",
  "(serious)": "103.png",
  "(heaven)": "104.png",
  "(happy)": "105.png",
  "(excited)": "106.png",
  "(sing)": "107.png",
  "(cry)": "108.png",
  "(normal2)": "201.png",
  "(shame2)": "202.png",
  "(love2)": "203.png",
  "(interesting2)": "204.png",
  "(blush2)": "205.png",
  "(fire2)": "206.png",
  "(angry2)": "207.png",
  "(shine2)": "208.png",
  "(panic2)": "209.png",
  "(normal3)": "301.png",
  "(satisfaction3)": "302.png",
  "(surprise3)": "303.png",
  "(smile3)": "304.png",
  "(shock3)": "305.png",
  "(gaze3)": "306.png",
  "(wink3)": "307.png",
  "(happy3)": "308.png",
  "(excited3)": "309.png",
  "(love3)": "310.png",
  "(normal4)": "401.png",
  "(surprise4)": "402.png",
  "(serious4)": "403.png",
  "(love4)": "404.png",
  "(shine4)": "405.png",
  "(sweat4)": "406.png",
  "(shame4)": "407.png",
  "(sleep4)": "408.png",
  "(heart)": "501.png",
  "(teardrop)": "502.png",
  "(star)": "503.png",
} as const;

export type CommentEmojiToken = keyof typeof COMMENT_EMOJI_FILES;

export const COMMENT_EMOJIS = Object.entries(COMMENT_EMOJI_FILES).map(([token, file]) => ({
  token: token as CommentEmojiToken,
  url: `https://s.pximg.net/common/images/emoji/${file}`,
}));

export function commentEmojiUrl(token: string): string | null {
  const file = COMMENT_EMOJI_FILES[token as CommentEmojiToken];
  return file ? `https://s.pximg.net/common/images/emoji/${file}` : null;
}

export function tokenizeCommentText(value: string): CommentTextSegment[] {
  if (!value) return [];
  const segments: CommentTextSegment[] = [];
  let textStart = 0;
  let cursor = 0;

  while (cursor < value.length) {
    const opening = value.indexOf("(", cursor);
    if (opening < 0) break;
    const closing = value.indexOf(")", opening + 1);
    if (closing < 0 || closing - opening > 20) {
      cursor = opening + 1;
      continue;
    }
    const token = value.slice(opening, closing + 1);
    const url = commentEmojiUrl(token);
    if (!url) {
      cursor = opening + 1;
      continue;
    }
    if (opening > textStart) segments.push({ kind: "text", value: value.slice(textStart, opening) });
    segments.push({ kind: "emoji", token, url });
    cursor = closing + 1;
    textStart = cursor;
  }

  if (textStart < value.length) segments.push({ kind: "text", value: value.slice(textStart) });
  return segments;
}

export function insertCommentEmoji(
  value: string,
  token: CommentEmojiToken,
  selectionStart: number,
  selectionEnd: number,
  maxCharacters = 140,
): { value: string; cursor: number } {
  const safeStart = Math.max(0, Math.min(selectionStart, value.length));
  const safeEnd = Math.max(safeStart, Math.min(selectionEnd, value.length));
  const candidate = `${value.slice(0, safeStart)}${token}${value.slice(safeEnd)}`;
  const nextValue = Array.from(candidate).slice(0, maxCharacters).join("");
  return {
    value: nextValue,
    cursor: Math.min(safeStart + token.length, nextValue.length),
  };
}
