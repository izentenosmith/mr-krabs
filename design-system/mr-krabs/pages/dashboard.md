# Page: Resumen (dashboard)

Overrides and extends [MASTER.md](../MASTER.md) for the dashboard (FR-RP-02, FR-RP-03; DEC-29
Q6–Q8). Data: `report_cash_flow` → `CashFlowDto` (monthly) and `report_spending` →
`BreakdownDto` (C0). Charts follow MASTER §8 and the chart palette in §8.1; rules marked
**shared** come from [budget.md](budget.md).

## Layout

```
Resumen                                              ‹ Noviembre 2026 ›
┌──────────────────────────┬──────────────────────────┐
│ Ingresos                 │ Gastos                   │  stat tiles (shown month)
│ $3.150.000               │ $2.410.000               │
├──────────────────────────┼──────────────────────────┤
│ Ahorro e inversión       │ Flujo libre              │
│ $400.000                 │ $340.000                 │
│ 12,7 % de los ingresos   │                          │
└──────────────────────────┴──────────────────────────┘
┌ Flujo de caja · dic 2025 – nov 2026 ──────────── [Gráfico | Tabla] ┐
│ ⓘ excluded-currency line for this card, if any                     │
│ ■ Ingresos  ■ Gastos  ■ Ahorro e inversión  ─● Flujo libre         │
│   ▮   ▮   ▮   ▮ …                                       above zero │
│ ──────────────────────────────────────────────────── 0             │
│   ▮   ▮   ▮   ▮ …   (gastos next to zero, ahorro below) below zero │
└────────────────────────────────────────────────────────────────────┘
┌ Gastos por grupo · noviembre 2026 ──────────────────────────────────┐
│ Comida          ████████████          $655.000   27,2 %            │
│ Gastos fijos    █████████             $520.000   21,6 %            │
└─────────────────────────────────────────────────────────────────────┘
```

- **Header row:** MASTER §5.4, with the period stepper (months; default the current month).
  `view: "combined"` (as budget.md). This page has no primary button.
- The shown month drives everything: the tiles and "Gastos por grupo" show that month
  (`report_spending {from: m, to: m}`); the cash-flow chart shows the 12 months ending with it
  (`report_cash_flow {from, to}` with the 12-month window from the period helper, MASTER §5.4).
- Sections stack vertically, 24px apart, each a `Card` (border, no shadow, `p-4`); the title
  `text-base font-semibold`, the period after it in `text-sm text-muted-foreground`.
- **Excluded currencies:** each card shows its own DTO's `excluded` line (shared), under its
  title; the tiles use the cash-flow card's.

## Stat tiles

`Card`s in a grid, 16px gap: **two columns** while the content area is under 1280px wide, four
from 1280px (at 1024×700 the content is ~720px). Caption `text-xs text-muted-foreground`; value
as MASTER §3's hero figure (`text-[28px] font-semibold tabular-nums`), the shown month's
`CashFlowMonthDto` `display` as received (no sign added or removed), never truncated.

| Tile | Value | Rendering |
|---|---|---|
| Ingresos | `income` | `success` (MASTER §2 income) |
| Gastos | `expense` | `foreground` |
| Ahorro e inversión | `saved` | `foreground`; second line `text-xs text-muted-foreground` "{tasa} de los ingresos" with the savings rate's `PercentDto.display`, or "—" when it is `null` (no income) |
| Flujo libre | `freeCashFlow` | `foreground`; negative `minor`: `destructive` + `CircleAlert` + visible "déficit" |

Percentages arrive as `PercentDto {bp, display}` formatted in Rust (X-17); the UI shows
`display` and never formats `bp`. FR-RP-02's "net" (income − expense) is not a tile: DEC-29
Q7 defines free cash flow as the month's bottom line; a separate net is an M2 follow-up for the
nexus if the owner wants it.

## Cash flow chart

