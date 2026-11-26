# Page: Resumen (dashboard)

Overrides and extends [MASTER.md](../MASTER.md) for the dashboard (FR-RP-02, FR-RP-03; DEC-29
Q6–Q8). Data: `report_cash_flow` → `CashFlowDto` (monthly) and `report_spending` →
`BreakdownDto` (C0 contract as of #37 `69f7842`).
Charts follow MASTER §8 and §8.1; rules marked **(shared)** are budget.md's report-grid rules.

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
│ ⓘ excluded-currency line for dic 2025 – nov 2026, if any            │
│ ■ Ingresos  ■ Gastos  ■ Ahorro e inversión  ─● Flujo libre         │
│   ▮   ▮   ▮   ▮ …                                       above zero │
│ ──────────────────────────────────────────────────── 0             │
│   ▮   ▮   ▮   ▮ …   (gastos next to zero, ahorro below) below zero │
└────────────────────────────────────────────────────────────────────┘
┌ Gastos por grupo · noviembre 2026 ──────────────────────────────────┐
│ Comida          ████████████  $655.000   27,2 %                    │
│ Gastos fijos    █████████     $520.000   21,6 %                    │
└─────────────────────────────────────────────────────────────────────┘
```

- **Header row:** MASTER §5.4 with the period stepper (months; default the current month).
  `view: "combined"` (as budget.md). This page has no primary button (MASTER §5).
- The **shown month** (the stepper's) drives everything: the tiles and "Gastos por grupo" show
  it (`report_spending {from: m, to: m}`); the chart shows the 12 months ending with it
  (`report_cash_flow {from, to}`, the window from the period helper, MASTER §5.4).
- Sections stack vertically, 24px apart, each a `Card` (border, no shadow, `p-4`); title
  `text-base font-semibold`, the period after it in `text-sm text-muted-foreground`.
- **Excluded currencies (shared):** the cash-flow card shows its DTO's notice naming its
  12-month window; "Gastos por grupo" shows its own, naming the month. The tiles show none (the
  cash-flow DTO's count spans 12 months, not the tile's month).

## Stat tiles

`Card`s in a grid, 16px gap: **two columns** while the content area is under 1280px wide, four
from 1280px (at 1024×700 the content is ~720px). Caption `text-xs text-muted-foreground`; value
as MASTER §3's hero figure (`text-[28px] font-semibold tabular-nums`, `foreground`), the shown
month's `CashFlowMonthDto` `display` as received (no sign added or removed), never truncated.
The caption names the kind, so tiles follow MASTER §2's "Report grids" rule: no income color.

| Tile | Value | Extra |
|---|---|---|
| Ingresos | `income` | — |
| Gastos | `expense` | a negative `minor` is a credit (shared rule: visible "crédito") |
| Ahorro e inversión | `saved` | second line `text-xs text-muted-foreground` "{tasa} de los ingresos" with `savingsRate.display` (a `PercentDto`); when `savingsRate` is `null` (no income) the whole line is "—" |
| Flujo libre | `freeCashFlow` | negative `minor`: `destructive` + `CircleAlert` + visible "déficit" |

Percentages arrive as `PercentDto {bp, display}` formatted in Rust (X-17); the UI shows
`display` and never formats `bp`. FR-RP-02's "net" (income − expense) is not a tile: DEC-29 Q7
defines free cash flow as the month's bottom line; a separate net is an M2 follow-up for the
nexus if the owner wants it.

## Cash flow chart

- **Form:** one bar group per month on a shared zero baseline (MASTER §8): **Ingresos**
  (`chart-1`), **Gastos** (`chart-2`) and **Ahorro e inversión** (`chart-3`) share one stack with
  **sign-split stacking** (Recharts `stackOffset="sign"`): positive coordinates stack up from
  zero, negative ones down, so Gastos sits next to the baseline below zero with Ahorro under
  it, and a positive Ahorro (a net withdrawal) stacks above Ingresos instead of overlapping
  Gastos. **Flujo libre** is a line in `foreground`
  (2px) with 8px dot markers ringed 2px in `card`: a different mark, so it never depends on
  color.
- **Plot values (X-17):** each month's `plot: CashFlowPlotDto {income, expense, saved,
  freeCashFlow}` carries Rust-computed signed coordinates (inflows positive, outflows
  negative, so a net withdrawal from savings rises above zero as money coming in). The UI
  passes `Number(plot.<series>.minor)` to the chart library as an opaque coordinate and does
  nothing else with it; every number a person reads is the month's display amounts.
- **Marks:** MASTER §8; bar width 60% of the band.
- **Axes:** x = short month names from the i18n catalogue (`text-xs text-muted-foreground`),
  the **shown** month `font-semibold`. No y-axis tick labels (MASTER §8). The zero baseline is
  1px `muted-foreground`.
- **Focused month vs shown month.** The shown month is the stepper's (bold label). The
  **focused** month is the keyboard/hover cursor: a 1px dashed vertical guide in
  `muted-foreground` through its band (MASTER §8's one guide) plus the tooltip. No fill is
  drawn behind marks.
- **Keyboard and screen reader:** the plot is `role="group"` with `aria-roledescription` and
  `aria-label` from the i18n catalogue (es-CL "gráfico"), one tab stop (`tabindex="0"`, focus
  ring). On focus the focused month is the shown month. ← → move the focused month,
  Home/End (⌘←/⌘→) jump to the ends, Esc hides the tooltip. Enter makes the focused month the
  shown month (as the stepper would): the card keeps the previous data on screen while the new
  window loads (no "Cargando…" swap), focus stays on the plot, and the focused month stays the
  same month (now the last band). A visually hidden `aria-live="polite"` region announces the
  tooltip's text on every keyboard move (the stepper's live region stays silent for this
  change). The `aria-label` describes the shown month: "Flujo de caja de
  {mes inicial} a {mes final}. {mes}: ingresos {x}, gastos {y}, ahorro e inversión {z}, flujo
  libre {w}. Usa las flechas para recorrer los meses o abre la vista de tabla." Hover shows the
  tooltip for the hovered band without moving the keyboard's focused month and without
  announcing. If a screen reader keeps the arrow keys, the table view is the path.
- **Tooltip** (`bg-popover`, border, `shadow-md`, `text-xs`): month and year, then the four
  values with their swatches, and the savings rate (or "—").
- **Legend:** always visible above the plot, left-aligned: 10px rounded swatch (a short line
  with a dot for Flujo libre) + label in `text-xs text-foreground`.
- **Table view:** a `ToggleGroup` "Gráfico | Tabla" in the card header swaps the plot for a
  table of the same series, with a visually hidden `<caption>` "Flujo de caja, {mes inicial} a
  {mes final}": rows = months (oldest first, `th scope="row"`), columns Mes ·
  Ingresos · Gastos · Ahorro e inversión · Flujo libre · Tasa de ahorro; amounts right-aligned
  per MASTER §3; the choice is per window.

## Gastos por grupo

- The rows of `BreakdownDto` whose `category.depth` is 0 (top-level expense groups, rolled up,
  FR-BU-08), in the DTO's order: largest amount first (X-17). One series, every value directly
  labelled, so no legend and no tooltip (MASTER §8 exemptions).
- A `<table>` with a visually hidden `<caption>` "Gastos por grupo, {mes}", column headers
  (visually hidden) Grupo · Monto · Porcentaje:
  - **Grupo:** `<th scope="row">`, `text-sm`, 180px, truncate with the full name in `title`;
    its content is a link (shared drill-down, `month`) whose text is the group name and whose
    accessible name adds ", ver gastos de {mes}" — a stated departure from the shared
    amount-first name, because here the amount has its own cell.
  - **Monto:** the bar, then `amount.display` (right-aligned, `tabular-nums`), plus "crédito"
    when negative (shared rule).
  - **Porcentaje:** `share.display` (a `PercentDto`), or "—" when `share` is `null`
    (`text-muted-foreground`).
- **The bar** (decorative, `aria-hidden`) is `chart-2` (the "Gastos" color, so color follows the
  entity across the page), 12px tall, 4px radius at its end, no track, in a fixed-width track
  area at the start of the Monto cell. Its length is `share.bp` set as a CSS variable:
  `width: min(calc(var(--share-bp, 0) * 0.01%), 100%)`. CSS draws it; TS does no arithmetic.
  A `null` share or a negative amount (a credit, shared) renders **no bar element**.

## States (MASTER §7.2)

Per card: "Cargando…" with `aria-busy`. **No amounts** (every amount in that card's DTO has
`minor` 0 **and** its `excluded` is empty): the `Inbox` block "No hay montos en este período."
with an outline small link-button "Ir a Transacciones" (there is no page primary to point at).
`NOT_IMPLEMENTED`: the Hourglass block. Load error: `ErrorAlert` + "Reintentar". One card
failing does not hide the others; the tiles follow the cash-flow card's state.
