# Page: Resumen (dashboard)

Overrides and extends [MASTER.md](../MASTER.md) for the dashboard (FR-RP-02, FR-RP-03; DEC-29
Q6–Q8). Data: `report_cash_flow` (monthly), `report_spending` (by category, rolled up).
Charts follow MASTER §8 and its chart palette.

## Layout

```
Resumen                                              ‹ Noviembre 2026 ›
ⓘ 2 movimientos en USD sin convertir · No se incluyen en los totales.
┌──────────────┬──────────────┬──────────────┬──────────────┐
│ Ingresos     │ Gastos       │ Ahorro e inv.│ Flujo libre  │  stat tiles (shown month)
│ +3.150.000   │ 2.410.000    │ 400.000      │ 340.000      │
│              │              │ 12,7 % de…   │              │
└──────────────┴──────────────┴──────────────┴──────────────┘
┌ Flujo de caja · dic 2025 – nov 2026 ──────────── [Gráfico | Tabla] ┐
│ ■ Ingresos  ■ Gastos  ■ Ahorro e inversión  ─● Flujo libre         │
│   ▮   ▮   ▮   ▮ …                                                  │  above zero
│ ──────────────────────────────── 0                                 │
│   ▮   ▮   ▮   ▮ …   (gastos, then ahorro stacked below)            │  below zero
└────────────────────────────────────────────────────────────────────┘
┌ Gastos por grupo · noviembre 2026 ──────────────────────────────────┐
│ Comida          ████████████          655.000   27,2 %             │
│ Gastos fijos    █████████             520.000   21,6 %             │
└─────────────────────────────────────────────────────────────────────┘
```

- **Header row** (44px): title, then the period stepper (MASTER §5.4, months). Default:
  the current month. `view: combined` (as budget.md).
- The shown month drives everything: the stat tiles and "Gastos por grupo" show that
  month; the cash-flow chart shows the 12 months ending with it (`report_cash_flow {from,
  to}`).
- Sections stack vertically, 24px apart, each a `Card` (border, no shadow, `p-4`), section
  title `text-base font-semibold` with the period in `text-sm text-muted-foreground`.
- The excluded-currency line (as budget.md) sits under the header row and covers every
  section on the page.

## Stat tiles

Four `Card`s in one row (`grid-cols-4`, 16px gap): caption `text-xs text-muted-foreground`,
value as MASTER §3's hero figure (`text-[28px] font-semibold tabular-nums`), `display`
as received.

| Tile | Value | Rendering |
|---|---|---|
| Ingresos | income | `success`, leading `+` as MASTER §2 |
| Gastos | expense | `foreground` |
| Ahorro e inversión | saved | `foreground`; second line `text-xs text-muted-foreground` "{tasa} de los ingresos" (savings rate) |
| Flujo libre | free cash flow | `foreground`; below zero: `destructive` + `CircleAlert` + visible "déficit" |

## Cash flow chart

- **Form:** one bar group per month on a shared zero baseline (MASTER §8). Above zero:
  **Ingresos** (`chart-1`). Below zero, stacked: **Gastos** (`chart-2`) next to the
  baseline, then **Ahorro e inversión** (`chart-3`). **Flujo libre** is a line in
  `foreground` (2px) with 8px dot markers ringed 2px in `card`: a different mark, so it
  never depends on color.
- **Marks:** bar width 60% of the band, 4px radius on the end away from the baseline,
  2px `card` gap between stacked segments; gridlines `border` (horizontal only, dashed off);
  the zero baseline 1px `muted-foreground`.
- **Axes:** x = short month names in the UI language (`text-xs text-muted-foreground`),
  the shown month `font-medium`. **No y-axis tick labels**: they would need number
  formatting in TS (CLAUDE.md rule 5). Values are read from tooltips, the tiles and the
  table, all `display` strings from Rust.
- **Geometry only in TS:** to place marks the chart reads `minor` as a JS number and draws
  outflows below zero by sign; nothing shown to the user is derived from that.
- **Legend:** always visible above the plot, left-aligned: 10px rounded swatch (or a short
  line with a dot for Flujo libre) + label in `text-xs text-foreground`. Labels never wear
  the series color.
- **Tooltip** (`bg-popover`, border, `shadow-md`, `text-xs`): month and year, then the
  four values with their swatch and the savings rate. Hover shows it per month band; the
  plot is one tab stop (`tabindex="0"`, focus ring) where ← → move the active month and
  show its tooltip, Home/End jump to the ends, Esc hides it. The active band gets a
  `muted` background behind the bars.
- **`aria-label`** on the plot: "Flujo de caja de {mes inicial} a {mes final}. {mes}:
  ingresos {x}, gastos {y}, ahorro e inversión {z}, flujo libre {w}." for the shown
  month, plus "Usa las flechas para recorrer los meses o abre la vista de tabla."
- **Table view:** a `ToggleGroup` "Gráfico | Tabla" in the card header swaps the plot for a
  table of the same series: rows = months (oldest first), columns Mes · Ingresos · Gastos ·
  Ahorro e inversión · Flujo libre · Tasa de ahorro; amounts right-aligned per MASTER §3;
  the choice is per window.

## Gastos por grupo

- Top-level expense groups (rolled up, FR-BU-08) for the shown month, in the order
  `report_spending` returns. A ranked bar list — one series, so no legend; each row is
  direct-labeled.
- It is a table (`role="table"`, column headers visually hidden): Grupo (`text-sm`, 180px,
  truncate) · bar (decorative, `aria-hidden`) · Monto (`display`, right-aligned,
  `tabular-nums`) · Porcentaje (the share's display string, `text-muted-foreground`).
  Because it is already a table it needs no separate table view.
- The bar is `chart-2` (the "Gastos" color, so color follows the entity across the page),
  12px tall, 4px radius at its end, track none; its length is the group's share of the
  month's spending set through a CSS variable (`width: calc(var(--share-bp) * 0.01%)`).
  A credit (share below zero) draws no bar and shows its amount as budget.md's credits.
- Each row is a link-style button to Transacciones filtered by that group (with
  descendants) and the month; `aria-label` "Ver gastos de {grupo}, {mes}".

## States (MASTER §7.2)

Per card: "Cargando…" with `aria-busy`; **empty** (no postings in the period): the `Inbox`
block "Aún no hay movimientos en este período." (no button; the view's primary is MASTER's
"+ Nuevo"); `NOT_IMPLEMENTED`: the Hourglass block; load error: `ErrorAlert` +
"Reintentar". One card failing does not hide the others.
