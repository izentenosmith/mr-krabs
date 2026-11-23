# Page: Presupuesto (plan-vs-real matrix)

Overrides and extends [MASTER.md](../MASTER.md) for the budget page (FR-BU-02, FR-BU-03,
FR-BU-08, FR-RP-07; DEC-29, DEC-30). Data: `budget_matrix`, `budget_set`,
`budget_copy_month`. Everything not stated here follows MASTER.

## Layout

```
Presupuesto                     [Mes | Año]   ‹ Noviembre 2026 ›   [Copiar mes anterior]
ⓘ 3 movimientos en USD sin convertir · No se incluyen en los totales.
┌──────────────────────────┬─────────────┬─────────────┬───────────────────┐
│ Categoría                │ Planificado │        Real │        Diferencia │  sticky header
├──────────────────────────┼─────────────┼─────────────┼───────────────────┤
│ INGRESOS                 │             │             │                   │  section row
│ ▾ Ingresos               │  3.200.000  │  3.150.000  │ ▼ 50.000          │  group subtotal
│     Sueldo               │  3.000.000  │  3.000.000  │ 0                 │
│ GASTOS                   │             │             │                   │
│ ▾ Comida                 │    600.000  │    655.000  │ ▲ 55.000 excedido │  group subtotal
│     Supermercado         │    450.000  │    520.000  │ ▲ 70.000 excedido │
│     Delivery             │     80.000  │     −12.000 │ ▼ 92.000          │  credit
│ ─────────────────────────────────────────────────────────────────────── │
│ Total ingresos · Total gastos · Ahorro e inversión · Flujo libre        │  summary rows
└──────────────────────────┴─────────────┴─────────────┴───────────────────┘
```

- **Header row** (one line, 44px): page title `text-xl font-semibold` left; then the
  mode `ToggleGroup` "Mes | Año", the period stepper (MASTER §5.4) and, in month mode only,
  the outline button "Copiar mes anterior". No primary button on this page beyond MASTER's.
- The page reads `view: combined` until the lens switcher arrives (as MASTER §5.1), and
  in-cell edits write `scope: household`.
- The matrix fills the content width and scrolls inside its own region (both axes) so the
  header row and the excluded-currency notice stay put.

## The matrix

- TanStack Table, `role="treegrid"`, rows in the order `budget_matrix` returns (category tree
  order). Rows: 36px, cells `text-[13px] tabular-nums`, amounts right-aligned.
- **Sticky:** the header row (`top-0`, z 10) and the category column (`left-0`, z 10, `bg-card`,
  `border-r`). The corner cell is sticky on both axes.
- **Category column:** min 240px. Indentation `12px + 16px × depth`. A group row has a
  disclosure button (Lucide `ChevronDown`/`ChevronRight`, ghost, 16px, `aria-expanded`,
  `aria-label` "Contraer {name}"/"Expandir {name}"); groups start expanded; collapsing is
  per window and not persisted.
- **Section rows** "Ingresos" and "Gastos" (one per kind, income first): `text-xs
  font-medium text-muted-foreground`, uppercase not used (sentence case, MASTER §9), no
  amounts.
- **Group subtotal rows** (`isGroup`): `bg-muted`, name and amounts `font-medium`. They show
  the roll-up (own line + every descendant, DEC-30 Q9) and are read-only.
  When a group has its own budget line or its own postings, the first child row is
  "{name} (general)" carrying those own amounts, editable like a leaf.
- **Leaf rows:** `bg-card`; the name truncates with the full name in `title`.
- **Archived categories** appear only when the period has amounts for them: name in
  `muted-foreground` + Lucide `Archive` (14px) with a visually hidden "Archivada" (as §5.1);
  their planned cell is read-only.
- **Summary rows** at the bottom, after a 2px `border-t`: "Total ingresos", "Total gastos",
  "Ahorro e inversión", "Flujo libre" (glossary). `font-medium`, read-only, no drill-down.
  "Flujo libre" below zero uses `destructive` + `CircleAlert` + visible "déficit".
- A value the DTO does not carry (for example a planned amount for "Ahorro e inversión")
  renders "—" in `muted-foreground` with a visually hidden "Sin dato".

### Month mode ("Mes", default)

Columns: Categoría · Planificado · Real · Diferencia (each number column 136px). The period
stepper steps months. Default: the current month.

### Year mode ("Año")

Columns: Categoría · Ene … Dic · Total — **actuals only** (DEC-30 Q11), 96px per month
column, short month names in the UI language. The period stepper steps years. The current
month's header cell has `bg-accent` and `aria-current="date"`. "Copiar mes anterior" is hidden.
At 1024×700 the table scrolls horizontally behind the sticky category column.

## Cells

All amounts are `MoneyDto.display` as received (MASTER §3), never recomputed.

