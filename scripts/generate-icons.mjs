import { execFileSync } from "node:child_process";
import { cp, mkdtemp, readdir, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { repositoryPath } from "./test-paths.mjs";

// Generate in isolation so Tauri's unused iOS outputs never enter the repository.
const temporary = await mkdtemp(path.join(os.tmpdir(), "pixnya-icons-"));
const icons = repositoryPath("src-tauri", "icons");
const android = repositoryPath("src-tauri", "gen", "android", "app", "src", "main", "res");
try {
  execFileSync(process.execPath, [
    repositoryPath("node_modules", "@tauri-apps", "cli", "tauri.js"),
    "icon", path.join(icons, "pixnya-mascot-source.png"), "--output", temporary,
  ], { cwd: repositoryPath(), stdio: "inherit" });
  for (const entry of await readdir(temporary, { withFileTypes: true })) {
    if (entry.isFile()) await cp(path.join(temporary, entry.name), path.join(icons, entry.name));
  }
  await cp(path.join(temporary, "android"), android, { recursive: true });
  // Round-icon launchers use the same adaptive foreground and background.
  await cp(path.join(android, "mipmap-anydpi-v26", "ic_launcher.xml"),
    path.join(android, "mipmap-anydpi-v26", "ic_launcher_round.xml"));
  await cp(path.join(icons, "128x128.png"), repositoryPath("static", "favicon.png"));
} finally {
  await rm(temporary, { recursive: true, force: true });
}
