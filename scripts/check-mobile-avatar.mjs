import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../src/app.css", import.meta.url), "utf8");

function ruleBody(selector, source = css) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escaped}\\s*\\{(?<body>[^}]*)\\}`));
  if (!match?.groups?.body) throw new Error(`Missing CSS rule: ${selector}`);
  return match.groups.body;
}

function pixels(body, property) {
  const match = body.match(new RegExp(`${property}:\\s*(\\d+)px`));
  if (!match) throw new Error(`Missing pixel property: ${property}`);
  return Number(match[1]);
}

const avatar = ruleBody(".login-avatar");
const mobileStart = css.indexOf("@media (max-width: 959px)");
const mobileEnd = css.indexOf("@media (max-width: 560px)");
if (mobileStart < 0 || mobileEnd < mobileStart) {
  throw new Error("Missing mobile breakpoint");
}

const mobileCss = css.slice(mobileStart, mobileEnd);
const mobileTopbar = ruleBody(".app-topbar", mobileCss);
const columns = mobileTopbar.match(
  /grid-template-columns:\s*(\d+)px\s+1fr\s+(\d+)px/,
);
if (!columns) throw new Error("Missing mobile topbar tracks");

const width = pixels(avatar, "width");
const height = pixels(avatar, "height");
const marginMatch = avatar.match(/margin:\s*0\s+(\d+)px/);
const baseMargin = marginMatch ? Number(marginMatch[1]) : 0;
const mobileAvatar = ruleBody(".login-avatar", mobileCss);
const mobileMarginMatch = mobileAvatar.match(/margin:\s*0\s+(\d+)px/);
const margin = mobileMarginMatch ? Number(mobileMarginMatch[1]) : baseMargin;
const track = Number(columns[2]);
const shrinkLocked =
  /flex:\s*0\s+0\s+\d+px/.test(avatar) || /flex-shrink:\s*0/.test(avatar);
const renderedWidth = shrinkLocked
  ? width
  : Math.min(width, track - margin * 2);

if (renderedWidth !== height) {
  throw new Error(
    `Mobile login avatar is ${renderedWidth}x${height}px: ${width}px width + ` +
      `${margin}px side margins is shrinkable inside a ${track}px track.`,
  );
}

if (width + margin * 2 > track) {
  throw new Error(
    `Mobile login avatar outer width (${width + margin * 2}px) exceeds its ${track}px track.`,
  );
}

if (!/aspect-ratio:\s*1(?:\s*\/\s*1)?\s*;/.test(avatar)) {
  throw new Error("Mobile login avatar lacks an explicit 1:1 aspect ratio guard.");
}

console.log(`PASS mobile login avatar remains square at ${width}x${height}px.`);