- **Planificado (editable):** a cell button showing the amount, or empty when there is no
  line. Enter, double-click or typing a digit starts editing: the cell becomes a
  `MoneyInput` (MASTER §7.1) at `h-8`, text selected. Enter or Tab commits
  (`budget_set {period, categoryId, scope, amount}` with the text as typed; empty or 0
  deletes the line, DEC-30 Q9); Esc cancels. While saving the input is read-only with
  `aria-busy`; no spinner. Saved: the cell shows the new value after the matrix refetches; no toast.
  - `VALIDATION`: the input stays open with the typed text, `aria-invalid`,
    `border-destructive`, and a popover under the cell with `CircleAlert` + the field message
    (MASTER §7.1); Esc discards.
  - Any other error: the input closes back to the old value and a `toast.error` per
    MASTER §7.2.
- **Real (drill-down):** a link-style button (`text-foreground`, underline on hover/focus,
  focus ring) that opens Transacciones filtered by `categoryId` (with descendants) and the
  cell's month range (`/transactions?categoryId=…&from=…&to=…`). The month range comes from
  the DTO, not from date math in TS. `aria-label` "Ver movimientos de {categoría}, {mes}".
  In year mode every month cell and the Total cell drill down the same way.
- **Credits:** an actual below zero (refunds larger than spending in an expense category,
  or a reversed income) renders its `display` as received in `success`, with a visually
  hidden " (crédito)" and the tooltip "Las devoluciones superaron los gastos".
- **Diferencia:** `VarianceCell` (below).

### `VarianceCell`

Δ = planned − actual, as computed in Rust. The cell shows an icon (12px, MASTER's ▲/▼ as Lucide
`ArrowUp` / `ArrowDown`, meaning *actual above / below plan*) and `display`. The state is
read from the row's kind and the **signs** of `diff.minor` and `planned.minor` (sign checks
only, no arithmetic):

| State | When | Rendering |
|---|---|---|
| Over budget | expense, actual > plan | `destructive`, `ArrowUp`, `display`, visible `text-xs` "excedido" (MASTER §2) |
| Under budget | expense, actual < plan | `success`, `ArrowDown`, `display`, visually hidden "bajo el presupuesto" |
| On plan | Δ = 0 | `muted-foreground`, no icon, `display` |
| Unplanned | no planned amount, actual ≠ 0 | `muted-foreground`, no icon, `display`, visually hidden "sin presupuesto" |
| Income above plan | income, actual > plan | `success`, `ArrowUp`, visually hidden "sobre lo planificado" |
| Income below plan | income, actual < plan | `muted-foreground`, `ArrowDown`, visually hidden "bajo lo planificado" (not an error: pay may still arrive) |

Never color alone: every state has an icon, a sign or a label (MASTER §1.3).

## Copiar mes anterior

Outline button, month mode only. It calls `budget_copy_month {from: <previous month>,
to: <shown month>, overwrite: false}`: it fills only empty slots, so it needs no
confirmation (DEC-30 Q9). While running the button is disabled with `aria-busy`. Result,
as a Sonner default toast (no undo in M2): "Se copiaron {n} montos de {mes anterior}", or
"No había montos nuevos para copiar" when nothing was empty to fill. Errors: `toast.error`
per MASTER §7.2.

## Excluded currencies (DEC-29 Q8)

When the DTO's `excluded` list is not empty, one line above the matrix: Lucide `Info`
(14px) + `text-xs text-muted-foreground` "{n} movimientos en {USD} sin convertir" (singular
"1 movimiento en {USD} sin convertir"), currencies joined with " · ", then
"No se incluyen en los totales." It is information, never `warning` or `destructive`:
nothing is wrong, M4 converts them.

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the matrix region.
- **No plan yet** (no budget line in the shown month): the matrix still renders (it is the
  input) with one `text-sm text-muted-foreground` line above it: "Aún no hay presupuesto
  para {mes}. Escribe un monto en Planificado o copia el mes anterior."
- **Not yet available** (`NOT_IMPLEMENTED`): the region shows the Hourglass block; the mode
  toggle, stepper and "Copiar mes anterior" are disabled.
- **Load error:** `ErrorAlert` with the per-code copy and an outline "Reintentar".

## Keyboard (MASTER §5, NFR-11)

- The treegrid is one tab stop with a roving focus: ↑ ↓ ← → move between cells, Home/End
  to the row's first/last cell; on a group's name cell → expands and ← collapses.
- Enter: edits a Planificado cell, opens a Real cell's drill-down, toggles a group.
- Esc cancels an edit. Tab leaves the grid (while editing, Tab commits and moves right).
- Focus ring per MASTER §5 inside sticky cells too (the ring is inset: `ring-inset`).

## Accessibility

- Row headers are the category cells (`role="rowheader"`), with `aria-level` = depth + 1
  and `aria-expanded` on groups.
- Column headers name the month in year mode ("Noviembre", not "Nov") via `aria-label`.
- Amount cells read as `display` plus the state labels above.
