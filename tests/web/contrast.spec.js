/**
 * Colour and token integrity for both shipped themes.
 *
 * Every pair below is a pair the console actually renders, so a token change
 * that drops normal text under 4.5:1 or a component/focus indicator under 3:1
 * fails here rather than in an audit.
 */

import { describe, expect, it } from 'vitest';

import { STYLE_CSS } from './helpers/console.js';
import { contrast, customProperties, referencedProperties, resolveToken, themes } from './helpers/css.js';

const { dark, light } = themes(STYLE_CSS);

/** Normal-size text: WCAG 2.2 AA requires 4.5:1. */
const TEXT_PAIRS = [
  ['--color-text', '--color-bg'],
  ['--color-text', '--color-surface'],
  ['--color-text', '--color-surface-alt'],
  ['--color-text-muted', '--color-bg'],
  ['--color-text-muted', '--color-surface'],
  ['--color-text-muted', '--color-surface-alt'],
  ['--color-on-primary', '--color-primary'],
  ['--color-on-danger', '--color-danger'],
  ['--color-status-attention', '--color-surface'],
  ['--color-status-attention', '--color-surface-alt'],
  ['--color-status-active', '--color-surface'],
  ['--color-status-active', '--color-surface-alt'],
  ['--color-status-waiting', '--color-surface'],
  ['--color-status-waiting', '--color-surface-alt'],
  ['--color-status-completed', '--color-surface'],
  ['--color-status-completed', '--color-surface-alt'],
  ['--ansi-black', '--color-surface-alt'],
  ['--ansi-red', '--color-surface-alt'],
  ['--ansi-green', '--color-surface-alt'],
  ['--ansi-yellow', '--color-surface-alt'],
  ['--ansi-blue', '--color-surface-alt'],
  ['--ansi-magenta', '--color-surface-alt'],
  ['--ansi-cyan', '--color-surface-alt'],
  ['--ansi-white', '--color-surface-alt'],
];

/** Component boundaries, focus indicators, and meters: WCAG 2.2 AA requires 3:1. */
const COMPONENT_PAIRS = [
  ['--color-border', '--color-bg'],
  ['--color-border', '--color-surface'],
  ['--color-border', '--color-surface-alt'],
  ['--color-focus', '--color-bg'],
  ['--color-focus', '--color-surface'],
  ['--color-focus', '--color-surface-alt'],
  ['--color-primary', '--color-surface'],
  ['--color-primary', '--color-surface-alt'],
  ['--color-danger', '--color-surface'],
];

function ratio(tokens, foreground, background) {
  const fg = resolveToken(tokens, foreground);
  const bg = resolveToken(tokens, background);
  expect(fg, `${foreground} must be defined`).toBeTruthy();
  expect(bg, `${background} must be defined`).toBeTruthy();
  return contrast(fg, bg);
}

describe.each([
  ['default (dark)', dark],
  ['prefers-color-scheme: light', light],
])('%s theme', (_name, tokens) => {
  it.each(TEXT_PAIRS)('%s on %s reaches 4.5:1', (foreground, background) => {
    expect(ratio(tokens, foreground, background)).toBeGreaterThanOrEqual(4.5);
  });

  it.each(COMPONENT_PAIRS)('%s on %s reaches 3:1', (foreground, background) => {
    expect(ratio(tokens, foreground, background)).toBeGreaterThanOrEqual(3);
  });
});

describe('token integrity', () => {
  it('defines every custom property the stylesheet references', () => {
    const defined = customProperties(STYLE_CSS);
    const missing = Array.from(referencedProperties(STYLE_CSS)).filter(
      (name) => !defined.has(name),
    );
    expect(missing).toEqual([]);
  });

  it('references every custom property it defines', () => {
    const defined = customProperties(STYLE_CSS);
    const referenced = referencedProperties(STYLE_CSS);
    const unused = Array.from(defined.keys()).filter((name) => !referenced.has(name));
    expect(unused).toEqual([]);
  });

  it('gives the light theme the same token surface as the default theme', () => {
    const colourTokens = (tokens) =>
      Array.from(tokens.keys())
        .filter((name) => name.startsWith('--color-') || name.startsWith('--ansi-'))
        .sort();
    expect(colourTokens(light)).toEqual(colourTokens(dark));
  });
});

describe('motion and focus rules', () => {
  it('never animates every property at once', () => {
    expect(STYLE_CSS).not.toMatch(/transition:\s*all/);
  });

  it('only transitions compositable or layout-safe properties', () => {
    const transitions = Array.from(STYLE_CSS.matchAll(/transition:\s*([^;]+);/g)).map((match) =>
      match[1].replace(/\s+/g, ' ').trim(),
    );
    expect(transitions.length).toBeGreaterThan(0);
    for (const declaration of transitions) {
      for (const part of declaration.split(',')) {
        const property = part.trim().split(/\s+/)[0];
        expect(['opacity', 'transform', 'background-color', 'color', 'top']).toContain(property);
      }
    }
  });

  it('never suppresses focus without a visible replacement', () => {
    const suppressions = Array.from(STYLE_CSS.matchAll(/outline:\s*(none|0)\s*;/g));
    expect(suppressions).toHaveLength(0);
    expect(STYLE_CSS).toMatch(/:focus-visible\s*\{[^}]*outline:\s*var\(--focus-width\)/);
  });

  it('honours reduced-motion and increased-contrast preferences', () => {
    expect(STYLE_CSS).toMatch(/@media \(prefers-reduced-motion: reduce\)/);
    expect(STYLE_CSS).toMatch(/@media \(prefers-contrast: more\)/);
  });
});
