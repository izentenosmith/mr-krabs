# Page: Calendario (month day grid)

Overrides and extends [MASTER.md](../MASTER.md) for the calendar page (FR-RP-06, the
spreadsheet's day grid; DEC-29 Q6). Data: `report_month_calendar` → `CalendarDto` (C0);
quick-add reuses the transaction dialog (`tx_create`). Everything not stated here follows
MASTER and the rules marked **shared** in [budget.md](budget.md).

## Layout

```
Calendario                                         ‹ Noviembre 2026 ›
ⓘ 1 movimiento en USD sin convertir en noviembre · No se incluyen en los totales.
┌────────────────────┬────────┬────────┬─── … ───┬────────┬────────────┐
│ Categoría          │  1  L  │  2  M  │         │ 30  L  │      Total │  sticky header
├────────────────────┼────────┼────────┼─── … ───┼────────┼────────────┤
│ Gastos             │        │        │         │        │            │  section row
│ ▾ Comida           │$12.000 │        │         │        │   $655.000 │  group (roll-up)
│     Supermercado   │        │        │         │        │   $520.000 │
│ Ingresos           │        │        │         │        │            │  section row
│     Sueldo         │        │        │         │$3.000.000│ $3.000.000│
├────────────────────┼────────┼────────┼─── … ───┼────────┼────────────┤
│ Gastos del día     │$12.000 │        │         │        │            │  sticky bottom
│ Ingresos del día   │        │        │         │$3.000.000│          │  sticky bottom
└────────────────────┴────────┴────────┴─── … ───┴────────┴────────────┘
```

- **Header row:** MASTER §5.4, with the period stepper (months; default the current month).
  `view: "combined"` (as budget.md).
- The excluded-currency line is budget.md's (shared), naming the month.

## The grid

- `role="treegrid"`, rows in the order of `CalendarDto.rows`. Row semantics, section rows,
  group rows, indentation, collapse, archived styling and sticky rules are budget.md's
  (shared).
- **Columns:** Categoría (min 200px) · one per day `1…daysInMonth` (min 80px, growing to fit
  the widest `display` in the column, never truncated) · Total (min 120px, also growing).
  The day header shows the day number (`text-xs font-medium`) and the weekday initial
  (`text-xs text-muted-foreground`, UI language); `aria-label` is the full date. Today's
  header: `bg-accent` and `aria-current="date"`. Weekends are not tinted (MASTER §1.2).
- **Sticky on all four sides:** header row (top), the two day-total rows (bottom), Categoría
  (left) and Total (right, `border-l`); all `bg-card` (group rows keep `bg-muted`), z 10,
  corners z 11.
- **Cells:** `text-[13px] tabular-nums`, right-aligned, `rows[i].days[d].display` as received.
  An empty day (`minor` 0) is blank, so the month reads like the spreadsheet. Credits as
  budget.md (shared). The Total column shows `rows[i].total`.
- **Day totals:** two rows, "Gastos del día" (`dayTotals.expense[d]`) and "Ingresos del día"
  (`dayTotals.income[d]`), `font-medium`, blank when zero. The UI never sums.

## Quick-add (DEC-29 Q6)

- Activating a **day cell** (click or Enter) opens the existing `TxDialog` in create mode with
  that date. TxDialog gains one additive create-mode prop, `preset?: { date: string;
  categoryId?: string }`; its fields, validation and errors are unchanged. The cell passes its
  row's category when the category is live, group rows included (any category can take
  transactions, FR-BU-01), so the type follows the category's kind. An archived row passes
  the date only (archived categories leave every picker, DEC-30 Q10).
- A **day header** opens the dialog with that date and no category.
- Cells and headers are buttons, `aria-label` "Agregar movimiento el {día} de {mes}" (plus
  ", {categoría}" for a cell). Hover and focus show a ghost `Plus` (12px, `muted-foreground`)
  at the cell's top-left; the amount stays right-aligned.
- On save the dialog closes as it does in Transacciones, the grid refetches and focus returns
  to the cell that opened it.

## Drill-down (shared, budget.md)

- A **Total** cell opens Transacciones for the row's `categoryId` (with descendants) and the
  month. A **day-total** cell opens it for that day (`day=YYYY-MM-DD`, all categories). Note
  that Transacciones also lists transfers and non-base-currency entries, which the totals
  leave out.

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the grid region.
- **Empty month:** the grid renders (blank cells are the quick-add targets) with one
  `text-sm text-muted-foreground` line above it: "Sin movimientos en {mes}. Elige un día
  para agregar uno."
- **Not yet available:** Hourglass block in the region; stepper disabled.
- **Load error:** `ErrorAlert` + outline "Reintentar".

## Keyboard

budget.md's treegrid keys (shared), with Enter activating quick-add or a drill-down. No other
keys are overridden. Esc in the dialog closes it (MASTER §5).