- **Form:** one bar group per month on a shared zero baseline (MASTER §8): **Ingresos**
  (`chart-1`) above zero; **Gastos** (`chart-2`) and **Ahorro e inversión** (`chart-3`)
  stacked below zero, Gastos next to the baseline. **Flujo libre** is a line in `foreground`
  (2px) with 8px dot markers ringed 2px in `card`: a different mark, so it never depends on
  color.
- **Plot values (X-17):** each series point carries a Rust-computed signed plot value as a
  `MoneyDto` (outflows negative, so a net withdrawal from savings rises above zero as money
  coming in). The UI passes `Number(plot.minor)` to the chart library as an opaque
  coordinate and does nothing else with it; every number a person reads is a `display`.
- **Marks:** MASTER §8; bar width 60% of the band.
- **Axes:** x = short month names in the UI language (`text-xs text-muted-foreground`), the
  shown month `font-semibold`. No y-axis tick labels (MASTER §8). The zero baseline is 1px
  `muted-foreground`.
- **Active month:** marked by a 1px dashed vertical guide in `muted-foreground` through the
  band's center and its bold x label — **no fill behind the marks** (a `muted` band would drop
  `chart-3` below 3:1 in dark mode).
- **Legend:** always visible above the plot, left-aligned: 10px rounded swatch (a short line
  with a dot for Flujo libre) + label in `text-xs text-foreground`.
- **Tooltip** (`bg-popover`, border, `shadow-md`, `text-xs`): month and year, then the four
  values with their swatches, and the savings rate (or "—").
- **Keyboard and screen reader:** the plot is `role="group"` with
  `aria-roledescription="gráfico"`, one tab stop (`tabindex="0"`, focus ring). ← → move the
  active month and show its tooltip, Home/End jump to the ends, Esc hides it. A visually
  hidden `aria-live="polite"` region announces the tooltip's text on every move. The
  `aria-label` summarizes: "Flujo de caja de {mes inicial} a {mes final}. {mes}: ingresos {x},
  gastos {y}, ahorro e inversión {z}, flujo libre {w}. Usa las flechas para recorrer los meses
  o abre la vista de tabla." Hover shows the same tooltip per band.
- **Table view:** a `ToggleGroup` "Gráfico | Tabla" in the card header swaps the plot for a
  table of the same series: rows = months (oldest first), columns Mes · Ingresos · Gastos ·
  Ahorro e inversión · Flujo libre · Tasa de ahorro; amounts right-aligned per MASTER §3; the
  choice is per window.

## Gastos por grupo

- The rows of `BreakdownDto` whose `category.depth` is 0 (top-level expense groups, rolled up,
  FR-BU-08), in the DTO's order: largest amount first (X-17).
  A ranked bar list: one series, every value directly labelled, so no legend and no tooltip
  (MASTER §8 exemptions).
- A `<table>` with `aria-label` "Gastos por grupo, {mes}"; column headers (visually hidden)
  Grupo · Barra · Monto · Porcentaje. Cells: the group name (`text-sm`, 180px, truncate,
  full name in `title`) holding a link-style button (shared drill-down; `aria-label` "Ver
  gastos de {grupo}, {mes}"); the bar (decorative, `aria-hidden`); `amount.display`
  (right-aligned, `tabular-nums`); the share's `PercentDto.display`, or "—" when `null`
  (`text-muted-foreground`).
- The bar is `chart-2` (the "Gastos" color, so color follows the entity across the page), 12px
  tall, 4px radius at its end, no track. Its length is the share, from the share's `bp` set through a
  CSS variable (`style="--share-bp: 2720"`, `width: calc(var(--share-bp) * 0.01%)`): CSS
  draws it, TS does no arithmetic. A negative
  `amount.minor` (a credit, shared) draws no bar.

## States (MASTER §7.2)

Per card: "Cargando…" with `aria-busy`; **empty** (no postings in the period): the `Inbox`
block "Aún no hay movimientos en este período." with an outline small button "Ir a
Transacciones" (there is no page primary to point at); `NOT_IMPLEMENTED`: the Hourglass block;
load error: `ErrorAlert` + "Reintentar". One card failing does not hide the others.
