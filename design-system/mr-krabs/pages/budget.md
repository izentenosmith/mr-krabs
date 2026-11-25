# Page: Presupuesto (plan-vs-real matrix)

Overrides and extends [MASTER.md](../MASTER.md) for the budget page (FR-BU-02, FR-BU-03,
FR-BU-08, FR-RP-07; DEC-29, DEC-30). Data: `budget_matrix` → `BudgetMatrixDto`, `budget_set`,
`budget_copy_month` (C0 contract, `packages/api-client/src/bindings.ts`, with X-17's additions;
exact field names as #37 merges them). Everything not stated here follows MASTER.

Sections marked **(shared)** are the report-grid rules that `calendar.md`, `dashboard.md` and
`categories.md` reuse; a page that departs from one says so.

## Layout

```
Presupuesto                     [Mes | Año]   ‹ Noviembre 2026 ›   [Copiar mes anterior]
ⓘ 3 movimientos en USD sin convertir en 2026 · No se incluyen en los totales.
┌──────────────────────────┬─────────────┬──────────────────┬──────────────────────┐
│ Categoría                │ Planificado │             Real │           Diferencia │
├──────────────────────────┼─────────────┼──────────────────┼──────────────────────┤
│ ▾ Ingresos               │             │                  │                      │  section
│   ▾ Ingresos             │ $3.200.000  │       $3.150.000 │ ↓ $50.000            │  group
│       Sueldo             │ $3.000.000  │       $3.000.000 │ $0                   │
│ ▾ Gastos                 │             │                  │                      │  section
│   ▾ Comida               │   $600.000  │         $655.000 │ ⊘ -$55.000 excedido  │  group
│       Supermercado       │   $450.000  │         $520.000 │ ⊘ -$70.000 excedido  │
│       Delivery           │    $80.000  │ -$12.000 crédito │ ↓ $92.000            │  credit
├──────────────────────────┼─────────────┼──────────────────┼──────────────────────┤
│ Total ingresos           │ $3.200.000  │       $3.150.000 │ ↓ $50.000            │  summary
│ Total gastos             │ $2.300.000  │       $2.410.000 │ ⊘ -$110.000 excedido │
│ Ahorro e inversión       │           — │         $400.000 │                    — │
│ Flujo libre              │           — │         $340.000 │                    — │
└──────────────────────────┴─────────────┴──────────────────┴──────────────────────┘
```

(`⊘` stands for Lucide `CircleAlert`, `↓` for `ArrowDown`. Values are `display` strings as Rust
formats them, sign included.)

- **Header row:** MASTER §5.4. Controls: the mode `ToggleGroup` "Mes | Año", the period
  stepper, and in month mode the outline button "Copiar mes anterior". This page has no
  primary button (MASTER §5).
- **View and scope.** The page reads `view: "combined"` (as MASTER §5.1) and in-cell edits
  write `scope: "household"`. M2 has no way to write a personal line, so in M2 every planned
  cell equals the household line it edits. Per-view editing (personal lines, the lens
  switcher) is decided with FR-HH-03 in M3.
- The matrix scrolls inside its own region (both axes); the header row and the notice stay put.

## Report grid (shared)

### Structure

- TanStack Table rendering a `role="treegrid"` (the calendar and categories pages use the same
  structure). Rows 36px, cells `text-[13px] tabular-nums`, amounts right-aligned.
- **Row order:** category rows keep the DTO's order. Before each run of rows of one
  `category.kind`, the grid inserts a **section row** ("Ingresos" or "Gastos").
- **Levels:** section rows are `aria-level="1"`, parents of their run; a category row is
  `aria-level` = `category.depth + 2`. `aria-setsize` and `aria-posinset` count the row's
  visible siblings (rows under the same parent row). Summary and day-total rows are
  `aria-level="1"` rows after the sections.
- **Cells:** the first cell of every row is `role="rowheader"`. A section row has only that
  cell (spanning the row; its label `text-xs font-medium text-muted-foreground`, no amounts).
- **Expand and collapse:** section rows and groups (`category.isGroup`) carry `aria-expanded`
  on the row and start expanded; the state is per window and not persisted. The chevron
  (`ChevronDown` / `ChevronRight`, 16px) is decorative (`aria-hidden`); clicking it or the
  row header cell toggles.
- **Indentation:** `12px + 16px × (aria-level − 1)` in the row header cell; min width 240px;
  the name truncates with the full name in `title`.
- **Sticky:** the header row and the first column, z 10 (`border-r` on the column); the
  corner cell z 11. A sticky cell takes its row's background (`bg-card`, or `bg-muted` on a
  group row).
