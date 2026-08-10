import { readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { inspectAndroidGradleSupplyChain } from "./check-android-gradle-supply-chain.mjs";
import { discoverGradlePomVerificationEntries } from "./gradle-license-evidence.mjs";

const scriptPath = fileURLToPath(import.meta.url);
const projectRoot = fileURLToPath(new URL("../", import.meta.url));

function xmlAttributes(value) {
  const attributes = new Map();
  for (const match of value.matchAll(/([A-Za-z][A-Za-z0-9_-]*)="([^"]*)"/g)) {
    attributes.set(match[1], match[2]);
  }
  return attributes;
}

function escapeXmlAttribute(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

function componentBlock(text, coordinate) {
  for (const match of text.matchAll(/^(\s*)<component\s+([^>]+)>([\s\S]*?)^\1<\/component>/gm)) {
    const attributes = xmlAttributes(match[2]);
    if (`${attributes.get("group")}:${attributes.get("name")}:${attributes.get("version")}` === coordinate) {
      return {
        start: match.index,
        end: match.index + match[0].length,
        indent: match[1],
        attributes: match[2],
        body: match[3],
        text: match[0],
      };
    }
  }
  return null;
}

function artifactBlock(body, artifactName) {
  for (const match of body.matchAll(/<artifact\s+([^>]+)>([\s\S]*?)<\/artifact>/g)) {
    if (xmlAttributes(match[1]).get("name") === artifactName) return match[2];
  }
  return null;
}

function renderedArtifact(indent, entry, newline) {
  const artifactIndent = `${indent}   `;
  const hashIndent = `${artifactIndent}   `;
  return `${artifactIndent}<artifact name="${escapeXmlAttribute(entry.pomName)}">${newline}${hashIndent}<sha256 value="${entry.sha256}" origin="Reviewed local Maven POM license evidence"/>${newline}${artifactIndent}</artifact>${newline}`;
}

function renderedComponent(entry, newline) {
  const [group, name, version] = entry.coordinate.split(":");
  const indent = "      ";
  return `${indent}<component group="${escapeXmlAttribute(group)}" name="${escapeXmlAttribute(name)}" version="${escapeXmlAttribute(version)}">${newline}${renderedArtifact(indent, entry, newline)}${indent}</component>${newline}`;
}

export function mergePomVerificationMetadata(metadataText, entries) {
  if (!/<components>[\s\S]*<\/components>/.test(metadataText)) {
    throw new Error("Unsupported Gradle verification metadata document.");
  }
  const newline = metadataText.includes("\r\n") ? "\r\n" : "\n";
  let output = metadataText;
  let addedCount = 0;
  for (const entry of [...entries].sort((left, right) => left.coordinate.localeCompare(right.coordinate))) {
    if (
      !/^.+:.+:.+$/.test(entry.coordinate ?? "") ||
      typeof entry.pomName !== "string" ||
      !/^[a-f0-9]{64}$/.test(entry.sha256 ?? "")
    ) {
      throw new Error("Invalid discovered Maven POM verification entry.");
    }
    const component = componentBlock(output, entry.coordinate);
    if (!component) {
      const insertionPoint = output.lastIndexOf(`${newline}   </components>`);
      if (insertionPoint < 0) throw new Error("Gradle verification metadata has no components closing tag.");
      output = `${output.slice(0, insertionPoint)}${newline}${renderedComponent(entry, newline)}${output.slice(insertionPoint + newline.length)}`;
      addedCount += 1;
      continue;
    }

    const artifact = artifactBlock(component.body, entry.pomName);
    if (artifact !== null) {
      const hashes = [...artifact.matchAll(/<sha256\s+[^>]*value="([a-fA-F0-9]{64})"[^>]*\/>/g)].map(
        (match) => match[1].toLowerCase(),
      );
      if (!hashes.includes(entry.sha256)) {
        throw new Error(
          `${entry.coordinate} ${entry.pomName} does not match its existing tracked SHA-256.`,
        );
      }
      continue;
    }

    const closing = `${component.indent}</component>`;
    const closingOffset = component.text.lastIndexOf(closing);
    if (closingOffset < 0) throw new Error(`Cannot extend verification metadata for ${entry.coordinate}.`);
    const replacement = `${component.text.slice(0, closingOffset)}${renderedArtifact(component.indent, entry, newline)}${component.text.slice(closingOffset)}`;
    output = `${output.slice(0, component.start)}${replacement}${output.slice(component.end)}`;
    addedCount += 1;
  }
  return { text: output, addedCount };
}

function parseArguments(argv) {
  const options = {
    check: false,
    write: false,
    gradleUserHome: process.env.GRADLE_USER_HOME,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") options.check = true;
    else if (argument === "--write") options.write = true;
    else if (argument === "--gradle-user-home") {
      const value = argv[index + 1];
      if (!value) throw new Error("--gradle-user-home requires a path.");
      options.gradleUserHome = resolve(value);
      index += 1;
    } else throw new Error(`Unknown argument: ${argument}`);
  }
  if (options.check === options.write) throw new Error("Choose exactly one of --check or --write.");
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  const inventory = inspectAndroidGradleSupplyChain(projectRoot);
  const entries = discoverGradlePomVerificationEntries(inventory, {
    gradleUserHome: options.gradleUserHome,
  });
  const metadataPath = join(
    projectRoot,
    "src-tauri",
    "gen",
    "android",
    "gradle",
    "verification-metadata.xml",
  );
  const current = readFileSync(metadataPath, "utf8");
  const merged = mergePomVerificationMetadata(current, entries);
  if (options.check) {
    if (merged.addedCount > 0) {
      throw new Error(
        `${merged.addedCount} Maven POM files still lack tracked SHA-256 evidence; review and run this script with --write.`,
      );
    }
    console.log(`${entries.length} Maven component and parent POM files have tracked SHA-256 evidence.`);
    return;
  }

  const temporaryPath = `${metadataPath}.tmp-${process.pid}`;
  try {
    writeFileSync(temporaryPath, merged.text, "utf8");
    renameSync(temporaryPath, metadataPath);
  } catch (error) {
    rmSync(temporaryPath, { force: true });
    throw error;
  }
  console.log(
    `Added ${merged.addedCount} reviewed Maven POM SHA-256 entries to ${metadataPath}; inspect the diff before committing.`,
  );
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(scriptPath)) {
  try {
    runCli();
  } catch (error) {
    console.error(error?.message ?? error);
    process.exitCode = 1;
  }
}
