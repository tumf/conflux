// Loopback-only destination policy, shared by the callback and the plugin.
//
// The callback is handed a destination that ultimately came from a plugin
// running next to an agent, and it POSTs a prompt to it. Allowing an arbitrary
// host would turn "a change finished" into an outbound request to anywhere, so
// the policy is narrow and lives in one place: only a loopback address, only
// http, and only a port.

const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1", "[::1]"]);

/**
 * Parse and validate an OpenCode server base URL.
 *
 * @param {string} value
 * @returns {URL} the validated URL
 * @throws {Error} when the destination is not a plain loopback HTTP endpoint
 */
export function requireLoopback(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`'${value}' is not a URL`);
  }
  if (url.protocol !== "http:") {
    throw new Error(
      `only http:// is accepted for a local OpenCode server, got '${url.protocol}'`,
    );
  }
  if (!LOOPBACK_HOSTS.has(url.hostname) && !LOOPBACK_HOSTS.has(url.host)) {
    throw new Error(
      `only loopback destinations are accepted, got '${url.hostname}'. ` +
        `A completion callback must not be able to reach the network`,
    );
  }
  if (url.username || url.password) {
    throw new Error("credentials in the destination URL are not accepted");
  }
  return url;
}

/**
 * An OpenCode session ID is an opaque handle. Keep it to a shape that is safe
 * to interpolate into a path, so a hostile value cannot traverse one.
 *
 * @param {string} value
 * @returns {string}
 */
export function requireSessionId(value) {
  if (typeof value !== "string" || !/^[A-Za-z0-9._-]{1,128}$/.test(value)) {
    throw new Error(
      `'${value}' is not an OpenCode session ID: 1-128 characters of [A-Za-z0-9._-]`,
    );
  }
  return value;
}

/**
 * Resolve a callback path against a validated base and prove it stayed home.
 *
 * The base alone is not the security boundary — the *final* URL is. `new URL`
 * lets a path replace everything before it, so an absolute `--path`, a
 * protocol-relative `//host`, or a backslash variant (the WHATWG parser folds
 * `\` to `/` for http) would silently retarget a callback that passed
 * `requireLoopback` on its base. Those shapes are rejected by inspection, and
 * then the resolved origin is compared to the base's and re-validated as
 * loopback, so anything the inspection missed still cannot leave.
 *
 * @param {string | URL} base an OpenCode server base URL
 * @param {string} path the callback path to resolve against it
 * @returns {URL} the resolved, same-origin, loopback URL
 * @throws {Error} when the resolved destination is not the base's own origin
 */
export function resolveLoopbackTarget(base, path) {
  const validated = requireLoopback(base instanceof URL ? base.href : base);
  if (typeof path !== "string" || path.length === 0) {
    throw new Error("a callback path is required");
  }
  // Fold the backslash variants into the shapes below rather than testing for
  // them separately: `/\evil`, `\\evil`, and `//evil` are one attack.
  const folded = path.replace(/\\/g, "/");
  if (/^[A-Za-z][A-Za-z0-9+.-]*:/.test(folded)) {
    throw new Error(
      `a callback path must not carry a scheme, got '${path}'. ` +
        `The destination comes from the operator's registration, not from the path`,
    );
  }
  if (folded.startsWith("//")) {
    throw new Error(
      `a callback path must not be protocol-relative, got '${path}'`,
    );
  }

  let target;
  try {
    target = new URL(path, validated);
  } catch {
    throw new Error(`'${path}' is not a usable callback path`);
  }
  if (target.origin !== validated.origin) {
    throw new Error(
      `a callback path must not change origin: '${path}' resolves to ` +
        `'${target.origin}', not '${validated.origin}'`,
    );
  }
  // Belt and braces: the resolved URL is validated on its own terms too.
  requireLoopback(target.href);
  return target;
}
