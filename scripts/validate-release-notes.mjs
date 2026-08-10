import { pathToFileURL } from "node:url";

export const PUBLIC_RELEASE_REQUIRED_SECTIONS = Object.freeze([
  "## Unofficial status and platforms",
  "## API and OAuth boundary",
  "## Low-security connections",
  "## Source, licenses, SBOM, and checksums",
  "## Upgrade verification and limitations",
]);

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const requireText = (condition, message) => {
  if (!condition) {
    throw new Error(message);
  }
};

const parseVersion = (value) => value.split(".").map(Number);
const compareVersions = (left, right) => {
  const a = parseVersion(left);
  const b = parseVersion(right);
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
};

const labeledValue = (body, labels) => {
  const line = body.split("\n").find((candidate) =>
    labels.some((label) => candidate.trimStart().startsWith(`${label}:`))
  );
  requireText(Boolean(line), `release notes are missing: ${labels[0]}`);
  const value = line.slice(line.indexOf(":") + 1).trim();
  requireText(value !== "", `release notes contain an empty value: ${labels[0]}`);
  return value;
};

export function validateStableReleaseNotes({ notes, version, commitSha, allowPendingUpgrades = false }) {
  requireText(typeof notes === "string" && notes.trim() !== "", "release notes are empty");
  requireText(/^[1-9][0-9]*\.[0-9]+\.[0-9]+$/.test(version), "stable release version is invalid");
  requireText(/^[0-9a-f]{40}$/i.test(commitSha), "source commit must be a full Git SHA");
  requireText(!notes.includes("{{") && !notes.includes("}}"), "release notes contain an unfinished template placeholder");

  const lines = notes.replace(/\r\n?/g, "\n").split("\n");
  const pendingLines = lines.filter((line) => /(?:\bPENDING\b|待验收)/i.test(line));
  if (allowPendingUpgrades) {
    for (const line of pendingLines) {
      requireText(
        /^(?:\s*- (?:Windows x64|Linux x64|Android ARM64):|\s*(?:Failure-path coverage|失败路径覆盖|Known limitations|已知限制):)/.test(line),
        "PENDING is only allowed in Draft upgrade evidence",
      );
    }
  } else {
    requireText(pendingLines.length === 0, "stable release notes still contain PENDING evidence");
  }
  const sectionIndexes = PUBLIC_RELEASE_REQUIRED_SECTIONS.map((heading) => {
    const matches = lines.flatMap((line, index) => line.trim() === heading ? [index] : []);
    requireText(matches.length === 1, `release notes must contain exactly one section: ${heading}`);
    return matches[0];
  });
  requireText(
    sectionIndexes.every((index, position) => position === 0 || index > sectionIndexes[position - 1]),
    "release note sections are out of order",
  );

  const bodies = sectionIndexes.map((start, index) => {
    const end = sectionIndexes[index + 1] ?? lines.length;
    const body = lines.slice(start + 1, end).join("\n").trim();
    requireText(body !== "", `release note section is empty: ${PUBLIC_RELEASE_REQUIRED_SECTIONS[index]}`);
    return body;
  });

  requireText(/(?:unofficial|非官方)/i.test(bodies[0]), "unofficial status is missing");
  for (const platform of ["Windows x64", "Linux x64", "Android ARM64"]) {
    requireText(bodies[0].includes(platform), `supported platform is missing: ${platform}`);
  }
  requireText(/(?:non-public|非公开)/i.test(bodies[1]) && /OAuth/i.test(bodies[1]), "App API or OAuth boundary is missing");
  requireText(/(?:extractable|可提取|可从.{0,20}提取)/i.test(bodies[1]), "OAuth parameter extractability is missing");
  requireText(/(?:default|默认)/i.test(bodies[2]) && /(?:man-in-the-middle|中间人)/i.test(bodies[2]), "low-security default or risk is missing");

  const requiredAssets = [
    "LICENSE.txt",
    `pixnya-${version}-source.tar.gz`,
    `pixnya-${version}-third-party-licenses.tar.gz`,
    `pixnya-${version}.spdx.json`,
    `pixnya-${version}-android-runtime.spdx.json`,
    "SHA256SUMS.txt",
  ];
  requireText(bodies[3].includes(commitSha), "release notes do not name the source commit");
  requireText(bodies[3].includes("GPL-3.0-only"), "GPL-3.0-only disclosure is missing");
  for (const asset of requiredAssets) {
    requireText(bodies[3].includes(asset), `release notes do not name required attachment: ${asset}`);
  }

  const target = escapeRegExp(version);
  for (const platform of ["Windows x64", "Linux x64", "Android ARM64"]) {
    const line = bodies[4].split("\n").find((candidate) => candidate.trimStart().startsWith(`- ${platform}:`));
    requireText(Boolean(line), `upgrade result is missing: ${platform}`);
    const transition = line.match(new RegExp(`(\\d+\\.\\d+\\.\\d+)\\s*(?:->|→)\\s*(${target})`));
    requireText(Boolean(transition), `upgrade version transition is missing: ${platform}`);
    const baseline = transition[1];
    requireText(compareVersions(baseline, version) < 0, `upgrade baseline is not older than the target: ${platform}`);
    if (version === "1.0.0") {
      requireText(baseline === "0.29.0", `first stable baseline must be 0.29.0: ${platform}`);
    }
    if (allowPendingUpgrades && /(?:\bPENDING\b|待验收)/i.test(line)) {
      continue;
    }
    const fields = line.split(";").map((field) => field.trim());
    requireText(fields.length >= 3 && fields[1] !== "", `upgrade device or operating system is missing: ${platform}`);
    requireText(/^(?:PASS|通过)$/i.test(fields[2]), `upgrade result is not marked as passed: ${platform}`);
  }

  const failureCoverage = labeledValue(bodies[4], ["Failure-path coverage", "失败路径覆盖"]);
  const knownLimitations = labeledValue(bodies[4], ["Known limitations", "已知限制"]);
  if (!allowPendingUpgrades || !/(?:\bPENDING\b|待验收)/i.test(failureCoverage)) {
    for (const [label, pattern] of [
      ["wrong signature", /(?:wrong signature|错误签名)/i],
      ["corrupted manifest", /(?:corrupted manifest|损坏清单)/i],
      ["interrupted download", /(?:interrupted download|下载中断)/i],
      ["low space", /(?:low space|空间不足)/i],
      ["cancelled install", /(?:cancelled install|取消安装)/i],
      ["retry", /(?:retry|重试)/i],
    ]) {
      requireText(pattern.test(failureCoverage), `failure-path coverage is missing: ${label}`);
    }
  }
  requireText(
    allowPendingUpgrades || !/(?:\bPENDING\b|待验收)/i.test(knownLimitations),
    "known limitations are still pending",
  );
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
      allowPendingUpgrades: process.argv.includes("--allow-pending-upgrades"),
    });
    console.log("Stable release notes contain every required public disclosure.");
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