- **Group rows** (`category.isGroup`): `bg-muted`, `font-medium`; their amounts are roll-ups
  (own line + every descendant, DEC-30 Q9).
- **Archived categories:** a row whose `category.isArchived` is true is shown archived: name
  in `muted-foreground` + Lucide `Archive` (14px) with a visually hidden "Archivada" (MASTER
  §5.1). Report rows treat `isArchived` as *effective* (true when the category or an ancestor
  is archived) — cross-slice ask to C0/R1 to define it so.

### Amounts and credits

- Every amount is `MoneyDto.display` as received, sign included (MASTER §3), never
  recomputed or re-signed. In a report grid every amount is `foreground`: the section row
  names the kind, so the income/expense colors of MASTER §2 do not apply here (MASTER §2,
  "Report grids").
- **Credit:** in an expense row (`category.kind = "expense"`), an actual whose `minor` is
  negative (refunds larger than spending) shows its `display` followed by a visible `text-xs
  text-muted-foreground` "crédito", with the tooltip "Las devoluciones superaron los gastos".
  It is neither `success` nor `destructive`: nothing is wrong and it is not income. In an
  income row a negative actual is shown as received, with no label.

### Drill-down

- Only the cells a page names drill down. Each is a **link** (TanStack Router `Link`, so it is
  in the VoiceOver rotor's links list), `text-foreground` (an override of MASTER's `primary`
  links: these are amounts, MASTER §2), underline on hover and focus.
- Its target is Transacciones with `categoryId` (with descendants, `TxQuery.categoryId`) and a
  period: `month=YYYY-MM`, `day=YYYY-MM-DD` or `year=YYYY`. The transactions page builds
  `from`/`to` with the period helper (MASTER §5.4, X-17).
- Its accessible name starts with the amount: "{display}, ver movimientos de {categoría},
  {periodo}".
- Transacciones lists every entry, transfers and other currencies included; a drill-down
  total can therefore differ from the cell.

### Excluded currencies (DEC-29 Q8)

When a DTO's `excluded` list is not empty, one line above the region it covers: Lucide `Info`
(14px) + `text-xs text-muted-foreground` "{count} movimientos en {currency} sin convertir"
(singular "1 movimiento…"), entries joined with " · ", then the period the DTO covers (for
example "en 2026"), then "No se incluyen en los totales." It is information, never `warning`
or `destructive`: M4 converts them.

### Keyboard (MASTER §5, NFR-11, WAI-ARIA treegrid)

- The grid is one tab stop with roving focus on cells. ↑ ↓ move to the same column in the
  previous/next visible row (on a section row, to its only cell). Home/End go to the row's
  first/last cell; Ctrl+Home/Ctrl+End to the first/last row.
- On a row header cell, → expands a collapsed row, otherwise moves one cell right; ← collapses
  an expanded row, otherwise moves to the parent row. In any other cell, ← → move one cell.
- Enter on a link follows it; Enter on a row header toggles it. Pages add their own Enter
  actions on their cells.
- Tab leaves the grid. The focus ring is MASTER §5's, inset (`ring-inset`) so sticky cells
  don't clip it.
- After a refetch, focus stays on the same row (by category id) and column.

## The matrix

- **Summary rows** after a 2px `border-t`, from `BudgetMatrixDto.summary`: "Total ingresos"
  (`income`), "Total gastos" (`expense`; X-17 drops the separate total-expenses field),
  "Ahorro e inversión" (`saved`) and "Flujo libre" (`freeCashFlow`). `font-medium`, read-only,
  no drill-down. Budgets are per category (DEC-30 Q9), so "Ahorro e inversión" and "Flujo
  libre" have no plan: their Planificado and Diferencia cells show "—" in `muted-foreground`
  with a visually hidden "Sin presupuesto". A negative "Flujo libre" actual uses `destructive`
  + `CircleAlert` + visible "déficit".

### Month mode ("Mes", default)

