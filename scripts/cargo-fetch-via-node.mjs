import { spawn } from "node:child_process";
import http from "node:http";

const MAX_RESPONSE_BYTES = 64 * 1024 * 1024;
const MIRROR_PORT = 10425;
const server = http.createServer(async (request, response) => {
  try {
    const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
    if (request.method !== "GET") return send(response, 405, "method not allowed");
    if (requestUrl.pathname === "/index/config.json") {
      const port = server.address().port;
      return sendJson(response, {
        dl: `http://127.0.0.1:${port}/crates/{crate}/{version}/download`,
      });
    }

    const indexPath = requestUrl.pathname.match(/^\/index\/([a-z0-9_+\/-]+)$/)?.[1];
    if (indexPath && !indexPath.includes("..")) {
      return forward(response, `https://index.crates.io/${indexPath}`);
    }

    const cratePath = requestUrl.pathname.match(
      /^\/crates\/([a-zA-Z0-9_-]+)\/([a-zA-Z0-9.+_-]+)\/download$/,
    );
    if (cratePath) {
      const [, crateName, version] = cratePath;
      return forward(
        response,
        `https://static.crates.io/crates/${crateName}/${crateName}-${version}.crate`,
      );
    }
    return send(response, 404, "not found");
  } catch (error) {
    return send(response, 502, error instanceof Error ? error.message : "upstream failure");
  }
});

server.listen(MIRROR_PORT, "127.0.0.1", async () => {
  const port = server.address().port;
  console.error(`cargo mirror listening on 127.0.0.1:${port}`);
  const cargo = spawn(
    "cargo",
    [
      "fetch",
      "-vv",
      "--config",
      'source.crates-io.replace-with="node-proxy"',
      "--config",
      `source.node-proxy.registry="sparse+http://127.0.0.1:${port}/index/"`,
    ],
    { stdio: "inherit", windowsHide: true },
  );
  cargo.once("error", (error) => {
    console.error(error);
    server.close(() => process.exitCode = 1);
  });
  cargo.once("exit", (code) => {
    server.close(() => process.exitCode = code ?? 1);
  });
});

async function forward(response, url) {
  const upstream = await fetchWithRetry(url);
  if (!upstream.ok) return send(response, upstream.status, `upstream ${upstream.status}`);
  const length = Number(upstream.headers.get("content-length") ?? 0);
  if (length > MAX_RESPONSE_BYTES) return send(response, 413, "upstream response too large");
  const bytes = Buffer.from(await upstream.arrayBuffer());
  if (bytes.length > MAX_RESPONSE_BYTES) return send(response, 413, "upstream response too large");
  response.writeHead(200, {
    "content-type": upstream.headers.get("content-type") ?? "application/octet-stream",
    "content-length": String(bytes.length),
    etag: upstream.headers.get("etag") ?? "",
  });
  response.end(bytes);
}

async function fetchWithRetry(url) {
  let lastError;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      return await fetch(url, {
        redirect: "follow",
        signal: AbortSignal.timeout(20_000),
      });
    } catch (error) {
      lastError = error;
      console.error(`cargo mirror upstream retry ${attempt}: ${new URL(url).pathname}`);
    }
  }
  throw lastError;
}

function sendJson(response, value) {
  const body = Buffer.from(JSON.stringify(value));
  response.writeHead(200, {
    "content-type": "application/json",
    "content-length": String(body.length),
  });
  response.end(body);
}

function send(response, status, message) {
  const body = Buffer.from(message);
  response.writeHead(status, {
    "content-type": "text/plain; charset=utf-8",
    "content-length": String(body.length),
  });
  response.end(body);
}
