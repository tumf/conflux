/**
 * Static analysis of the shipped stylesheet.
 *
 * jsdom has no layout engine, so these checks read the CSS source rather than a
 * rendered box: which custom properties exist, which are referenced, what the
 * measured contrast of each declared pair is, and which layout rules are
 * present. That is the part of the visual contract a repository-local suite can
 * verify honestly, and it is the part that regresses when a token is renamed or
 * a colour is nudged.
 */

/** Extract the balanced body of the first at-rule whose prelude contains `needle`. */
export function sliceAtRule(css, needle) {
  const start = css.indexOf(needle);
  if (start === -1) return null;
  const open = css.indexOf('{', start);
  if (open === -1) return null;
  let depth = 0;
  for (let index = open; index < css.length; index += 1) {
    if (css[index] === '{') depth += 1;
    else if (css[index] === '}') {
      depth -= 1;
      if (depth === 0) return css.slice(open + 1, index);
    }
  }
  return null;
}

/** All `--name: value` declarations inside every `:root` block of a source. */
export function customProperties(css) {
  const properties = new Map();
  const blocks = /:root\s*\{([\s\S]*?)\}/g;
  let block = blocks.exec(css);
  while (block !== null) {
    const declarations = /(--[A-Za-z0-9-]+)\s*:\s*([^;]+);/g;
    let declaration = declarations.exec(block[1]);
    while (declaration !== null) {
      properties.set(declaration[1], declaration[2].trim());
      declaration = declarations.exec(block[1]);
    }
    block = blocks.exec(css);
  }
  return properties;
}

/** Every custom property referenced through `var()` anywhere in the source. */
export function referencedProperties(css) {
  const referenced = new Set();
  const pattern = /var\(\s*(--[A-Za-z0-9-]+)/g;
  let match = pattern.exec(css);
  while (match !== null) {
    referenced.add(match[1]);
    match = pattern.exec(css);
  }
  return referenced;
}

/** Resolve a token through any chain of `var()` indirections. */
export function resolveToken(properties, name, seen = new Set()) {
  if (seen.has(name)) return null;
  seen.add(name);
  const value = properties.get(name);
  if (value === undefined) return null;
  const indirect = /^var\(\s*(--[A-Za-z0-9-]+)\s*\)$/.exec(value);
  if (indirect) return resolveToken(properties, indirect[1], seen);
  return value;
}

function channel(value) {
  const srgb = value / 255;
  return srgb <= 0.04045 ? srgb / 12.92 : ((srgb + 0.055) / 1.055) ** 2.4;
}

/** Relative luminance of a `#rgb` or `#rrggbb` colour. */
export function luminance(hex) {
  const normalized = hex.trim().replace('#', '');
  const full =
    normalized.length === 3
      ? normalized
          .split('')
          .map((part) => part + part)
          .join('')
      : normalized;
  const r = Number.parseInt(full.slice(0, 2), 16);
  const g = Number.parseInt(full.slice(2, 4), 16);
  const b = Number.parseInt(full.slice(4, 6), 16);
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** WCAG contrast ratio between two hex colours. */
export function contrast(foreground, background) {
  const a = luminance(foreground);
  const b = luminance(background);
  const light = Math.max(a, b);
  const dark = Math.min(a, b);
  return (light + 0.05) / (dark + 0.05);
}

/**
 * The two shipped themes, as resolved token maps.
 *
 * @param {string} css
 * @returns {{dark: Map<string,string>, light: Map<string,string>}}
 */
export function themes(css) {
  const lightBody = sliceAtRule(css, '@media (prefers-color-scheme: light)');
  const contrastBody = sliceAtRule(css, '@media (prefers-contrast: more)');
  // Preference blocks redefine tokens conditionally, so they must not leak into
  // the default theme's map.
  let defaults = css;
  for (const body of [lightBody, contrastBody]) {
    if (body) defaults = defaults.replace(body, '');
  }
  const dark = customProperties(defaults);
  const light = new Map(dark);
  if (lightBody) {
    for (const [name, value] of customProperties(lightBody)) light.set(name, value);
  }
  return { dark, light };
}
