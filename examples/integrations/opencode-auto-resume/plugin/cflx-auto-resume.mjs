// OpenCode plugin: bind a cflx enqueue to the session that asked for it.
//
// # The problem it solves
//
// MCP does not tell a server which OpenCode session called it, and asking the
// model to construct a callback command is prompt-compliance, not engineering.
// A plugin is the one place that sees both halves: the tool result (which
// carries the execution binding) and the session ID (which OpenCode passes to
// every hook). So the plugin — not the model — registers the sink.
//
// # What it does not do
//
// It does not rewrite tool results, inspect unrelated tools, parse shell
// commands, or interpret the model's text. It filters one exact tool name,
// reads three fields out of a versioned envelope, and makes one call.

import { callTool } from "../lib/cflx-mcp.mjs";
import { composeMessage, resumeSession } from "../lib/resume.mjs";

/** Tool name suffix this plugin reacts to, after any MCP server prefix. */
const ENQUEUE_TOOL = "cflx_enqueue";

/** How often the owner-continuity observer checks, in milliseconds. */
const OBSERVER_INTERVAL_MS = 60_000;

/** How long it keeps checking before giving up, in milliseconds. */
const OBSERVER_LIFETIME_MS = 4 * 60 * 60 * 1000;

/**
 * Whether an OpenCode tool name is the cflx enqueue tool.
 *
 * Segment-exact rather than a substring test: OpenCode namespaces MCP tools as
 * `<server>_<tool>`, and a plugin that matched `includes("cflx_enqueue")` would
 * also fire for someone else's `my_cflx_enqueue_wrapper`.
 *
 * @param {string} name
 * @returns {boolean}
 */
export function isEnqueueTool(name) {
  return typeof name === "string" && (name === ENQUEUE_TOOL || name.endsWith(`_${ENQUEUE_TOOL}`));
}

/**
 * Pull the execution binding out of a tool result, or return null.
 *
 * Fails closed: an unsuccessful outcome, a missing execution ID, or anything
 * that is not the versioned envelope produces no registration at all.
 *
 * @param {unknown} output
 * @returns {{change_id: string, execution_id: string, instance_id: string} | null}
 */
export function extractBinding(output) {
  let envelope = output;
  if (typeof envelope === "string") {
    try {
      envelope = JSON.parse(envelope);
    } catch {
      return null;
    }
  }
  if (!envelope || typeof envelope !== "object") return null;
  if (envelope.ok !== true) return null;
  if (envelope.operation !== "enqueue") return null;
  const { change_id, execution_id, instance_id } = envelope;
  if (!change_id || !execution_id || !instance_id) return null;
  return { change_id, execution_id, instance_id };
}

/**
 * The plugin factory OpenCode loads.
 *
 * @param {{client?: object, directory?: string, worktree?: string}} context
 */
export const CflxAutoResume = async (context = {}) => {
  const config = {
    cflx: process.env.CFLX_BIN ?? "cflx",
    unixSocket: process.env.CFLX_UNIX_SOCKET,
    authTokenEnv: process.env.CFLX_AUTH_TOKEN_ENV,
    server: process.env.OPENCODE_SERVER ?? "http://127.0.0.1:4096",
    callback:
      process.env.CFLX_RESUME_CALLBACK ??
      new URL("../callback/cflx-resume-session.mjs", import.meta.url).pathname,
    state: process.env.CFLX_RESUME_STATE,
    node: process.execPath,
    cwd: context.worktree ?? context.directory ?? process.cwd(),
  };

  const observers = new Map();

  /**
   * Watch for the one failure mode owner-side delivery cannot cover.
   *
   * A crashed owner has no process left to run a callback, so its registrations
   * die silently. The observer is deliberately low-frequency and bounded: it is
   * a safety net for a rare event, not a poll loop standing in for the push.
   */
  function observeOwnerContinuity(binding, sessionID) {
    const key = `${binding.instance_id}/${binding.execution_id}`;
    if (observers.has(key)) return;
    const started = Date.now();
    const timer = setInterval(async () => {
      if (Date.now() - started > OBSERVER_LIFETIME_MS) {
        stop();
        return;
      }
      let envelope;
      try {
        envelope = await callTool(
          "cflx_notify_get",
          {
            change_id: binding.change_id,
            execution_id: binding.execution_id,
            instance_id: binding.instance_id,
          },
          config,
        );
      } catch {
        return; // A transient failure is not evidence about the owner.
      }
      // Only these two say the owner this execution belonged to is gone. Every
      // other outcome — including an ordinary refusal — is left alone, and none
      // of them is ever reported as completion.
      if (
        envelope.outcome !== "owner_restarted" &&
        envelope.outcome !== "owner_not_running"
      ) {
        if (envelope.detail?.terminal_dispatched === true) stop();
        return;
      }
      stop();
      try {
        await resumeSession({
          server: config.server,
          session: sessionID,
          text: composeMessage({
            event_type: "owner_restarted",
            change_id: binding.change_id,
            execution_id: binding.execution_id,
            instance_id: binding.instance_id,
          }),
        });
      } catch (error) {
        process.stderr.write(`cflx-auto-resume: ${error.message}\n`);
      }
    }, OBSERVER_INTERVAL_MS);
    // Never hold the process open for this.
    timer.unref?.();
    function stop() {
      clearInterval(timer);
      observers.delete(key);
    }
    observers.set(key, stop);
  }

  return {
    "tool.execute.after": async (input, output) => {
      if (!isEnqueueTool(input?.tool)) return;
      const sessionID = input?.sessionID;
      if (!sessionID) return;

      const binding = extractBinding(
        output?.output ?? output?.metadata?.structuredContent ?? output,
      );
      if (!binding) return;

      const command = [
        config.node,
        config.callback,
        "--server",
        config.server,
        "--session",
        sessionID,
      ];
      if (config.state) command.push("--state", config.state);

      try {
        const envelope = await callTool(
          "cflx_notify_set",
          { ...binding, command },
          config,
        );
        if (envelope.ok !== true) {
          process.stderr.write(
            `cflx-auto-resume: notify_set returned ${envelope.outcome}\n`,
          );
          return;
        }
      } catch (error) {
        process.stderr.write(`cflx-auto-resume: ${error.message}\n`);
        return;
      }
      observeOwnerContinuity(binding, sessionID);
    },
  };
};

export default CflxAutoResume;
