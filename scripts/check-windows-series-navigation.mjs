import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const executable = fileURLToPath(new URL(`../artifacts/windows/pixnya-${packageJson.version}-windows-x64-debug.exe`, import.meta.url));
await access(executable);

const port = await reservePort();
const testRootPrefix = fileURLToPath(new URL("../target/windows-series-navigation-runtime-", import.meta.url));
const testRoot = await mkdtemp(testRootPrefix);
const isolatedApplicationRoot = path.join(testRoot, "application");
const localAppData = path.join(testRoot, "local-app-data");
const roamingAppData = path.join(testRoot, "roaming-app-data");
const webviewData = path.join(testRoot, "webview");
await Promise.all([isolatedApplicationRoot, localAppData, roamingAppData, webviewData].map((directory) => mkdir(directory, { recursive: true })));

const child = spawn(executable, [], {
  env: {
    ...process.env,
    LOCALAPPDATA: localAppData,
    APPDATA: roamingAppData,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    WEBVIEW2_USER_DATA_FOLDER: webviewData,
    PIXIV_CLIENT_TEST_ROOT: isolatedApplicationRoot,
  },
  stdio: "ignore",
  windowsHide: true,
});

try {
  const target = await waitFor(async () => {
    const targets = await readTargets(port);
    return targets.find((candidate) => candidate.type === "page" && /^(?:https?:\/\/tauri\.localhost|tauri:\/\/localhost)/.test(candidate.url));
  }, 20_000, "main WebView");
  const cdp = await connectCdp(target.webSocketDebuggerUrl);
  try {
    await cdp.navigate(new URL("/series/artworks/77", target.url).href);
    await waitFor(
      () => cdp.evaluate(`location.pathname === '/series/artworks/77' && document.body.innerText.includes('登录后查看作品系列') && document.querySelector('a[href="/login"]') !== null`),
      10_000,
      "artwork series route",
    );

    await cdp.navigate(new URL("/series/novels/88", target.url).href);
    await waitFor(
      () => cdp.evaluate(`location.pathname === '/series/novels/88' && document.body.innerText.includes('登录后查看小说系列') && document.querySelector('a[href="/login"]') !== null`),
      10_000,
      "novel series route",
    );
    console.log(`PASS Windows ${packageJson.version}: artwork and novel series routes render in the packaged WebView.`);
  } finally {
    cdp.close();
  }
} finally {
  child.kill();
  await Promise.race([new Promise((resolve) => child.once("exit", resolve)), new Promise((resolve) => setTimeout(resolve, 2_000))]);
  if (testRoot.startsWith(testRootPrefix)) await rm(testRoot, { recursive: true, force: true, maxRetries: 3 }).catch(() => {});
}

async function reservePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const address = server.address();
  const selectedPort = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolve) => server.close(resolve));
  return selectedPort;
}

async function readTargets(selectedPort) {
  try { const response = await fetch(`http://127.0.0.1:${selectedPort}/json`); return response.ok ? response.json() : []; }
  catch { return []; }
}

async function waitFor(operation, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try { const result = await operation(); if (result) return result; } catch (error) { lastError = error; }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  throw new Error(`Timed out waiting for ${label}`, { cause: lastError });
}

async function connectCdp(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => { socket.addEventListener("open", resolve, { once: true }); socket.addEventListener("error", reject, { once: true }); });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data));
    if (!message.id || !pending.has(message.id)) return;
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(message.error.message)); else resolve(message.result);
  });
  const send = (method, params = {}) => {
    const id = ++sequence;
    return new Promise((resolve, reject) => { pending.set(id, { resolve, reject }); socket.send(JSON.stringify({ id, method, params })); });
  };
  return {
    async navigate(urlToOpen) { const response = await send("Page.navigate", { url: urlToOpen }); if (response.errorText) throw new Error(`CDP navigation failed: ${response.errorText}`); },
    async evaluate(expression) { const response = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }); if (response.exceptionDetails) throw new Error(response.exceptionDetails.exception?.description ?? "CDP evaluation failed"); return response.result.value; },
    close() { socket.close(); },
  };
}
