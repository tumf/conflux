// A one-shot MCP client for `cflx client mcp`.
//
// Why spawn the server instead of adding a `cflx client notify` command: the
// client namespace is deliberately four commands, and the notify operations are
// tool-shaped rather than command-shaped. Spawning the same adapter an MCP host
// would use keeps exactly one implementation of routing, binding validation, and
// outcome mapping — this file only frames JSON-RPC.
//
// Everything here is bounded: one process, one request, one deadline, and the
// child is killed if it outlives it.

import { spawn } from "node:child_process";

/** Default ceiling for one tool call, in milliseconds. */
export const DEFAULT_TIMEOUT_MS = 20_000;

/**
 * Call one `cflx client mcp` tool and return its envelope.
 *
 * @param {string} name tool name, e.g. `cflx_notify_set`
 * @param {object} args tool arguments
 * @param {{cflx?: string, unixSocket?: string, authTokenEnv?: string, cwd?: string, timeoutMs?: number}} [options]
 * @returns {Promise<object>} the versioned client envelope
 */
export async function callTool(name, args, options = {}) {
  const {
    cflx = "cflx",
    unixSocket,
    authTokenEnv,
    cwd,
    timeoutMs = DEFAULT_TIMEOUT_MS,
  } = options;

  const argv = ["client"];
  if (unixSocket) argv.push("--unix-socket", unixSocket);
  // A token *name*, never a token value: argv is visible to anything that can
  // list processes.
  if (authTokenEnv) argv.push("--auth-token-env", authTokenEnv);
  argv.push("mcp");

  const child = spawn(cflx, argv, {
    cwd,
    stdio: ["pipe", "pipe", "pipe"],
  });

  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdout += chunk;
  });
  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });

  const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);

  const finished = new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });

  const frames = [
    {
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: { protocolVersion: "2025-06-18", capabilities: {} },
    },
    { jsonrpc: "2.0", method: "notifications/initialized", params: {} },
    {
      jsonrpc: "2.0",
      id: 2,
      method: "tools/call",
      params: { name, arguments: args },
    },
  ];
  child.stdin.write(frames.map((frame) => JSON.stringify(frame)).join("\n") + "\n");
  // Closing stdin is what ends the session: the adapter serves until EOF.
  child.stdin.end();

  try {
    await finished;
  } finally {
    clearTimeout(timer);
  }

  const response = stdout
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return null;
      }
    })
    .find((frame) => frame && frame.id === 2);

  if (!response) {
    throw new Error(
      `no result frame for '${name}'` + (stderr ? `: ${stderr.trim()}` : ""),
    );
  }
  if (response.error) {
    throw new Error(`'${name}' failed: ${response.error.message}`);
  }
  // The envelope is the contract. `structuredContent` is the same object the
  // text content carries, parsed.
  const envelope =
    response.result?.structuredContent ??
    JSON.parse(response.result?.content?.[0]?.text ?? "{}");
  return envelope;
}
