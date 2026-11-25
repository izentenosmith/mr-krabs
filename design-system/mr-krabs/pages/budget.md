# Page: Presupuesto (plan-vs-real matrix)

Overrides and extends [MASTER.md](../MASTER.md) for the budget page (FR-BU-02, FR-BU-03,
FR-BU-08, FR-RP-07; DEC-29, DEC-30). Data: `budget_matrix` → `BudgetMatrixDto`, `budget_set`,
`budget_copy_month` (C0 contract, `packages/api-client/src/bindings.ts`). Everything not stated
here follows MASTER. Rules marked **shared** are reused by `calendar.md` and `dashboard.md`.

## Layout

```
Presupuesto                     [Mes | Año]   ‹ Noviembre 2026 ›   [Copiar mes anterior]
ⓘ 3 movimientos en USD sin convertir en 2026 · No se incluyen en los totales.
┌──────────────────────────┬─────────────┬─────────────┬──────────────────────┐
│ Categoría                │ Planificado │        Real │           Diferencia │  sticky header
├──────────────────────────┼─────────────┼─────────────┼──────────────────────┤
│ Ingresos                 │             │             │                      │  section row
│ ▾ Ingresos               │ $3.200.000  │ $3.150.000  │ ↓ $50.000            │  group (roll-up)
│     Sueldo               │ $3.000.000  │ $3.000.000  │ $0                   │
│ Gastos                   │             │             │                      │  section row
│ ▾ Comida                 │   $600.000  │   $655.000  │ ⊘ -$55.000 excedido  │  group (roll-up)
│     Supermercado         │   $450.000  │   $520.000  │ ⊘ -$70.000 excedido  │
│     Delivery             │    $80.000  │   -$12.000  │ ↓ $92.000            │  credit
│ ─────────────────────────────────────────────────────────────────────────── │
│ Total ingresos · Total gastos · Ahorro e inversión · Flujo libre            │  summary rows
└──────────────────────────┴─────────────┴─────────────┴──────────────────────┘
```

(`⊘` stands for Lucide `CircleAlert`, `↓` for `ArrowDown`. Values are `display` strings as Rust
formats them, sign included.)

- **Header row:** MASTER §5.4. Controls: the mode `ToggleGroup` "Mes | Año", the period
  stepper, and in month mode the outline button "Copiar mes anterior". This page has no
  primary button.
- **View and scope.** The page reads `view: "combined"` (as MASTER §5.1) and in-cell edits
  write `scope: "household"`. M2 has no way to write a personal line, so in M2 every planned
  cell equals the household line it edits. Per-view editing (personal lines, the lens
  switcher) is decided with FR-HH-03 in M3.
- The matrix scrolls inside its own region (both axes); the header row and the notice stay put.

## The matrix

- TanStack Table, `role="treegrid"`. Rows come in the order of `BudgetMatrixDto.rows`.
  Rows 36px, cells `text-[13px] tabular-nums`, amounts right-aligned.
- **Row semantics (shared):** each `role="row"` carries `aria-level` = `category.depth + 1`,
  `aria-setsize` and `aria-posinset` among its siblings, and, on a group
  (`category.isGroup`), `aria-expanded`. The category cell is `role="rowheader"`. The chevron
  (`ChevronDown` / `ChevronRight`, 16px) is decorative (`aria-hidden`); clicking the
  category cell, Enter, → and ← expand and collapse. Groups start expanded; the state is per
  window and not persisted.
- **Sticky (shared):** header row and category column `bg-card`, z 10 (`border-r` on the
  column); the corner cell z 11 so it paints over both.
- **Category column (shared):** min 240px; indentation `12px + 16px × category.depth`; the
  name truncates, full name in `title`.
- **Section rows (shared):** a section row is inserted wherever `category.kind` changes from
  one row to the next, labelled "Ingresos" or "Gastos" (`text-xs font-medium
  text-muted-foreground`, no amounts, `role="row"` with `aria-level="1"`). No fixed order is
  claimed: rows keep the DTO's order.
