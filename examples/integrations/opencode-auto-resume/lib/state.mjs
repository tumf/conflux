// Where the callback keeps its delivery state, and why it checks the directory
// before it writes one byte into it.
//
// The `.inflight` claim and the `.done` marker are not bookkeeping: they decide
// whether a message is sent. Anyone who can create, replace, or read those
// files can suppress a delivery (pre-create the marker), hold one hostage
// (pre-create a claim that never goes stale), or watch which changes an
// operator is running. So the directory holding them has to be provably the
// invoking user's alone before a claim is read or created.
//
// A shared, predictable path under the system temporary directory could not
// give that: `/tmp` is world-writable, so the first user to create
// `/tmp/cflx-auto-resume` owns it for everyone else on the machine.

import { lstatSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

/** Permission bits nobody but the owning user may hold. */
const FOREIGN_ACCESS_BITS = 0o077;

/** Directory mode the callback creates and requires. */
export const PRIVATE_MODE = 0o700;

/**
 * Where delivery state lives when the operator names no directory.
 *
 * Inside the invoking user's own home, which no other unprivileged user may
 * write, so the default cannot be pre-created by somebody else. The callback
 * runs with a *replaced* environment — no `HOME` — and `os.homedir()` falls
 * back to the passwd entry for the real uid there, which is the same answer.
 *
 * @returns {string}
 * @throws {Error} when this user has no home directory to hold state in
 */
export function defaultStateDir() {
  const home = homedir();
  if (typeof home !== "string" || home.length === 0) {
    throw new Error(
      "this user has no home directory to hold callback delivery state; " +
        "pass --state <dir> naming an owner-private directory",
    );
  }
  return join(home, ".local", "state", "cflx", "opencode-auto-resume");
}

/**
 * Prove the state directory is a real directory that only we can reach,
 * creating it privately if it is absent.
 *
 * Created with mode 0700 from the outset rather than relaxed afterwards: a
 * directory that is briefly group-readable is a directory somebody had time to
 * read. The check that follows uses `lstat`, so a symlink is seen as a symlink
 * rather than as whatever it points at.
 *
 * @param {string} dir
 * @returns {string} the same directory, once it has been vouched for
 * @throws {Error} when the path is not an owner-private directory
 */
export function requirePrivateStateDir(dir) {
  if (typeof dir !== "string" || dir.length === 0) {
    throw new Error("a callback state directory is required");
  }

  let stats = lstatOrNull(dir);
  if (stats === null) {
    mkdirSync(dir, { recursive: true, mode: PRIVATE_MODE });
    stats = lstatOrNull(dir);
    if (stats === null) {
      throw new Error(`the callback state directory '${dir}' could not be created`);
    }
  }

  if (stats.isSymbolicLink()) {
    throw new Error(
      `the callback state path '${dir}' is a symlink; delivery state must be a ` +
        `real directory, because a link can be repointed after it is checked`,
    );
  }
  if (!stats.isDirectory()) {
    throw new Error(`the callback state path '${dir}' is not a directory`);
  }

  // Windows has no meaningful uid here, so ownership is checked where the
  // platform actually reports it rather than guessed at where it does not.
  const uid = typeof process.getuid === "function" ? process.getuid() : null;
  if (uid !== null && stats.uid !== uid) {
    throw new Error(
      `the callback state directory '${dir}' is owned by uid ${stats.uid}, ` +
        `not by this user (uid ${uid})`,
    );
  }
  if ((stats.mode & FOREIGN_ACCESS_BITS) !== 0) {
    throw new Error(
      `the callback state directory '${dir}' is group- or world-accessible ` +
        `(mode ${(stats.mode & 0o7777).toString(8)}); it must be 0700`,
    );
  }
  return dir;
}

/**
 * @param {string} path
 * @returns {import("node:fs").Stats | null} null when the path does not exist
 */
function lstatOrNull(path) {
  try {
    return lstatSync(path);
  } catch (error) {
    if (error.code === "ENOENT") return null;
    throw error;
  }
}
