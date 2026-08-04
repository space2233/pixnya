import { readFileSync } from "node:fs";

const profile = readFileSync(
  new URL("../src/routes/profile/+page.svelte", import.meta.url),
  "utf8",
);
const login = readFileSync(
  new URL("../src/routes/login/+page.svelte", import.meta.url),
  "utf8",
);

function ruleBody(source, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = source.match(new RegExp(`${escaped}\\s*\\{(?<body>[^}]*)\\}`));
  if (!match?.groups?.body) throw new Error(`Missing CSS rule: ${selector}`);
  return match.groups.body;
}

function requirePattern(body, pattern, message) {
  if (!pattern.test(body)) throw new Error(message);
}

const icon = ruleBody(profile, ".quick-section a > span");
const title = ruleBody(profile, ".quick-section b");
const description = ruleBody(profile, ".quick-section small");
const chevron = ruleBody(profile, ".quick-section i");

requirePattern(icon, /grid-column:\s*1\s*;/, "Quick-entry icon is not locked to column 1.");
requirePattern(title, /grid-column:\s*2\s*;/, "Quick-entry title is not locked to column 2.");
requirePattern(title, /grid-row:\s*1\s*;/, "Quick-entry title is not locked to row 1.");
requirePattern(
  description,
  /grid-column:\s*2\s*;/,
  "Quick-entry description is not locked to column 2.",
);
requirePattern(
  description,
  /grid-row:\s*2\s*;/,
  "Quick-entry description is not locked to row 2.",
);
requirePattern(chevron, /grid-column:\s*3\s*;/, "Quick-entry chevron is not locked to column 3.");

const mobileStart = profile.indexOf("@media (max-width: 720px)");
const narrowStart = profile.indexOf("@media (max-width: 420px)");
if (mobileStart < 0 || narrowStart < mobileStart) throw new Error("Missing profile mobile breakpoint.");
const mobileCss = profile.slice(mobileStart, narrowStart);
const mobileTitle = ruleBody(mobileCss, ".quick-section b");
const mobileDescription = ruleBody(mobileCss, ".quick-section small");
requirePattern(mobileTitle, /font-size:\s*12px\s*;/, "Mobile quick-entry title must be 12px.");
requirePattern(
  mobileDescription,
  /font-size:\s*10px\s*;/,
  "Mobile quick-entry description must be 10px.",
);

if (/当前状态/.test(profile)) {
  throw new Error("Profile must not show the redundant current-status label.");
}
const profileMain = ruleBody(profile, ".profile-main");
const profileAvatar = ruleBody(profile, ".profile-avatar");
requirePattern(
  profileMain,
  /padding:\s*20px\s+24px\s*;/,
  "Profile identity row needs breathing room below the banner.",
);
requirePattern(
  profileAvatar,
  /margin-top:\s*0\s*;/,
  "Profile avatar must not overlap the banner with a negative margin.",
);

if (!/<div class="pixiv-symbol"><span>p<\/span><\/div>/.test(login)) {
  throw new Error("Pixiv p glyph needs an inner span for optical centering.");
}
const pixivGlyph = ruleBody(login, ".pixiv-symbol span");
requirePattern(pixivGlyph, /line-height:\s*1\s*;/, "Pixiv p glyph needs a unit line height.");
requirePattern(
  pixivGlyph,
  /translateY\(-0\.1em\)/,
  "Pixiv p glyph needs a -0.1em optical vertical correction.",
);

console.log("PASS mobile quick entries and Pixiv p glyph keep their intended layout.");
