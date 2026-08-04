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
const testRootPrefix = fileURLToPath(new URL("../target/windows-session-r18-runtime-", import.meta.url));
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
    await cdp.addScript(`
      (() => {
        const probe = window.__sessionRestoreProbe = {
          sawRestoring: false,
          promptBeforeRestore: false,
        };
        const promptVisible = () => document.body?.innerText.includes('登录后载入完整内容') ?? false;
        const inspect = () => {
          const restoring = document.querySelector('.app-frame')?.dataset.sessionRestoring === 'true';
          if (restoring) probe.sawRestoring = true;
          if (restoring && promptVisible()) probe.promptBeforeRestore = true;
        };
        const observe = () => {
          if (!document.documentElement) {
            setTimeout(observe, 0);
            return;
          }
          new MutationObserver(inspect).observe(document.documentElement, { childList: true, subtree: true, characterData: true });
          inspect();
        };
        observe();
      })();
    `);
    await cdp.navigate(new URL(`/?session-probe=${Date.now()}`, mainTarget.url).href);
    try {
      await waitFor(
        () => cdp.evaluate(`window.__sessionRestoreProbe && document.querySelector('.app-frame')?.dataset.sessionRestoring === 'false' && document.body.innerText.includes('登录后载入完整内容')`),
        10_000,
        "completed logged-out session restoration",
      );
    } catch (error) {
      const snapshot = await cdp.evaluate(`({
        url: location.href,
        probe: window.__sessionRestoreProbe ?? null,
        restoring: document.querySelector('.app-frame')?.dataset.sessionRestoring ?? null,
        hasPrompt: document.body?.innerText.includes('登录后载入完整内容') ?? false,
        body: document.body?.innerText.slice(0, 300) ?? ''
      })`);
      throw new Error(`Session runtime snapshot: ${JSON.stringify(snapshot)}`, { cause: error });
    }
    const promptBeforeRestore = await cdp.evaluate(`window.__sessionRestoreProbe.promptBeforeRestore`);
    if (promptBeforeRestore) throw new Error("The login prompt was rendered before restore_session completed");

    await cdp.navigate(new URL("/settings", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]')?.getAttribute('aria-checked') === 'false'`),
      10_000,
      "default concealed R18 setting",
    );
    await cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]')?.getAttribute('aria-checked') === 'true' && localStorage.getItem('pixiv-client.r18-default-visible') === 'visible'`),
      5_000,
      "enabled persistent R18 setting",
    );
    await cdp.navigate(new URL(`/settings?r18-reload=${Date.now()}`, mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]')?.getAttribute('aria-checked') === 'true'`),
      10_000,
      "restored R18 setting",
    );
    await cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('button[aria-label="默认显示 R18"]')?.getAttribute('aria-checked') === 'false' && localStorage.getItem('pixiv-client.r18-default-visible') === 'concealed'`),
      5_000,
      "disabled persistent R18 setting",
    );

    console.log(`PASS Windows ${packageJson.version}: session prompt timing and persistent R18 visibility setting.`);
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
    async addScript(source) {
      await send("Page.enable");
      await send("Page.addScriptToEvaluateOnNewDocument", { source });
    },
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
