import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const defaultProjectRoot = fileURLToPath(new URL("../", import.meta.url));

const knownGradleWrappers = new Map([
  [
    "8.14.3",
    {
      distributionSha256: "bd71102213493060956ec229d946beee57158dbd89d0e62b91bca0fa2c5f3531",
      wrapperJarSha256: "7d3a4ac4de1c32b59bc6a4eb8ecb8e612ccd0cf1ae1e99f66902da64df296172",
    },
  ],
]);

const lockDefinitions = [
  {
    name: "app",
    lockPath: "app/gradle.lockfile",
    buildPath: "app/build.gradle.kts",
    lockingPattern: /dependencyLocking\s*\{[\s\S]*?lockAllConfigurations\(\)[\s\S]*?lockMode\.set\(LockMode\.STRICT\)[\s\S]*?\}/,
  },
  {
    name: "buildscript",
    lockPath: "buildscript-gradle.lockfile",
    buildPath: "build.gradle.kts",
    lockingPattern: /configurations\.classpath\s*\{[\s\S]*?resolutionStrategy\.activateDependencyLocking\(\)[\s\S]*?\}/,
    extraPattern: /dependencyLocking\s*\{[\s\S]*?lockMode\.set\(LockMode\.STRICT\)[\s\S]*?\}/,
  },
  {
    name: "buildSrc",
    lockPath: "buildSrc/gradle.lockfile",
    buildPath: "buildSrc/build.gradle.kts",
    lockingPattern: /dependencyLocking\s*\{[\s\S]*?lockAllConfigurations\(\)[\s\S]*?lockMode\.set\(LockMode\.STRICT\)[\s\S]*?\}/,
  },
];

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function normalizedTextBytes(value) {
  return Buffer.from(value.toString("utf8").replace(/\r\n?/g, "\n"), "utf8");
}

function readRequired(path, description) {
  if (!existsSync(path)) throw new Error(`${description} is missing: ${path}`);
  return readFileSync(path);
}

function parseProperties(text) {
  const properties = new Map();
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || !line.includes("=")) continue;
    const separator = line.indexOf("=");
    properties.set(line.slice(0, separator), line.slice(separator + 1));
  }
  return properties;
}

function wrapperVersion(distributionUrl) {
  const match = distributionUrl.match(/gradle-(\d+\.\d+(?:\.\d+)?)-bin\.zip$/);
  if (!match) throw new Error(`Unsupported Gradle distribution URL: ${distributionUrl}`);
  return match[1];
}

function parseLockfile(text, source) {
  const entries = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#") || line.startsWith("empty=")) continue;
    const separator = line.lastIndexOf("=");
    if (separator <= 0) throw new Error(`${source} contains an invalid lock entry: ${line}`);
    const coordinate = line.slice(0, separator);
    const coordinateParts = coordinate.split(":");
    if (coordinateParts.length !== 3 || coordinateParts.some((part) => !part)) {
      throw new Error(`${source} contains an unsupported component coordinate: ${coordinate}`);
    }
    const configurations = line.slice(separator + 1).split(",").filter(Boolean).sort();
    if (configurations.length === 0) {
      throw new Error(`${source} does not record a configuration for ${coordinate}`);
    }
    entries.push({ coordinate, configurations });
  }
  if (entries.length === 0) throw new Error(`${source} contains no locked components.`);
  return entries;
}

function declaredCoordinates(buildSource) {
  const coordinates = [];
  const dependencyCall = /\b(?:classpath|implementation|api|compileOnly|runtimeOnly|testImplementation|androidTestImplementation)\s*\(\s*"([^"\n]+)"\s*\)/g;
  for (const match of buildSource.matchAll(dependencyCall)) {
    const coordinate = match[1];
    const parts = coordinate.split(":");
    if (parts.length !== 3 || parts.some((part) => !part)) {
      throw new Error(`Unsupported direct Gradle dependency coordinate: ${coordinate}`);
    }
    const version = parts[2];
    if (/\+|\b(?:latest|release|snapshot)\b|[\[\]()]/i.test(version)) {
      throw new Error(`Dynamic or changing Gradle dependency versions are forbidden: ${coordinate}`);
    }
    coordinates.push(coordinate);
  }
  return [...new Set(coordinates)].sort();
}