- **Group rows** (`category.isGroup`): `bg-muted`, `font-medium`; the amounts are roll-ups
  (own line + every descendant, DEC-30 Q9).
- **Archived categories** (`category.isArchived`) appear only when the DTO returns them: name
  in `muted-foreground` + Lucide `Archive` (14px) with a visually hidden "Archivada"
  (MASTER §5.1). They stay editable, so a line can still be cleared.
- **Summary rows** after a 2px `border-t`, from `BudgetMatrixDto.summary`: "Total ingresos"
  (`income`), "Total gastos" (`expense`), "Ahorro e inversión" (`saved`), "Flujo libre"
  (`freeCashFlow`). `font-medium`, read-only, no drill-down; their Diferencia is the plain
  `display` in `muted-foreground` with no state. "Flujo libre" with a negative actual uses
  `destructive` + `CircleAlert` + visible "déficit". (`summary.totalExpenses` is not shown
  until its meaning is defined, C0 cross-slice note.)
- A value the DTO marks as absent renders "—" in `muted-foreground` with a visually hidden
  "Sin dato".

### Month mode ("Mes", default)

Columns: Categoría · Planificado · Real · Diferencia (136px each), from
`rows[i].months[m]` (`planned`, `actual`, `diff`) for the shown month. The stepper steps
months; default the current month.

### Year mode ("Año")

Columns: Categoría · Ene … Dic — **actuals only** (DEC-30 Q11), 112px per month, short
month names in the UI language with the full name as `aria-label`. There is no Total column:
the DTO carries no annual total and the UI does not add. The stepper steps years. The
current month's header has `bg-accent` and `aria-current="date"`. "Copiar mes anterior" is
hidden. At 1024×700 the table scrolls horizontally behind the sticky column.

FR-BU-03's "% used" is not a column of this page (DEC-30 Q11 fixed the columns); it is an
M2 follow-up for the nexus.

## Cells

All amounts are `MoneyDto.display` as received, sign included (MASTER §3); never
recomputed or re-signed.

- **Planificado (editable)** — a cell button. On a leaf it shows `months[m].planned`. On a
  group it shows the roll-up and edits the group's **own** line: the input opens with
  `ownPlanned[m]` and a `text-xs text-muted-foreground` hint under it, "Monto propio del
  grupo; el total suma sus subcategorías." After saving, the cell shows the roll-up again.
  - Enter, double-click or typing a digit starts editing: the cell becomes a `MoneyInput`
    (MASTER §7.1) at `h-8`, text selected. While editing, every key goes to the input (arrows,
    Home, End move the caret).
  - Enter, Tab, Shift+Tab or blur commits (`budget_set {period, categoryId, scope, amount}`
    with the text as typed; empty or 0 deletes the line, DEC-30 Q9); Esc cancels.
  - Saving: the input is read-only with `aria-busy`, no spinner. Saved: the matrix refetches;
    no toast.
  - `VALIDATION`: the input stays open with the typed text, `aria-invalid`,
    `border-destructive`, and the cell error popover (MASTER §7.2).
  - `NOT_IMPLEMENTED`: the cell returns to its old value and the not-available toast shows
    (MASTER §7.2); never `toast.error`.
  - Any other error: the cell returns to its old value and `toast.error` (MASTER §7.2).
