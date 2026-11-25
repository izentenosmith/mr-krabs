import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * MASTER §8.1: the categorical chart tokens stay legible marks (WCAG 1.4.11, ≥ 3:1) on every
 * surface a mark is drawn on, in both appearances, and stay clear of the reserved colors.
 * Colors only — no money is computed here.
 */

const css = readFileSync(join(import.meta.dirname, "index.css"), "utf8");

const series = ["chart-1", "chart-2", "chart-3"];
const surfaces = ["card", "background", "popover"];
/** Never equal to a chart color. */
const reserved = [
  "success",
  "destructive",
  "warning",
  "brand",
  "primary",
  "scope-personal",
  "scope-household",
];
/** Also kept perceptually apart (OKLab ΔE × 100 ≥ 15): the status and brand hues. */
const distant = ["success", "destructive", "warning", "brand"];

type Tokens = Record<string, string>;

/** Every `--name: value;` in a block, raw. */
function declarations(block: string): Tokens {
  const out: Tokens = {};
  for (const [, name, value] of block.matchAll(/--([a-z0-9-]+):\s*([^;]+);/gi)) {
    if (name && value) out[name] = value.trim().toLowerCase();
  }
  return out;
}

function section(start: string, end: string): string {
  const from = css.indexOf(start);
  const to = css.indexOf(end, from);
  if (from < 0 || to < 0) throw new Error(`index.css: block "${start}" not found`);
  return css.slice(from, to);
}

const lightStart = css.indexOf(":root {");
const darkStart = css.indexOf("@media (prefers-color-scheme: dark)");
const themeStart = css.indexOf("@theme inline");

const lightOwn = declarations(section(":root {", "}"));
const darkOwn = declarations(section("@media (prefers-color-scheme: dark)", "@theme inline"));
const theme = declarations(section("@theme inline", "@layer base"));
const light = lightOwn;
const dark = { ...lightOwn, ...darkOwn };

function hex(t: Tokens, name: string): string {
  const value = t[name];
  if (value === undefined) throw new Error(`--${name} is not defined`);
  if (!/^#[0-9a-f]{6}$/.test(value)) throw new Error(`--${name} is "${value}", not #rrggbb`);
  return value;
}

function channel(h: string, i: number): number {
  return parseInt(h.slice(i, i + 2), 16) / 255;
}

function linear(c: number): number {
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(h: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => linear(channel(h, i)));
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}

function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** OKLab (Ottosson 2020). */
function oklab(h: string): [number, number, number] {
  const [r, g, b] = [1, 3, 5].map((i) => linear(channel(h, i))) as [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function deltaE(a: string, b: string): number {
  const [l1, a1, b1] = oklab(a);
  const [l2, a2, b2] = oklab(b);
  return 100 * Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

describe("index.css structure", () => {
  it("has the light, dark and @theme blocks in that order", () => {
    expect(lightStart).toBeGreaterThanOrEqual(0);
    expect(darkStart).toBeGreaterThan(lightStart);
    expect(themeStart).toBeGreaterThan(darkStart);
  });

  it.each([...series, ...surfaces])("defines --%s in the dark block itself", (name) => {
    expect(darkOwn[name]).toBeDefined();
  });

  it.each(series)("maps --color-%s to var(--%s) in @theme", (name) => {
    expect(theme[`color-${name}`]).toBe(`var(--${name})`);
  });
});

describe.each([
  ["light", light],
  ["dark", dark],
])("chart palette (%s)", (_mode, t) => {
  it.each(series.flatMap((s) => surfaces.map((surface) => [s, surface])))(
    "%s on %s is at least 3:1",
    (name, surface) => {
      expect(contrast(hex(t, name), hex(t, surface))).toBeGreaterThanOrEqual(3);
    },
  );

  it("equals no status, brand, primary or scope color", () => {
    const taken = new Set(reserved.map((r) => hex(t, r)));
    for (const s of series) expect(taken.has(hex(t, s))).toBe(false);
  });

  it.each(series.flatMap((s) => distant.map((r) => [s, r])))(
    "%s is at least ΔE 15 from %s",
    (name, other) => {
      expect(deltaE(hex(t, name), hex(t, other))).toBeGreaterThanOrEqual(15);
    },
  );

  it.each(series.flatMap((s, i) => series.slice(i + 1).map((o) => [s, o])))(
    "%s and %s are at least ΔE 15 apart",
    (a, b) => {
      expect(deltaE(hex(t, a), hex(t, b))).toBeGreaterThanOrEqual(15);
    },
  );
});
