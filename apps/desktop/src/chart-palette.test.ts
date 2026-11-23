import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * MASTER §8: the categorical chart tokens stay legible marks (WCAG 1.4.11, ≥ 3:1) on
 * `card` and `background` in both appearances, and never reuse a status or brand color.
 * Colors only — no money is computed here.
 */

const css = readFileSync(join(import.meta.dirname, "index.css"), "utf8");

type Tokens = Record<string, string>;

function tokens(block: string): Tokens {
  const out: Tokens = {};
  for (const [, name, hex] of block.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{6});/gi)) {
    if (name && hex) out[name] = hex.toLowerCase();
  }
  return out;
}

const darkStart = css.indexOf("@media (prefers-color-scheme: dark)");
const light = tokens(css.slice(css.indexOf(":root"), darkStart));
const dark = { ...light, ...tokens(css.slice(darkStart, css.indexOf("@theme"))) };

function hex(t: Tokens, name: string): string {
  const value = t[name];
  if (value === undefined) throw new Error(`--${name} is not defined in index.css`);
  return value;
}

function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

const series = ["chart-1", "chart-2", "chart-3"];
const reserved = ["success", "destructive", "warning", "brand", "primary"];

describe.each([
  ["light", light],
  ["dark", dark],
])("chart palette (%s)", (_mode, t) => {
  it.each(series)("%s is defined", (name) => {
    expect(hex(t, name)).toMatch(/^#[0-9a-f]{6}$/);
  });

  it.each(series.flatMap((s) => ["card", "background"].map((surface) => [s, surface])))(
    "%s on %s is at least 3:1",
    (name, surface) => {
      expect(contrast(hex(t, name), hex(t, surface))).toBeGreaterThanOrEqual(3);
    },
  );

  it("reuses no status, brand or primary color", () => {
    const taken = new Set(reserved.map((r) => hex(t, r)));
    for (const s of series) expect(taken.has(hex(t, s))).toBe(false);
  });

  it("has distinct series colors", () => {
    expect(new Set(series.map((s) => hex(t, s))).size).toBe(series.length);
  });
});