- **Real (drill-down, shared)** — a link-style button in `text-foreground` (an override of
  MASTER's `primary` links: these are amounts, MASTER §2), underline on hover and focus. It
  opens Transacciones with `categoryId` (with descendants, C0 `TxQuery.categoryId`) and the
  cell's month (`/transactions?categoryId=…&month=YYYY-MM`); a day cell uses `day=YYYY-MM-DD`.
  The transactions page turns `month` into `from`/`to` through the one period helper
  (MASTER §5.4, **pending Q-m2-design-system-2**). `aria-label` "Ver movimientos de
  {categoría}, {mes}". In year mode every month cell drills down the same way.
- **Credits (shared)** — in an **expense** row (`category.kind = "expense"`), an actual whose
  `minor` is negative (refunds larger than spending) renders its `display` in `success`, with a
  visually hidden " (crédito)" and the tooltip "Las devoluciones superaron los gastos". In an
  income row a negative actual is shown as received in `foreground`, unstyled.
- **Diferencia** — `VarianceCell` below.

### `VarianceCell`

Δ = planned − actual, computed in Rust (`diff`), shown as its `display` (signed: over budget
is negative). Icons are 12px Lucide; `ArrowDown`/`ArrowUp` stand for MASTER §7's ▼/▲ and mean
*actual below / above plan*. The state reads the row's `category.kind` and the **signs** of
`diff.minor` and `planned.minor` — sign checks only, no arithmetic. **First match wins:**

| # | State | When | Rendering |
|---|---|---|---|
| 1 | Unplanned | `planned.minor` is 0 | `muted-foreground`, no icon, visually hidden "sin presupuesto" |
| 2 | On plan | `diff.minor` is 0 | `muted-foreground`, no icon |
| 3 | Over budget | expense, `diff` negative | `destructive`, `CircleAlert`, visible `text-xs` "excedido" (MASTER §2) |
| 4 | Under budget | expense, `diff` positive | `success`, `ArrowDown`, visually hidden "bajo el presupuesto" |
| 5 | Income above plan | income, `diff` negative | `success`, `ArrowUp`, visually hidden "sobre lo planificado" |
| 6 | Income below plan | income, `diff` positive | `muted-foreground`, `ArrowDown`, visually hidden "bajo lo planificado" (not an error: pay may still arrive) |

Every state has an icon, a sign or a label besides color (MASTER §1.3).

## Copiar mes anterior

Outline button, month mode only: `budget_copy_month {from: <previous month>, to: <shown
month>, overwrite: false}` (the previous month comes from the period helper). It fills only
empty slots, so no confirmation (DEC-30 Q9). While running: disabled with `aria-busy`. The
command answers the number of lines written; Sonner default toast (no undo in M2): "Se
copiaron {n} montos de {mes anterior}" (singular "Se copió 1 monto…"), or "No había montos
nuevos para copiar" for 0. `NOT_IMPLEMENTED` → the not-available toast; other errors →
`toast.error` (MASTER §7.2).

## Excluded currencies (shared, DEC-29 Q8)

When a DTO's `excluded` list is not empty, one line above the region it covers: Lucide `Info`
(14px) + `text-xs text-muted-foreground` "{count} movimientos en {currency} sin convertir"
(singular "1 movimiento…"), entries joined with " · ", then the period the DTO covers ("en
2026" here, since `budget_matrix` counts the whole year, in both modes), then "No se incluyen
en los totales." It is information, never `warning` or `destructive`: M4 converts them.

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the matrix region.
- **No plan yet** (every planned cell of the shown month is zero): the matrix still renders
  (it is the input) with one `text-sm text-muted-foreground` line above it: "Aún no hay
  presupuesto para {mes}. Escribe un monto en Planificado o copia el mes anterior."
- **Not yet available** (the read answers `NOT_IMPLEMENTED`): the region shows the Hourglass
  block; the mode toggle, stepper and "Copiar mes anterior" are disabled.
- **Load error:** `ErrorAlert` with the per-code copy and an outline "Reintentar".

## Keyboard (shared, MASTER §5, NFR-11)

- The treegrid is one tab stop with roving focus: ↑ ↓ ← → move between cells, Home/End to the
  row's first/last cell. On a category cell, → expands and ← collapses (← on a collapsed or
  leaf row moves to its parent row).
- Enter: edits a Planificado cell, opens a Real cell's drill-down, toggles a group.
- Esc cancels an edit. Tab leaves the grid (while editing, Tab commits and moves right).
- Focus ring per MASTER §5, inset (`ring-inset`) so sticky cells don't clip it.
