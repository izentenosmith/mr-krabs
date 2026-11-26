# Page: Calendario (month day grid)

Overrides and extends [MASTER.md](../MASTER.md) for the calendar page (FR-RP-06, the
spreadsheet's day grid; DEC-29 Q6). Data: `report_month_calendar` → `CalendarDto` (C0);
quick-add reuses the transaction dialog (`tx_create`). Everything not stated here follows
MASTER and budget.md's **Report grid (shared)**.

## Layout

```
Calendario                                          ‹ Noviembre 2026 ›
ⓘ 1 movimiento en USD sin convertir en noviembre · No se incluyen en los totales.
┌────────────────────┬──────────┬──────────┬─── … ───┬────────────┬────────────┐
│ Categoría          │   1  D   │   2  L   │         │   30  L    │      Total │
├────────────────────┼──────────┼──────────┼─── … ───┼────────────┼────────────┤
│ ▾ Gastos           │          │          │         │            │            │  section
│   ▾ Comida         │  $12.000 │          │         │            │   $655.000 │  group
│       Supermercado │          │          │         │            │   $520.000 │
│ ▾ Ingresos         │          │          │         │            │            │  section
│       Sueldo       │          │          │         │ $3.000.000 │ $3.000.000 │
├────────────────────┼──────────┼──────────┼─── … ───┼────────────┼────────────┤
│ Gastos del día     │  $12.000 │          │         │            │            │  sticky bottom
│ Ingresos del día   │          │          │         │ $3.000.000 │            │  sticky bottom
└────────────────────┴──────────┴──────────┴─── … ───┴────────────┴────────────┘
```

(Section order follows the DTO's row order; this sketch is one example.)

- **Header row:** MASTER §5.4 with the period stepper (months; default the current month).
  `view: "combined"` (as budget.md).
- The excluded-currency line (shared) names the month.

## The grid

- budget.md's report grid (shared): treegrid structure, section rows, levels, group rows,
  archived styling, amounts, credits, sticky rules and keyboard.
- **Columns:** Categoría (min 200px) · one per day `1…daysInMonth` (min 80px, growing to fit
  the widest `display` in the column, never truncated) · Total (min 120px, also growing).
- **Day headers** (`role="columnheader"`): the day number (`text-xs font-medium`) over the
  weekday initial (`text-xs text-muted-foreground`, from the i18n catalogue); the header's
  accessible name is the full date ("1 de noviembre"). Today's header: `bg-accent` and
  `aria-current="date"`. Weekends are not tinted (MASTER §1.2). Inside the header sits the
  quick-add button (below).
- **Sticky on all four sides:** header row (top), the two day-total rows (bottom), Categoría
  (left) and Total (right, `border-l`); z 10, corners z 11; backgrounds as the shared rule.
- **Cells:** `rows[i].days[d].display` as received; an empty day (`minor` 0) is blank, so the
  month reads like the spreadsheet. The Total column shows `rows[i].total`.
- **Day totals:** two summary rows, "Gastos del día" (`dayTotals.expense[d]`) and "Ingresos del
  día" (`dayTotals.income[d]`), `font-medium`, blank when zero; a negative "Gastos del día" is a
  credit (shared rule). The UI never sums.

## Quick-add (DEC-29 Q6)

- Enter or click on a **day cell** opens the existing `TxDialog` in create mode with that date.
  TxDialog gains one additive create-mode prop, `preset?: { date: string; kind?:
  "expense" | "income"; categoryId?: string }`; its fields, validation and errors are
  unchanged. The cell passes its row's `category.kind` and `category.id` (group rows included:
  any category can take transactions, FR-BU-01); an archived row passes the date and kind only
  (archived categories leave every picker, DEC-30 Q10).
- The **day header's** button opens the dialog with that date only. The header row is in the
  grid's roving set (shared keyboard), so ↑ from the first row reaches it; Enter on a header
  activates its button.
- Accessible names start with the cell's content: "{display}, agregar movimiento el {día} de
  {mes}, {categoría}", or "Vacío, agregar movimiento el …" for a blank day; the header button
  is "Agregar movimiento el {día} de {mes}". Hover and focus show a ghost `Plus` (12px,
  `muted-foreground`) at the cell's top-left; the amount stays right-aligned.
- On save the dialog closes as it does in Transacciones, the grid refetches and focus returns
  to the cell that opened it.

## Drill-down (shared rule)

Day cells do **not** drill down (they quick-add). Drill-down links are: a row's **Total** cell
(the row's `categoryId` and `month`, name "{display}, ver movimientos de {categoría},
{mes}") and a **day-total** cell (`day`, all categories, name "{display}, ver movimientos del
{día} de {mes}").

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the grid region.
- **No amounts** (every amount in the DTO has `minor` 0 **and** `excluded` is empty): the grid
  renders (blank cells are the quick-add targets) with one `text-sm text-muted-foreground`
  line above it: "No hay montos en {mes}. Elige un día para agregar uno."
- **Not yet available:** Hourglass block in the region; stepper disabled.
- **Load error:** `ErrorAlert` + outline "Reintentar".
