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
const userDataPrefix = fileURLToPath(new URL("../target/windows-storage-webview-", import.meta.url));
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
    await cdp.navigate(new URL("/settings", mainTarget.url).href);
    const state = await waitFor(
      () => cdp.evaluate(`(() => {
        const row = document.querySelector('.storage-health-row');
        const select = document.querySelector('select[aria-label="在线媒体缓存上限"]');
        const exportRows = [...document.querySelectorAll('.export-destination-row')];
        const exportRow = exportRows.find((candidate) => candidate.textContent?.includes('下载导出目录'));
        const exportButton = [...(exportRow?.querySelectorAll('button') ?? [])]
          .find((button) => /选择目录|更改/.test(button.textContent ?? ''));
        const text = row?.textContent ?? '';
        return {
          ready: Boolean(row && select && !select.disabled && /可用/.test(text) && exportRow && exportButton && !exportButton.disabled),
          health: row?.classList.contains('critical') ? 'critical' : row?.classList.contains('low') ? 'low' : 'healthy',
          cacheLimit: select?.value ?? '',
          exportState: exportRow?.textContent?.includes('尚未选择') ? 'private-only' : 'configured',
          versionVisible: document.body.innerText.includes(${JSON.stringify(packageJson.version)})
        };
      })()`).then((value) => value.ready && value.versionVisible ? value : null),
      15_000,
      "storage policy settings",
    );
    if (![134217728, 268435456, 536870912, 1073741824].includes(Number(state.cacheLimit))) {
      throw new Error(`Unexpected cache limit ${state.cacheLimit}`);
    }
    console.log(
      `PASS Windows ${packageJson.version}: storage=${state.health}, cacheLimit=${state.cacheLimit}, export=${state.exportState}.`,
    );
  } finally {
    cdp.close();
  }
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
