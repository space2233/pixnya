import { randomUUID } from "node:crypto";
import { mkdir, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { TextDecoder } from "node:util";

function fail(message) {
  throw new Error(message);
}

function readOutputPath(args) {
  if (args.length !== 2 || args[0] !== "--output" || !args[1]?.trim()) {
    fail("Usage: node scripts/prepare-tauri-release-config.mjs --output <path>");
  }
  return path.resolve(args[1]);
}

function validatePublicKey(value) {
  const encoded = value?.trim() ?? "";
  if (!encoded || !/^[A-Za-z0-9+/]+={0,2}$/.test(encoded) || encoded.length % 4 !== 0) {
    fail("PIXNYA_UPDATER_PUBKEY must be one Base64 layer around a complete minisign public key.");
  }

  let decodedBytes;
  let decoded;
  try {
    decodedBytes = Buffer.from(encoded, "base64");
    if (decodedBytes.toString("base64") !== encoded) {
      fail("PIXNYA_UPDATER_PUBKEY is not canonical Base64.");
    }
    decoded = new TextDecoder("utf-8", { fatal: true }).decode(decodedBytes);
  } catch {
    fail("PIXNYA_UPDATER_PUBKEY must be one Base64 layer around a complete minisign public key.");
  }

  const lines = decoded.replace(/\r\n/g, "\n").split("\n");
  if (lines.at(-1) === "") lines.pop();
  if (
    lines.length !== 2 ||
    !/^untrusted comment: .+/.test(lines[0]) ||
    !/^RW[A-Za-z0-9+/]+={0,2}$/.test(lines[1])
  ) {
    fail("PIXNYA_UPDATER_PUBKEY must decode to a complete minisign public key.");
  }

  let publicKeyBytes;
  try {
    publicKeyBytes = Buffer.from(lines[1], "base64");
  } catch {
    fail("PIXNYA_UPDATER_PUBKEY must decode to a complete minisign public key.");
  }
  if (publicKeyBytes.length !== 42 || publicKeyBytes.toString("base64") !== lines[1]) {
    fail("PIXNYA_UPDATER_PUBKEY must decode to a complete minisign public key.");
  }

  return encoded;
}

async function writeJsonAtomically(outputPath, value) {
  const directory = path.dirname(outputPath);
  const temporaryPath = path.join(directory, `.${path.basename(outputPath)}.${randomUUID()}.tmp`);
  await mkdir(directory, { recursive: true });
  try {
    await writeFile(temporaryPath, `${JSON.stringify(value, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    });
    await rename(temporaryPath, outputPath);
  } catch (error) {
    await rm(temporaryPath, { force: true });
    throw error;
  }
}

try {
  const outputPath = readOutputPath(process.argv.slice(2));
  const publicKey = validatePublicKey(process.env.PIXNYA_UPDATER_PUBKEY);
  await writeJsonAtomically(outputPath, {
    bundle: { createUpdaterArtifacts: true },
    plugins: { updater: { pubkey: publicKey } },
  });
} catch (error) {
  console.error(error?.message ?? error);
  process.exitCode = 1;
}