function xmlAttributes(value) {
  const attributes = new Map();
  for (const match of value.matchAll(/([A-Za-z][A-Za-z0-9_-]*)="([^"]*)"/g)) {
    attributes.set(match[1], match[2]);
  }
  return attributes;
}

export function parseVerificationMetadata(text) {
  if (!/<verify-metadata>true<\/verify-metadata>/.test(text)) {
    throw new Error("Gradle verification metadata must verify repository metadata.");
  }
  if (!/<verify-signatures>false<\/verify-signatures>/.test(text)) {
    throw new Error("Gradle verification metadata must declare its signature policy explicitly.");
  }

  const components = new Map();
  const componentPattern = /<component\s+([^>]+)>([\s\S]*?)<\/component>/g;
  for (const componentMatch of text.matchAll(componentPattern)) {
    const attributes = xmlAttributes(componentMatch[1]);
    const group = attributes.get("group");
    const name = attributes.get("name");
    const version = attributes.get("version");
    if (!group || !name || !version) throw new Error("Verification metadata has an incomplete component.");
    const coordinate = `${group}:${name}:${version}`;
    const artifacts = [];
    const artifactPattern = /<artifact\s+([^>]+)>([\s\S]*?)<\/artifact>/g;
    for (const artifactMatch of componentMatch[2].matchAll(artifactPattern)) {
      const artifactName = xmlAttributes(artifactMatch[1]).get("name");
      const hashes = [...artifactMatch[2].matchAll(/<sha256\s+[^>]*value="([a-fA-F0-9]{64})"[^>]*\/>/g)]
        .map((match) => match[1].toLowerCase())
        .sort();
      if (!artifactName || hashes.length === 0) {
        throw new Error(`Verification metadata lacks SHA-256 evidence for ${coordinate}.`);
      }
      artifacts.push({ name: artifactName, sha256: hashes });
    }
    if (artifacts.length === 0) {
      throw new Error(`Verification metadata contains no artifacts for ${coordinate}.`);
    }
    components.set(coordinate, artifacts.sort((left, right) => left.name.localeCompare(right.name)));
  }
  if (components.size === 0) throw new Error("Gradle verification metadata contains no components.");
  return components;
}

export function inspectAndroidGradleVerificationArtifacts(projectRoot = defaultProjectRoot) {
  const verificationPath = join(
    projectRoot,
    "src-tauri",
    "gen",
    "android",
    "gradle",
    "verification-metadata.xml",
  );
  return parseVerificationMetadata(
    readRequired(verificationPath, "Gradle dependency verification metadata").toString("utf8"),
  );
}

