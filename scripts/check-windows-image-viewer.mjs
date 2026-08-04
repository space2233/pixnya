import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const executable = fileURLToPath(
    new URL(`../artifacts/windows/pixnya-${packageJson.version}-windows-x64-debug.exe`, import.meta.url),
);
await access(executable);

const port = await reservePort();
const testRootPrefix = fileURLToPath(new URL("../target/windows-viewer-runtime-", import.meta.url));
const testRoot = await mkdtemp(testRootPrefix);
const localAppData = path.join(testRoot, "local-app-data");
const roamingAppData = path.join(testRoot, "roaming-app-data");
const webviewData = path.join(testRoot, "webview");
await Promise.all([
  mkdir(localAppData, { recursive: true }),
  mkdir(roamingAppData, { recursive: true }),
  mkdir(webviewData, { recursive: true }),
]);
const isolatedApplicationRoot = path.join(testRoot, "application");
await seedOfflineArtwork(path.join(isolatedApplicationRoot, "data", "offline-library"));

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
    await cdp.navigate(new URL("/offline/artworks/9001", mainTarget.url).href);
    await waitFor(
      async () => {
        const state = await cdp.evaluate(`(() => ({
          count: document.querySelectorAll('.artwork-gallery-preview figure > button').length,
          text: document.body?.innerText?.slice(0, 500) ?? '',
          url: location.href,
        }))()`);
        if (state.count !== 2) {
          state.ipc = await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke('read_offline_text', {
            key: 'artwork-9001', assetName: 'detail.json'
          }).then((value) => ({ ok: true, value })).catch((error) => ({
            ok: false,
            error: JSON.stringify(error, Object.getOwnPropertyNames(error)),
          }))`);
        }
        if (state.count !== 2) throw new Error(JSON.stringify(state));
        return true;
      },
      15_000,
      "two-page offline artwork",
    );
    await cdp.evaluate(`document.querySelector('.artwork-gallery-preview figure > button').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.viewer[role="dialog"] .image-stage img')?.src.startsWith('blob:') ?? false`),
      10_000,
      "offline image in the immersive viewer",
    );

    const initialState = await cdp.evaluate(`document.querySelector('.viewer header span')?.textContent ?? ''`);
    if (!initialState.includes("1 / 2") || !initialState.includes("100%")) {
      throw new Error(`Unexpected initial viewer state: ${initialState}`);
    }
    await cdp.evaluate(`document.querySelector('.viewer footer button[aria-label="放大"]').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.viewer header span')?.textContent.includes('125%') ?? false`),
      5_000,
      "button zoom",
    );
    await cdp.evaluate(`document.querySelector('.viewer .page-nav.next').click()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.viewer header span')?.textContent.includes('2 / 2 · 100%') ?? false`),
      5_000,
      "next-page navigation and zoom reset",
    );
    await cdp.evaluate(`document.querySelector('.viewer').dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }))`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.viewer header span')?.textContent.includes('1 / 2') ?? false`),
      5_000,
      "keyboard page navigation",
    );
    await cdp.evaluate(`(() => {
      const viewport = document.querySelector('.viewer .viewport');
      const bounds = viewport.getBoundingClientRect();
      viewport.dispatchEvent(new MouseEvent('dblclick', {
        bubbles: true,
        clientX: bounds.left + bounds.width / 2,
        clientY: bounds.top + bounds.height / 2,
      }));
    })()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.viewer header span')?.textContent.includes('250%') ?? false`),
      5_000,
      "double-click zoom",
    );
    await cdp.evaluate(`document.querySelector('.viewer').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))`);
    await waitFor(
      () => cdp.evaluate(`!document.querySelector('.viewer')`),
      5_000,
      "viewer close through history-backed Escape handling",
    );
    console.log(`PASS Windows ${packageJson.version}: offline viewer image load, zoom, paging, keyboard, and close.`);
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

async function seedOfflineArtwork(libraryRoot) {
  const entryRoot = path.join(libraryRoot, "artwork-9001");
  const assetRoot = path.join(entryRoot, "assets");
  await mkdir(assetRoot, { recursive: true });
  const detail = Buffer.from(JSON.stringify({
    illustration: {
      id: "9001",
      title: "查看器自动测试",
      author: { id: "1", name: "本地测试", account: "local-test" },
    },
    caption: "仅存在于隔离测试目录",
    pages: [{ pageIndex: 0 }, { pageIndex: 1 }],
  }));
  const image = Buffer.from(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9WlS8AAAAASUVORK5CYII=",
    "base64",
  );
  const assets = [
    { name: "detail.json", contentType: "application/json", bytes: detail },
    { name: "page-0001.png", contentType: "image/png", bytes: image },
    { name: "page-0002.png", contentType: "image/png", bytes: image },
  ];
  await Promise.all(assets.map((asset) => writeFile(path.join(assetRoot, asset.name), asset.bytes)));
  const manifest = {
    entry: {
      key: "artwork-9001",
      kind: "artwork",
      resourceId: "9001",
      title: "查看器自动测试",
      author: "本地测试",
      coverUrl: null,
      storedAtUnixSeconds: Math.floor(Date.now() / 1000),
      assetCount: assets.length,
      sizeBytes: assets.reduce((total, asset) => total + asset.bytes.byteLength, 0),
    },
    assets: assets.map((asset) => ({
      name: asset.name,
      content_type: asset.contentType,
      size_bytes: asset.bytes.byteLength,
    })),
  };
  await writeFile(path.join(entryRoot, "manifest.json"), JSON.stringify(manifest));
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
