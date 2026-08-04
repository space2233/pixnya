import { spawn } from "node:child_process";
import { access, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
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
const testRootPrefix = fileURLToPath(new URL("../target/windows-catalog-runtime-", import.meta.url));
const testRoot = await mkdtemp(testRootPrefix);
const localAppData = path.join(testRoot, "local-app-data");
const roamingAppData = path.join(testRoot, "roaming-app-data");
const webviewData = path.join(testRoot, "webview");
await Promise.all([
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
    PIXIV_CLIENT_TEST_ROOT: path.join(testRoot, "application"),
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
    await cdp.navigate(new URL("/offline", mainTarget.url).href);
    await waitFor(
      () => cdp.evaluate(`(() => {
        const manager = document.querySelector('.collection-manager');
        const create = document.querySelector('.new-collection button[type="submit"]');
        return Boolean(manager && create && !create.disabled && document.querySelector('#library-title'));
      })()`),
      15_000,
      "local catalog manager",
    );

    await cdp.evaluate(`(() => {
      const details = document.querySelector('.collection-manager');
      const input = document.querySelector('#new-collection-name');
      const form = document.querySelector('.new-collection');
      details.open = true;
      input.value = '自动测试收藏夹';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      form.requestSubmit();
    })()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.collection-list')?.innerText.includes('自动测试收藏夹') ?? false`),
      10_000,
      "collection creation",
    );

    await cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll('.collection-row')]
        .find((candidate) => candidate.textContent?.includes('自动测试收藏夹'));
      [...row.querySelectorAll('button')].find((button) => button.textContent?.includes('重命名')).click();
    })()`);
    await waitFor(
      () => cdp.evaluate(`Boolean(document.querySelector('.collection-row form input'))`),
      5_000,
      "collection rename editor",
    );
    await cdp.evaluate(`(() => {
      const input = document.querySelector('.collection-row form input');
      input.value = '已重命名收藏夹';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.closest('form').requestSubmit();
    })()`);
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.collection-list')?.innerText.includes('已重命名收藏夹') ?? false`),
      10_000,
      "collection rename",
    );

    const clickDelete = () => cdp.evaluate(`(() => {
      const row = [...document.querySelectorAll('.collection-row')]
        .find((candidate) => candidate.textContent?.includes('已重命名收藏夹'));
      [...row.querySelectorAll('button')].find((button) => /删除/.test(button.textContent ?? '')).click();
    })()`);
    await clickDelete();
    await waitFor(
      () => cdp.evaluate(`document.querySelector('.collection-row button.confirm')?.textContent?.includes('确认删除') ?? false`),
      5_000,
      "collection delete confirmation",
    );
    await clickDelete();
    await waitFor(
      () => cdp.evaluate(`!document.querySelector('.collection-list')?.innerText.includes('已重命名收藏夹')`),
      10_000,
      "collection deletion",
    );
    console.log(`PASS Windows ${packageJson.version}: local collection create, rename, and confirmed delete.`);
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
