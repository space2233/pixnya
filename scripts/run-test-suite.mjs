import { spawn } from "node:child_process";
import { readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("../", import.meta.url));
const scriptsRoot = fileURLToPath(new URL("./", import.meta.url));
const supportedSuites = new Set(["quick", "rust", "full"]);
const suite = process.argv[2] ?? "quick";

const npmInvocation = (...args) =>
  process.platform === "win32"
    ? [process.env.ComSpec ?? "cmd.exe", ["/d", "/s", "/c", "npm.cmd", ...args]]
    : ["npm", args];

if (!supportedSuites.has(suite)) {
  console.error(`Unknown test suite: ${suite}. Expected quick, rust, or full.`);
  process.exit(2);
}

const runStage = (name, command, args, options = {}) =>
  new Promise((resolve, reject) => {
    console.log(`\n==> ${name}`);
    const child = spawn(command, args, {
      cwd: projectRoot,
      env: { ...process.env, ...options.env },
      stdio: "inherit",
    });

    child.once("error", reject);
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      const reason = signal ? `signal ${signal}` : `exit code ${code}`;
      reject(new Error(`${name} failed with ${reason}`));
    });
  });

async function findNodeTests() {
  const entries = await readdir(scriptsRoot, { withFileTypes: true });
  return entries
    .filter((entry) => entry.isFile() && entry.name.endsWith(".test.mjs"))
    .map((entry) => `scripts/${entry.name}`)
    .sort();
}

async function runQuickSuite() {
  const testFiles = await findNodeTests();
  if (testFiles.length === 0) {
    throw new Error("No scripts/*.test.mjs regression tests were found.");
  }

  const [compileCommand, compileArgs] = npmInvocation("run", "i18n:compile");
  const [npmCommand, npmArgs] = npmInvocation("run", "check");
  await runStage("Compile localization messages", compileCommand, compileArgs);
  // The source-boundary tests and svelte-check both scan the project tree.
  // Running them together caused heavy disk contention on Windows (about 20s
  // versus about 6s sequentially), so keep the cheap regression suite first.
  await runStage(`Node regression tests (${testFiles.length} files)`, process.execPath, [
    "--test",
    ...testFiles,
  ]);
  await runStage("Svelte type and accessibility checks", npmCommand, npmArgs);
}

async function runRustSuite() {
  const rustEnvironment = { CARGO_INCREMENTAL: "0" };
  await runStage("Rust formatting", "cargo", ["fmt", "--all", "--", "--check"], {
    env: rustEnvironment,
  });
  await runStage(
    "Rust Clippy",
    "cargo",
    ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
    { env: rustEnvironment },
  );
  await runStage("Rust workspace tests", "cargo", ["test", "--workspace"], {
    env: rustEnvironment,
  });
}

try {
  if (suite === "quick") {
    await runQuickSuite();
  } else if (suite === "rust") {
    await runRustSuite();
  } else {
    await runQuickSuite();
    await runRustSuite();
  }
  console.log(`\n${suite} test suite passed.`);
} catch (error) {
  console.error(`\n${error?.message ?? error}`);
  process.exitCode = 1;
}
