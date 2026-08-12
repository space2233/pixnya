import { pathToFileURL } from "node:url";

export const PUBLIC_RELEASE_REQUIRED_SECTIONS = Object.freeze([
  "## 中文",
  "## English",
]);

const requireText = (condition, message) => {
  if (!condition) throw new Error(message);
};

export function validateStableReleaseNotes({ notes, version, commitSha }) {
  requireText(typeof notes === "string" && notes.trim() !== "", "release notes are empty");
  requireText(/^[1-9][0-9]*\.[0-9]+\.[0-9]+$/.test(version), "stable release version is invalid");
  requireText(/^[0-9a-f]{40}$/i.test(commitSha), "source commit must be a full Git SHA");
  requireText(!notes.includes("{{") && !notes.includes("}}"), "release notes contain an unfinished template placeholder");
  requireText(!/\bPENDING\b/i.test(notes), "stable release notes still contain PENDING evidence");

  const lines = notes.replace(/\r\n?/g, "\n").split("\n");
  requireText(lines.some((line) => line.trim() === `# PixNya ${version}`), "release title does not match the version");

  const sectionIndexes = PUBLIC_RELEASE_REQUIRED_SECTIONS.map((heading) => {
    const matches = lines.flatMap((line, index) => line.trim() === heading ? [index] : []);
    requireText(matches.length === 1, `release notes must contain exactly one section: ${heading}`);
    return matches[0];
  });
  requireText(sectionIndexes[0] < sectionIndexes[1], "release note sections are out of order");

  const chinese = lines.slice(sectionIndexes[0] + 1, sectionIndexes[1]).join("\n").trim();
  const english = lines.slice(sectionIndexes[1] + 1).join("\n").trim();
  requireText(chinese !== "", "release note section is empty: ## 中文");
  requireText(english !== "", "release note section is empty: ## English");
  requireText(/[\u3400-\u9fff]/u.test(chinese), "Chinese release notes are missing Chinese text");
  requireText(/[A-Za-z]/.test(english), "English release notes are missing English text");

  for (const platform of [
    "Windows x64",
    "Windows ARM64",
    "Linux x64",
    "Android ARM64",
    "Android ARM32",
  ]) {
    requireText(notes.includes(platform), `supported platform is missing: ${platform}`);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const argumentsMap = new Map();
  for (let index = 2; index < process.argv.length; index += 2) {
    argumentsMap.set(process.argv[index], process.argv[index + 1]);
  }
  try {
    validateStableReleaseNotes({
      notes: process.env.RELEASE_NOTES ?? "",
      version: argumentsMap.get("--version") ?? "",
      commitSha: argumentsMap.get("--commit") ?? "",
    });
    console.log("Stable release notes contain concise Chinese and English summaries.");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
