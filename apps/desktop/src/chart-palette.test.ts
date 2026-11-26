import { readFileSync } from "node:fs";
import { join } from "node:path";
import postcss, { type AtRule, type Container, type Declaration, type Rule } from "postcss";

/**
 * MASTER §8.1: the categorical chart tokens stay legible marks (WCAG 1.4.11, ≥ 3:1) on every
 * surface a mark is drawn on, in both appearances, and stay clear of the reserved colors.
 * index.css is read with a real CSS parser (comments, strings and nesting handled), and its
 * shape is pinned so the two parsed `:root` rules are the only place these tokens can come
 * from. Colors only — no money is computed here.
 */

const root = postcss.parse(readFileSync(join(import.meta.dirname, "index.css"), "utf8"));

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

/** A guarded token: any `--chart…` or exactly a surface name, in any letter case. */
const guarded = (prop: string) => /^--chart/i.test(prop) || surfaces.includes(prop.slice(2).toLowerCase());
/** A guarded @theme mapping: `--color-chart…` or `--color-<surface>`, in any letter case. */
const guardedColor = (prop: string) =>
  /^--color-chart/i.test(prop) || surfaces.includes(prop.replace(/^--color-/i, "").toLowerCase());

const all = <T>(walk: (cb: (node: T) => void) => void): T[] => {
  const out: T[] = [];
  walk((n) => out.push(n));
  return out;
};
const decls = all<Declaration>((cb) => root.walkDecls(cb));
const atRules = all<AtRule>((cb) => root.walkAtRules(cb));

const topLight = root.nodes.filter((n): n is Rule => n.type === "rule" && n.selector === ":root");
const topDark = root.nodes.filter(
  (n): n is AtRule =>
    n.type === "atrule" && n.name === "media" && n.params.includes("prefers-color-scheme"),
);
const themes = atRules.filter((a) => a.name === "theme");

const lightRule = topLight[0];
const darkMedia = topDark[0];
const darkRule = darkMedia?.nodes?.[0] as Rule | undefined;
const theme = themes[0];

type Tokens = Record<string, string>;

/** The rule's own declarations, in order (a later one wins, as in CSS). */
function own(container: Container | undefined): Tokens {
  const out: Tokens = {};
  for (const node of container?.nodes ?? []) {
    if (node.type === "decl" && node.prop.startsWith("--")) out[node.prop.slice(2)] = node.value.trim().toLowerCase();
  }
  return out;
}

const lightOwn = own(lightRule);
const darkOwn = own(darkRule);
const themeOwn = own(theme);
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
  it("has exactly one top-level light :root rule", () => {
    expect(topLight).toHaveLength(1);
  });

  it("has exactly one top-level dark block, unconditional, holding only a :root rule", () => {
    expect(topDark).toHaveLength(1);
    expect(darkMedia?.params).toBe("(prefers-color-scheme: dark)");
    expect(darkMedia?.nodes).toHaveLength(1);
    expect(darkRule?.type).toBe("rule");
    expect(darkRule?.selector).toBe(":root");
  });

  it("has exactly one @theme block, top-level and inline", () => {
    expect(themes).toHaveLength(1);
    expect(theme?.parent).toBe(root);
    expect(theme?.params).toBe("inline");
  });

  it("has the light, dark and @theme blocks in that order", () => {
    const at = (n: unknown) => root.nodes.indexOf(n as never);
    expect(at(darkMedia)).toBeGreaterThan(at(lightRule));
    expect(at(theme)).toBeGreaterThan(at(darkMedia));
  });

  it("uses no escapes in custom property names", () => {
    expect(decls.filter((d) => d.prop.startsWith("--") && d.prop.includes("\\")).map((d) => d.prop)).toEqual([]);
  });

  // No later rule, other selector, nested rule or conditional block can override what this
  // file checks: every chart or surface token sits directly in one of the two parsed rules.
  it("declares chart and surface tokens only in the light and dark :root rules", () => {
    const stray = decls.filter((d) => guarded(d.prop) && d.parent !== lightRule && d.parent !== darkRule);
    expect(stray.map((d) => `${d.prop} in ${(d.parent as Rule).selector ?? "?"}`)).toEqual([]);
  });

  it("defines no chart or surface token beyond the guarded names, in lower case", () => {
    const names = decls.filter((d) => guarded(d.prop)).map((d) => d.prop.slice(2));
    expect([...new Set(names)].sort()).toEqual([...series, ...surfaces].sort());
  });

  it.each([...series, ...surfaces])("defines --%s in the dark rule itself", (name) => {
    expect(darkOwn[name]).toBeDefined();
  });

  it("maps chart and surface colors only in @theme, and nothing beyond them", () => {
    const mapped = decls.filter((d) => guardedColor(d.prop));
    expect(mapped.filter((d) => d.parent !== theme).map((d) => d.prop)).toEqual([]);
    expect(mapped.map((d) => d.prop.slice(2)).sort()).toEqual(
      [...series, ...surfaces].map((s) => `color-${s}`).sort(),
    );
  });

  it.each([...series, ...surfaces])("maps --color-%s to var(--%s) in @theme", (name) => {
    expect(themeOwn[`color-${name}`]).toBe(`var(--${name})`);
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
