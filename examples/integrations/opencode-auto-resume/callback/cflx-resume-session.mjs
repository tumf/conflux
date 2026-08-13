#!/usr/bin/env node
// The argv Conflux runs when one execution reaches a terminal classification.
//
// Registered with `cflx_notify_set` as, for example:
//
//   ["/usr/bin/env", "node", "<this file>",
//    "--server", "http://127.0.0.1:4096", "--session", "ses_abc",
//    "--state", "/tmp/cflx-auto-resume"]
//
// The owner passes the event through exactly five environment variables and one
// file. Nothing else is inherited — no PATH, no HOME, no owner token — so the
// interpreter is named explicitly in argv.
//
// # This is untrusted data
//
// The event file is data the callback reads and forwards. It is never executed,
// never `eval`'d, and never used to choose a destination: the destination comes
// from the operator's own registration and must be loopback.

import { readFileSync, mkdirSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { composeMessage, resumeSession } from "../lib/resume.mjs";

/** Payload versions this callback understands. */
const SUPPORTED_SCHEMA_VERSIONS = new Set([1]);

function parseArgs(argv) {
  const args = { path: "/session/{session}/message" };
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i];
    const value = argv[i + 1];
    switch (key) {
      case "--server":
        args.server = value;
        break;
      case "--session":
        args.session = value;
        break;
      case "--state":
        args.state = value;
        break;
      case "--path":
        args.path = value;
        break;
      default:
        throw new Error(`unknown argument '${key}'`);
    }
  }
  if (!args.server) throw new Error("--server is required");
  if (!args.session) throw new Error("--session is required");
  if (!args.state) args.state = join(tmpdir(), "cflx-auto-resume");
  return args;
}

/**
 * Read the event file and check it against the environment the owner set.
 *
 * The cross-check matters: the environment names the execution this delivery is
 * for, and a file whose contents disagree is not the payload for this call.
 */
function readEvent() {
  const path = process.env.CFLX_EVENT_PATH;
  if (!path) throw new Error("CFLX_EVENT_PATH is not set");
  const event = JSON.parse(readFileSync(path, "utf8"));

  if (!SUPPORTED_SCHEMA_VERSIONS.has(event.schema_version)) {
    throw new Error(
      `event schema version ${event.schema_version} is not one this callback reads`,
    );
  }
  const expected = {
    event_type: process.env.CFLX_EVENT_TYPE,
    execution_id: process.env.CFLX_EXECUTION_ID,
    change_id: process.env.CFLX_CHANGE_ID,
    instance_id: process.env.CFLX_INSTANCE_ID,
  };
  for (const [field, value] of Object.entries(expected)) {
    if (value && event[field] !== value) {
      throw new Error(
        `the event file's ${field} does not match the delivery's own ${field}`,
      );
    }
  }
  return event;
}

/**
 * Deduplicate locally by execution and event type.
 *
 * The owner already delivers a terminal at most once per execution, but a
 * callback can be re-run by an operator, and a duplicate resume would look to
 * the agent like a second completion.
 */
function claim(stateDir, event) {
  mkdirSync(stateDir, { recursive: true, mode: 0o700 });
  const marker = join(
    stateDir,
    `${event.execution_id}.${event.event_type}.done`.replace(/[^A-Za-z0-9._-]/g, "_"),
  );
  if (existsSync(marker)) return false;
  writeFileSync(marker, `${new Date().toISOString()}\n`, { mode: 0o600 });
  return true;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const event = readEvent();
  if (!claim(args.state, event)) {
    process.stderr.write(
      `cflx-resume-session: ${event.execution_id}/${event.event_type} was already delivered\n`,
    );
    return;
  }
  await resumeSession({
    server: args.server,
    session: args.session,
    text: composeMessage(event),
    path: args.path,
  });
}

main().catch((error) => {
  // Delivery is observability. A non-zero exit is recorded by the owner as a
  // bounded diagnostic and changes nothing about the change itself.
  process.stderr.write(`cflx-resume-session: ${error.message}\n`);
  process.exitCode = 1;
});
