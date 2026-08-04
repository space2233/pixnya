import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const androidPackageRoot = path.join(
  repositoryRoot,
  "src-tauri",
  "gen",
  "android",
  "app",
  "src",
  "main",
  "java",
  "io",
  "github",
  "space2233",
  "pixnya",
);

export function repositoryPath(...segments) {
  return path.join(repositoryRoot, ...segments);
}

export function androidPackagePath(...segments) {
  return path.join(androidPackageRoot, ...segments);
}