export function inspectAndroidGradleSupplyChain(projectRoot = defaultProjectRoot) {
  const androidRoot = join(projectRoot, "src-tauri", "gen", "android");
  const wrapperPropertiesPath = join(androidRoot, "gradle", "wrapper", "gradle-wrapper.properties");
  const wrapperJarPath = join(androidRoot, "gradle", "wrapper", "gradle-wrapper.jar");
  const wrapperPropertiesBytes = readRequired(wrapperPropertiesPath, "Gradle wrapper properties");
  const wrapperJarBytes = readRequired(wrapperJarPath, "Gradle wrapper JAR");
  const wrapperProperties = parseProperties(wrapperPropertiesBytes.toString("utf8"));
  const distributionUrl = (wrapperProperties.get("distributionUrl") ?? "").replaceAll("\\:", ":");
  const gradleVersion = wrapperVersion(distributionUrl);
  const expectedWrapper = knownGradleWrappers.get(gradleVersion);
  if (!expectedWrapper) throw new Error(`Gradle ${gradleVersion} has no reviewed wrapper checksums.`);
  if (wrapperProperties.get("distributionSha256Sum") !== expectedWrapper.distributionSha256) {
    throw new Error(`Gradle ${gradleVersion} distributionSha256Sum is missing or incorrect.`);
  }
  const wrapperJarDigest = sha256(wrapperJarBytes);
  if (wrapperJarDigest !== expectedWrapper.wrapperJarSha256) {
    throw new Error(`Gradle ${gradleVersion} wrapper JAR checksum is incorrect.`);
  }

  const lockedComponents = new Map();
  const fingerprintParts = [normalizedTextBytes(wrapperPropertiesBytes), wrapperJarBytes];
  for (const definition of lockDefinitions) {
    const buildPath = join(androidRoot, definition.buildPath);
    const buildSource = readRequired(buildPath, `${definition.name} Gradle build`).toString("utf8");
    if (!definition.lockingPattern.test(buildSource) || (definition.extraPattern && !definition.extraPattern.test(buildSource))) {
      throw new Error(`${definition.name} does not enable strict Gradle dependency locking.`);
    }
    const lockPath = join(androidRoot, definition.lockPath);
    const lockBytes = readRequired(lockPath, `${definition.name} Gradle lockfile`);
    fingerprintParts.push(Buffer.from(definition.lockPath), normalizedTextBytes(lockBytes));
    const entries = parseLockfile(lockBytes.toString("utf8"), definition.lockPath);
    const entriesByCoordinate = new Map(entries.map((entry) => [entry.coordinate, entry]));
    for (const coordinate of declaredCoordinates(buildSource)) {
      if (!entriesByCoordinate.has(coordinate)) {
        throw new Error(`${definition.name} direct dependency is not locked: ${coordinate}`);
      }
    }
    for (const entry of entries) {
      const current = lockedComponents.get(entry.coordinate) ?? { lockfiles: new Set(), configurations: new Set() };
      current.lockfiles.add(definition.lockPath);
      for (const configuration of entry.configurations) current.configurations.add(configuration);
      lockedComponents.set(entry.coordinate, current);
    }
  }

  const verificationPath = join(androidRoot, "gradle", "verification-metadata.xml");
  const verificationBytes = readRequired(verificationPath, "Gradle dependency verification metadata");
  fingerprintParts.push(
    Buffer.from("gradle/verification-metadata.xml"),
    normalizedTextBytes(verificationBytes),
  );
  const verifiedComponents = parseVerificationMetadata(verificationBytes.toString("utf8"));
  for (const coordinate of lockedComponents.keys()) {
    if (!verifiedComponents.has(coordinate)) {
      throw new Error(`Locked Gradle component has no checksum verification metadata: ${coordinate}`);
    }
  }

  const components = [...lockedComponents.entries()]
    .map(([coordinate, value]) => ({
      coordinate,
      lockfiles: [...value.lockfiles].sort(),
      configurations: [...value.configurations].sort(),
      artifacts: verifiedComponents.get(coordinate),
    }))
    .sort((left, right) => left.coordinate.localeCompare(right.coordinate));
  return {
    schemaVersion: 1,
    gradleVersion,
    fingerprint: `sha256:${sha256(Buffer.concat(fingerprintParts))}`,
    components,
  };
}

function parseArguments(argv) {
  const options = { check: false, output: null, projectRoot: defaultProjectRoot };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") {
      options.check = true;
    } else if (argument === "--output" || argument === "--project-root") {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value.`);
      if (argument === "--output") options.output = resolve(value);
      else options.projectRoot = resolve(value);
      index += 1;
    } else {
      throw new Error(`Unknown argument: ${argument}`);
    }
  }
  if (!options.check && !options.output) options.check = true;
  return options;
}

function runCli() {
  const options = parseArguments(process.argv.slice(2));
  const inventory = inspectAndroidGradleSupplyChain(options.projectRoot);
  if (options.output) {
    mkdirSync(dirname(options.output), { recursive: true });
    writeFileSync(options.output, `${JSON.stringify(inventory, null, 2)}\n`, "utf8");
    console.log(`Wrote ${options.output}`);
  }
  if (options.check) {
    console.log(
      `Android Gradle supply chain verified offline: ${inventory.components.length} locked components, Gradle ${inventory.gradleVersion}, ${inventory.fingerprint}.`,
    );
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(scriptPath)) {
  try {
    runCli();
  } catch (error) {
    console.error(error?.message ?? error);
    process.exitCode = 1;
  }
}
