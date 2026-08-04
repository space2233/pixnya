import { spawn } from "node:child_process";
import { access, mkdtemp, rm } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageJson = JSON.parse(
  await import("node:fs/promises").then(({ readFile }) =>
    readFile(new URL("../package.json", import.meta.url), "utf8"),
  ),
);

const executable = fileURLToPath(
  new URL(
    `../artifacts/windows/pixnya-${packageJson.version}-windows-x64-debug.exe`,
    import.meta.url,
  ),
);

await access(executable);

const requestedModes = (process.argv[2] ?? process.env.PIXIV_LOGIN_TEST_MODES ?? "standard,ech,compatible")
  .split(",")
  .map((mode) => mode.trim())
  .filter(Boolean);
const supportedModes = new Set(["standard", "ech", "compatible"]);
if (!requestedModes.length || requestedModes.some((mode) => !supportedModes.has(mode))) {
  throw new Error("PIXIV_LOGIN_TEST_MODES must contain standard, ech, or compatible");
}

for (const mode of requestedModes) {
  await checkMode(mode);
}

console.log(`PASS Windows warning preference and independent official login WebView in: ${requestedModes.join(", ")}.`);

async function checkMode(mode) {
  const port = await reservePort();
  const userDataPrefix = fileURLToPath(new URL("../target/windows-login-webview-", import.meta.url));
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
      const loginUrl = new URL(`/login?mode=${mode}`, mainTarget.url).href;
      await cdp.navigate(loginUrl);
      if (mode === "compatible") {
        await waitFor(
          () => cdp.evaluate("Boolean(document.querySelector('.danger-button'))"),
          10_000,
          "compatibility warning",
        );
        await cdp.evaluate("document.querySelector('.suppress-warning-choice input').click()", false);
        await cdp.evaluate("document.querySelector('.danger-button').click()", false);
        await waitFor(
          () => cdp.evaluate(`localStorage.getItem('pixiv-client.unsafe-connection-warning') === 'suppressed' && Boolean(document.querySelector('.official-button:not([disabled])'))`),
          20_000,
          "persisted compatibility warning preference",
        );

        await cdp.navigate(new URL("/settings/network", mainTarget.url).href);
        await waitFor(
          () => cdp.evaluate("Boolean(document.querySelector('.suppressed-warning-note button'))"),
          10_000,
          "warning restore control",
        );
        await cdp.navigate(`${loginUrl}&suppressed=${Date.now()}`);
        await waitFor(
          () => cdp.evaluate("Boolean(document.querySelector('.official-button:not([disabled])')) && !document.querySelector('.danger-button')"),
          20_000,
          "suppressed repeat warning",
        );

        await cdp.navigate(new URL("/settings/network", mainTarget.url).href);
        await waitFor(
          () => cdp.evaluate("Boolean(document.querySelector('.suppressed-warning-note button'))"),
          10_000,
          "restored warning setting",
        );
        await cdp.evaluate("document.querySelector('.suppressed-warning-note button').click()", false);
        await waitFor(
          () => cdp.evaluate(`localStorage.getItem('pixiv-client.unsafe-connection-warning') === 'visible'`),
          5_000,
          "visible compatibility warning preference",
        );
        await cdp.navigate(`${loginUrl}&restored=${Date.now()}`);
        await waitFor(
          () => cdp.evaluate("Boolean(document.querySelector('.danger-button'))"),
          10_000,
          "restored compatibility warning",
        );
        await cdp.evaluate("document.querySelector('.danger-button').click()", false);
      }

      await waitFor(
        () => cdp.evaluate("Boolean(document.querySelector('.official-button:not([disabled])'))"),
        20_000,
        `${mode} login preparation`,
      );
      await cdp.evaluate("document.querySelector('.official-button').click()", false);

      const expectedRoute =
        mode === "compatible" ? "WebView 本地 CONNECT 代理" : "WebView 系统网络";
      try {
        await waitFor(
          () =>
            cdp.evaluate(
              `document.querySelector('.launch-result')?.textContent.includes(${JSON.stringify(expectedRoute)}) === true`,
            ),
          25_000,
          `${mode} launch result`,
        );
      } catch (error) {
        const pageState = await cdp.evaluate(`({
          launchResult: document.querySelector('.launch-result')?.textContent ?? null,
          notice: document.querySelector('.notice')?.textContent ?? null,
          officialButtonText: document.querySelector('.official-button')?.textContent ?? null,
          officialButtonDisabled: document.querySelector('.official-button')?.disabled ?? null,
          bodyText: document.body.innerText.slice(0, 1200)
        })`);
        throw new Error(`${mode} launch state: ${JSON.stringify(pageState)}`, { cause: error });
      }

    } finally {
      cdp.close();
    }

    console.log(`PASS ${mode}: official login WebView created.`);
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
}

async function reservePort() {
  const server = net.createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

async function readTargets(port) {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/json`);
    if (!response.ok) return [];
    return await response.json();
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

  function send(method, params = {}) {
    const id = ++sequence;
    return new Promise((resolve, reject) => {
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }

  return {
    async navigate(url) {
      const response = await send("Page.navigate", { url });
      if (response.errorText) {
        throw new Error(`CDP navigation failed: ${response.errorText}`);
      }
    },
    async evaluate(expression, returnByValue = true) {
      const response = await send("Runtime.evaluate", {
        expression,
        awaitPromise: true,
        returnByValue,
      });
      if (response.exceptionDetails) {
        throw new Error(
          response.exceptionDetails.exception?.description ??
            response.exceptionDetails.text ??
            "CDP evaluation failed",
        );
      }
      return returnByValue ? response.result.value : response.result;
    },
    close() {
      socket.close();
    },
  };
}
