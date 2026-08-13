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
