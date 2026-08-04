import { spawn } from "node:child_process";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(await readFile(new URL("../package.json", import.meta.url), "utf8"));
const executable = fileURLToPath(
  new URL(
    `../artifacts/windows/pixnya-${packageJson.version}-windows-x64-debug.exe`,
    import.meta.url,
  ),
);
await access(executable);

const port = await reservePort();
const userDataPrefix = fileURLToPath(new URL("../target/windows-queue-webview-", import.meta.url));
const userDataFolder = await mkdtemp(userDataPrefix);
const child = spawn(executable, [], {
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    WEBVIEW2_USER_DATA_FOLDER: userDataFolder,
    PIXIV_CLIENT_TEST_ROOT: path.join(userDataFolder, "application"),
  },
  stdio: "ignore",
  windowsHide: true,
});

try {
  const mainTarget = await waitFor(async () => {
    const targets = await readTargets(port);
    return targets.find(
      (target) =>
        target.type === "page" &&
        /^(?:https?:\/\/tauri\.localhost|tauri:\/\/localhost)/.test(target.url),
    );
  }, 20_000, "main WebView");
  const cdp = await connectCdp(mainTarget.webSocketDebuggerUrl);
  try {
    await cdp.setViewport(390, 844);
    await cdp.navigate(new URL("/offline", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`({
        queueTitle: document.querySelector('#queue-title')?.textContent?.trim(),
        libraryTitle: document.querySelector('#library-title')?.textContent?.trim(),
        queueReady: Boolean(document.querySelector('.queue-section .task-list, .queue-section .queue-empty')),
        libraryReady: Boolean(document.querySelector('.library-section .entries, .library-section .empty')),
        queueError: document.querySelector('.queue-section [role="alert"]')?.textContent ?? null,
        libraryError: document.querySelector('.library-section [role="alert"]')?.textContent ?? null,
        refreshLayoutReady: (() => {
          const buttons = [...document.querySelectorAll('.section-refresh')];
          return buttons.length === 2 && buttons.every((button) => {
            const style = getComputedStyle(button);
            const textRange = document.createRange();
            textRange.selectNodeContents(button);
            return style.whiteSpace === 'nowrap' &&
              style.flexShrink === '0' &&
              button.getBoundingClientRect().width >= 68 &&
              textRange.getClientRects().length === 1;
          });
        })()
      })`).then((state) =>
        state.queueTitle === "下载队列" &&
        state.libraryTitle === "已下载内容" &&
        state.queueReady &&
        state.libraryReady &&
        state.refreshLayoutReady &&
        !state.queueError &&
        !state.libraryError,
      ),
      15_000,
      "persistent download queue UI",
    );

    await cdp.navigate(new URL("/settings", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(
        `document.body.innerText.includes('下载队列与离线资料库') && document.body.innerText.includes(${JSON.stringify(packageJson.version)})`,
      ),
      10_000,
      "queue settings and current version",
    );
  } finally {
    cdp.close();
  }
  console.log(`PASS Windows ${packageJson.version}: mobile queue refresh controls and storage settings mounted.`);
} finally {
  child.kill();
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 2_000)),
  ]);
  if (userDataFolder.startsWith(userDataPrefix)) {
    await rm(userDataFolder, { recursive: true, force: true, maxRetries: 3 }).catch(() => {});
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
    async setViewport(width, height) {
      await send("Emulation.setDeviceMetricsOverride", {
        width,
        height,
        deviceScaleFactor: 1,
        mobile: true,
      });
    },
    async navigate(url) {
      const response = await send("Page.navigate", { url });
      if (response.errorText) throw new Error(`CDP navigation failed: ${response.errorText}`);
    },
    async evaluate(expression) {
      const response = await send("Runtime.evaluate", {
        expression,
        awaitPromise: true,
        returnByValue: true,
      });
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
