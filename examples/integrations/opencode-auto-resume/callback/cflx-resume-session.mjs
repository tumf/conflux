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

import {
  existsSync,
  linkSync,
  mkdirSync,
  readFileSync,
  renameSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { randomUUID } from "node:crypto";
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

/** How long an in-flight claim is honoured before a later invocation may take it. */
const STALE_CLAIM_MS = 5 * 60 * 1000;

/** Exit status for "another process holds a fresh claim", distinct from a failure. */
const EXIT_IN_FLIGHT = 75;

/**
 * The two files one execution event can have, and what each one means.
 *
 * `.inflight` is a *claim*: someone is trying. `.done` is *delivery*: the POST
 * returned success. Keeping them apart is the whole point — the old single
 * marker was written before the POST, so a failed delivery permanently
 * suppressed every later retry while a concurrent pair could still both send.
 *
 * @param {string} stateDir
 * @param {{execution_id: string, event_type: string}} event
 */
function paths(stateDir, event) {
  const key = `${event.execution_id}.${event.event_type}`.replace(
    /[^A-Za-z0-9._-]/g,
    "_",
  );
  return {
    inflight: join(stateDir, `${key}.inflight`),
    done: join(stateDir, `${key}.done`),
  };
}

/**
 * Try to become the one process delivering this event.
 *
 * The owner already caps terminal delivery at one attempt per execution, but a
 * callback can be re-run by an operator or raced by a second registration, and
 * a duplicate resume would look to the agent like a second completion.
 *
 * Exclusive creation (`wx`) is the atomic primitive: check-then-write let two
 * processes both pass the check. Taking over a stale claim uses a rename as a
 * compare-and-swap — the first process to rename the claim away owns it, and
 * every other one gets `ENOENT`.
 *
 * @param {string} stateDir
 * @param {{execution_id: string, event_type: string}} event
 * @param {number} [now]
 * @returns {{state: "delivered" | "in_flight" | "claimed", owner?: string,
 *            inflight: string, done: string}}
 */
function claimDelivery(stateDir, event, now = Date.now()) {
  mkdirSync(stateDir, { recursive: true, mode: 0o700 });
  const { inflight, done } = paths(stateDir, event);
  if (existsSync(done)) return { state: "delivered", inflight, done };

  const owner = tryClaim(inflight, now);
  if (owner) return { state: "claimed", owner, inflight, done };

  let held;
  try {
    held = statSync(inflight);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    // It was released between our attempt and this look. One retry, then defer:
    // an unbounded loop here would be the retry policy this callback must not have.
    const second = tryClaim(inflight, now);
    return second
      ? { state: "claimed", owner: second, inflight, done }
      : { state: "in_flight", inflight, done };
  }
  if (now - held.mtimeMs < STALE_CLAIM_MS) {
    return { state: "in_flight", inflight, done };
  }

  // Stale. Rename is the compare-and-swap: exactly one process moves this name.
  const taken = `${inflight}.${randomUUID()}.taken`;
  try {
    renameSync(inflight, taken);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    return { state: "in_flight", inflight, done };
  }
  // We hold it exclusively now, so its age can be read without a race. If it
  // turned fresh between the stat and the rename we took someone's live claim;
  // put it back if the slot is still free and stand down either way.
  if (now - statSync(taken).mtimeMs < STALE_CLAIM_MS) {
    try {
      linkSync(taken, inflight);
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
    }
    unlinkSync(taken);
    return { state: "in_flight", inflight, done };
  }
  unlinkSync(taken);
  const seized = tryClaim(inflight, now);
  return seized
    ? { state: "claimed", owner: seized, inflight, done }
    : { state: "in_flight", inflight, done };
}

/**
 * Create the claim exclusively, returning our owner token or null if it exists.
 *
 * @param {string} inflight
 * @param {number} now
 * @returns {string | null}
 */
function tryClaim(inflight, now) {
  const owner = randomUUID();
  try {
    writeFileSync(
      inflight,
      `${JSON.stringify({ owner, claimed_at: new Date(now).toISOString() })}\n`,
      { flag: "wx", mode: 0o600 },
    );
    return owner;
  } catch (error) {
    if (error.code === "EEXIST") return null;
    throw error;
  }
}

/**
 * Promote our claim to durable evidence that the POST succeeded.
 *
 * A rename, so there is no window in which the marker exists half-written.
 *
 * @param {{owner: string, inflight: string, done: string}} claim
 */
function markDelivered(claim) {
  try {
    renameSync(claim.inflight, claim.done);
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
    // Our claim was taken over while we were posting. The delivery still
    // happened, so the marker is still the truth.
    writeFileSync(claim.done, `${new Date().toISOString()}\n`, { mode: 0o600 });
  }
}

/**
 * Release our claim after a failed POST so a later invocation may retry.
 *
 * Only ours: a claim carrying a different owner token belongs to whoever took
 * this one over as stale, and removing it would defeat their delivery.
 *
 * @param {{owner: string, inflight: string}} claim
 */
function releaseClaim(claim) {
  try {
    const held = JSON.parse(readFileSync(claim.inflight, "utf8"));
    if (held.owner !== claim.owner) return;
    unlinkSync(claim.inflight);
  } catch {
    // Already gone, or unreadable. Either way there is nothing of ours to release.
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const event = readEvent();
  const claim = claimDelivery(args.state, event);
  const subject = `${event.execution_id}/${event.event_type}`;

  if (claim.state === "delivered") {
    process.stderr.write(`cflx-resume-session: ${subject} was already delivered\n`);
    return;
  }
  if (claim.state === "in_flight") {
    // Distinct from failure on purpose: nothing went wrong, another process is
    // simply delivering this event. Retrying it here would be the duplicate.
    process.stderr.write(
      `cflx-resume-session: ${subject} is in flight in another process\n`,
    );
    process.exitCode = EXIT_IN_FLIGHT;
    return;
  }

  try {
    await resumeSession({
      server: args.server,
      session: args.session,
      text: composeMessage(event),
      path: args.path,
    });
  } catch (error) {
    // The claim must not outlive the attempt it stood for, or a single failed
    // POST would suppress delivery until the stale bound expires.
    releaseClaim(claim);
    throw error;
  }
  markDelivered(claim);
}

main().catch((error) => {
  // Delivery is observability. A non-zero exit is recorded by the owner as a
  // bounded diagnostic and changes nothing about the change itself.
  process.stderr.write(`cflx-resume-session: ${error.message}\n`);
  process.exitCode = 1;
});
