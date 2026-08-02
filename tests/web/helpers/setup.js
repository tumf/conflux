/**
 * Global setup for the operator-console browser tests.
 *
 * `web/app.js` autostarts against the real document when it is loaded in a
 * browser. The specs drive their own instances, so the autostart is switched off
 * before any spec imports the module.
 */

globalThis.__CFLX_NO_AUTOSTART__ = true;

// jsdom does not ship `Headers`. Node provides one; the tiny fallback below
// keeps the suite runnable on a runtime that provides neither.
if (typeof globalThis.Headers === 'undefined') {
  globalThis.Headers = class Headers {
    constructor(init) {
      this.map = new Map();
      if (init instanceof globalThis.Headers) {
        for (const [key, value] of init.map) this.map.set(key, value);
      } else if (init && typeof init === 'object') {
        for (const [key, value] of Object.entries(init)) this.set(key, value);
      }
    }
    set(key, value) {
      this.map.set(String(key).toLowerCase(), String(value));
    }
    get(key) {
      return this.map.get(String(key).toLowerCase()) ?? null;
    }
    has(key) {
      return this.map.has(String(key).toLowerCase());
    }
    entries() {
      return this.map.entries();
    }
    [Symbol.iterator]() {
      return this.map.entries();
    }
  };
}