Columns: Categoría · Planificado · Real · Diferencia (136px each; Real grows to fit
"crédito"), from `rows[i].months[m]` (`planned`, `actual`, `diff`) for the shown month. The
stepper steps months; default the current month. Real cells drill down (`month`).

### Year mode ("Año")

- Columns: Categoría · Ene … Dic · Total — **actuals only** (DEC-30 Q11), 112px per month,
  Total 128px `font-medium`. Short month names from the i18n catalogue, with the full name as
  the column header's `aria-label`. Total is the row's `year` cell actual from Rust (X-17);
  the UI never adds months.
- The stepper steps years and keeps the shared month (MASTER §5.4): the year shown is that
  month's year.
- The current month's header has `bg-accent` and `aria-current="date"`. "Copiar mes anterior"
  is hidden.
- Month cells drill down (`month`), Total cells too (`year`). Credits, archived styling and the
  summary rows (actuals only, "déficit" per month) work as in month mode. The "no plan yet"
  line is not shown in year mode.
- At 1024×700 the table scrolls horizontally behind the sticky column.

FR-BU-03's "% used" is not a column of this page (DEC-30 Q11 fixed the columns); it is an M2
follow-up for the nexus.

## Planificado cells (editing)

- A gridcell holding a button. On a leaf it shows `months[m].planned` (blank when `minor` is
  0). On a group it shows the roll-up and edits the group's **own** line (`ownPlanned[m]`), with
  a `text-xs text-muted-foreground` hint under the input: "Monto propio del grupo; el total
  suma sus subcategorías." Archived rows are editable too; if Rust rejects a line on an
  archived category, that is its `VALIDATION`.
- **Start:** Enter, double-click or typing a digit. The cell becomes a `MoneyInput` (MASTER
  §7.1, `h-8`) seeded with the cell's `display` (blank when `minor` is 0), text selected;
  typing a digit replaces it. While editing, every key goes to the input (arrows, Home and End
  move the caret).
- **Commit:** Enter (then focus moves to the cell below), Tab / Shift+Tab (then right / left),
  or blur. It sends `budget_set {period, categoryId, scope, amount}` with the text as typed;
  empty or 0 deletes the line (DEC-30 Q9). Unchanged text sends nothing. Esc cancels.
- **Saving:** the input is read-only with `aria-busy`; no spinner. Saved: the matrix refetches
  and the cell shows the new value; no toast.
- **`VALIDATION`:** the input stays open with the typed text and gets focus back (also after a
  blur commit), `aria-invalid`, `border-destructive`, and MASTER §7.2's cell error popover.
  Committing the same text again sends nothing; editing it clears the error.
- **`NOT_IMPLEMENTED`:** the cell returns to its old value and MASTER §7.2's not-available
  toast shows; never `toast.error`.
- **Any other error:** the cell returns to its old value and `toast.error` (MASTER §7.2).

## Diferencia cells — `VarianceCell`

Δ = planned − actual, computed in Rust (`diff`), shown as its `display` (signed: over budget is
negative). Icons are 12px Lucide; `ArrowDown`/`ArrowUp` stand for MASTER §7's ▼/▲ and mean
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

"Total ingresos" and "Total gastos" use the same table with their kind. Every state has an
icon, a sign or a label besides color (MASTER §1.3).

## Copiar mes anterior

Outline button, month mode only: `budget_copy_month {from: <previous month>, to: <shown
month>, overwrite: false}` (the previous month comes from the period helper). It fills only
empty slots, so no confirmation (DEC-30 Q9). While running: disabled with `aria-busy`. The
command answers the number of lines written; Sonner default toast (no undo in M2): "Se
copiaron {n} montos de {mes anterior}" (singular "Se copió 1 monto…"), or "No había montos
nuevos para copiar" for 0. `NOT_IMPLEMENTED` → the not-available toast; other errors →
`toast.error` (MASTER §7.2).

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the matrix region.
- **No plan yet** (month mode; every planned cell of the shown month has `minor` 0): the matrix
  still renders (it is the input) with one `text-sm text-muted-foreground` line above it: "Aún
  no hay presupuesto para {mes}. Escribe un monto en Planificado o copia el mes anterior."
- **Not yet available** (the read answers `NOT_IMPLEMENTED`): the region shows the Hourglass
  block; the mode toggle, stepper and "Copiar mes anterior" are disabled.
- **Load error:** `ErrorAlert` with the per-code copy and an outline "Reintentar".
