import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const executable = fileURLToPath(
    new URL(`../artifacts/windows/pixnya-${packageJson.version}-windows-x64-debug.exe`, import.meta.url),
);
await access(executable);

const port = await reservePort();
const testRootPrefix = fileURLToPath(new URL("../target/windows-history-runtime-", import.meta.url));
const testRoot = await mkdtemp(testRootPrefix);
const isolatedApplicationRoot = path.join(testRoot, "application");
const localAppData = path.join(testRoot, "local-app-data");
const roamingAppData = path.join(testRoot, "roaming-app-data");
const webviewData = path.join(testRoot, "webview");
await Promise.all([
  mkdir(isolatedApplicationRoot, { recursive: true }),
  mkdir(localAppData, { recursive: true }),
  mkdir(roamingAppData, { recursive: true }),
  mkdir(webviewData, { recursive: true }),
]);

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
  const mainTarget = await waitFor(async () => {
    const targets = await readTargets(port);
    return targets.find(
      (target) => target.type === "page" && /^(?:https?:\/\/tauri\.localhost|tauri:\/\/localhost)/.test(target.url),
    );
  }, 20_000, "main WebView");
  const cdp = await connectCdp(mainTarget.webSocketDebuggerUrl);
  try {
    await waitFor(
      () => cdp.evaluate(`typeof window.__TAURI_INTERNALS__?.invoke === 'function'`),
      10_000,
      "Tauri IPC runtime",
    );
    const initial = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('get_browsing_history')`);
    if (!initial.enabled || initial.limit !== 500 || initial.entries.length !== 0) {
      throw new Error(`Unexpected initial history: ${JSON.stringify(initial)}`);
    }

    for (const record of [
      { kind: "artwork", resourceId: "991001", title: "历史测试作品", subtitle: "测试作者", thumbnailUrl: null },
      { kind: "novel", resourceId: "991002", title: "历史测试小说", subtitle: "小说作者", thumbnailUrl: null },
      { kind: "user", resourceId: "991003", title: "历史测试用户", subtitle: "@history-test", thumbnailUrl: null },
      { kind: "artwork", resourceId: "991001", title: "历史测试作品（更新）", subtitle: "测试作者", thumbnailUrl: null },
    ]) {
      const accepted = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('record_browsing_history', { record: ${JSON.stringify(record)} })`);
      if (!accepted) throw new Error(`History record was unexpectedly rejected: ${record.resourceId}`);
    }

    await cdp.navigate(new URL("/history", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`document.querySelectorAll('.history-list article').length === 3 && document.body.innerText.includes('历史测试作品（更新）')`),
      10_000,
      "deduplicated local history page",
    );
    const firstTitle = await cdp.evaluate(`document.querySelector('.history-list article .copy strong')?.textContent ?? ''`);
    if (firstTitle !== "历史测试作品（更新）") throw new Error(`Unexpected history order: ${firstTitle}`);

    await cdp.evaluate(`document.querySelector('.kind-filters button:nth-child(3)').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelectorAll('.history-list article').length === 1 && document.body.innerText.includes('历史测试小说')`),
      5_000,
      "novel history filter",
    );
    await cdp.evaluate(`document.querySelector('.kind-filters button:first-child').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelectorAll('.history-list article').length === 3`),
      5_000,
      "all history filter",
    );

    await cdp.evaluate(`document.querySelector('.page-heading > button').click()`);
    await waitFor(async () => {
      const snapshot = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('get_browsing_history')`);
      return snapshot.enabled === false && await cdp.evaluate(`document.body.innerText.includes('当前不会写入新的浏览记录')`);
    }, 5_000, "paused history state");
    const ignored = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('record_browsing_history', { record: {
      kind: 'artwork', resourceId: '991004', title: '不应写入', subtitle: '测试作者', thumbnailUrl: null
    } })`);
    if (ignored !== false) throw new Error("Disabled history accepted a new record");
    await cdp.evaluate(`document.querySelector('.page-heading > button').click()`);
    await waitFor(
      async () => (await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('get_browsing_history')`)).enabled === true,
      5_000,
      "resumed history state",
    );

    await cdp.evaluate(`document.querySelector('.history-list article > button').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelectorAll('.history-list article').length === 2`),
      5_000,
      "single history removal",
    );
    await cdp.evaluate(`document.querySelector('button.clear').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button.clear')?.textContent.includes('再次点击确认') ?? false`),
      5_000,
      "history clear confirmation",
    );
    await cdp.evaluate(`document.querySelector('button.clear').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelectorAll('.history-list article').length === 0 && document.body.innerText.includes('还没有浏览记录')`),
      5_000,
      "history clear",
    );

    await cdp.navigate(new URL("/settings", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`document.body.innerText.includes('本机浏览历史') && document.querySelector('button[aria-label="在本机记录浏览历史"]')?.getAttribute('aria-checked') === 'true'`),
      10_000,
      "settings history control",
    );
    await cdp.evaluate(`document.querySelector('button[aria-label="在本机记录浏览历史"]').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button[aria-label="在本机记录浏览历史"]')?.getAttribute('aria-checked') === 'false'`),
      5_000,
      "settings history toggle",
    );
    await cdp.navigate(new URL("/history", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.page-heading > button')?.textContent.includes('已停止记录') ?? false`),
      10_000,
      "history page reflects settings preference",
    );
    await cdp.evaluate(`document.querySelector('.page-heading > button').click()`);

    console.log(`PASS Windows ${packageJson.version}: SQLite history, deduplication, filter, pause, remove, clear, and settings synchronization.`);
  } finally {
    cdp.close();
  }
} finally {
  child.kill();
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 2_000)),
  ]);
  if (testRoot.startsWith(testRootPrefix)) {
    await rm(testRoot, { recursive: true, force: true, maxRetries: 3 }).catch(() => {});
  }
}

async function reservePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  const selectedPort = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolve) => server.close(resolve));
  return selectedPort;
}

async function readTargets(selectedPort) {
  try {
    const response = await fetch(`http://127.0.0.1:${selectedPort}/json`);
    return response.ok ? response.json() : [];
  } catch {
    return [];
  }
}

async function waitFor(operation, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const result = await operation();
      if (result) return result;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  throw new Error(`Timed out waiting for ${label}`, { cause: lastError });
}

async function connectCdp(url) {
  const socket = new WebSocket(url);
  await new Promise((resolve, reject) => {
    socket.addEventListener("open", resolve, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let sequence = 0;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data));
    if (!message.id || !pending.has(message.id)) return;
    const { resolve, reject } = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) reject(new Error(message.error.message));
    else resolve(message.result);
  });
  const send = (method, params = {}) => {
    const id = ++sequence;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
  };
  return {
    async navigate(url) {
      const response = await send("Page.navigate", { url });
      if (response.errorText) throw new Error(`CDP navigation failed: ${response.errorText}`);
    },
    async evaluate(expression) {
      const response = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
      if (response.exceptionDetails) {
        throw new Error(response.exceptionDetails.exception?.description ?? "CDP evaluation failed");
      }
      return response.result.value;
    },
    close() {
      socket.close();
    },
  };
}
