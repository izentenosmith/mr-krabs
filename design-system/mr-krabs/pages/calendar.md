# Page: Calendario (month day grid)

Overrides and extends [MASTER.md](../MASTER.md) for the calendar page (FR-RP-06, the
spreadsheet's day grid; DEC-29 Q6). Data: `report_month_calendar`; quick-add reuses the
transaction dialog (`tx_create`). Everything not stated here follows MASTER and
[budget.md](budget.md) for row styling.

## Layout

```
Calendario                                   ‹ Noviembre 2026 ›
ⓘ 1 movimiento en USD sin convertir · No se incluyen en los totales.
┌────────────────────┬────┬────┬────┬─── … ───┬────┬──────────┐
│ Categoría          │ 1 L│ 2 M│ 3 M│         │30 D│    Total │  sticky header
├────────────────────┼────┼────┼────┼─── … ───┼────┼──────────┤
│ GASTOS             │    │    │    │         │    │          │
│ ▾ Comida           │12.0│    │45.0│         │    │  655.000 │  group subtotal
│     Supermercado   │    │    │45.0│         │    │  520.000 │
│ …                  │    │    │    │         │    │          │
├────────────────────┼────┼────┼────┼─── … ───┼────┼──────────┤
│ Total del día      │12.0│    │45.0│         │    │  655.000 │  sticky bottom row
└────────────────────┴────┴────┴────┴─── … ───┴────┴──────────┘
```
(Cells are abbreviated in this sketch only; the grid shows the full `display`.)

- **Header row** (44px): title `text-xl font-semibold`, then the period stepper (MASTER
  §5.4, months). Default: the current month. `view: combined` (as budget.md).
- The excluded-currency line is the same as budget.md's.

## The grid

- `role="grid"`; rows in the order `report_month_calendar` returns. Section rows ("Gastos",
  then "Ingresos"), group subtotal rows (`isGroup`), indentation, collapse and archived styling
  follow budget.md exactly.
- **Columns:** Categoría (min 200px) · one per day `1…n` (72px) · Total (112px).
  The day header shows the day number (`text-xs font-medium`) over the weekday initial
  (`text-xs text-muted-foreground`, from the UI language). Today's header cell: `bg-accent`
  and `aria-current="date"`. Weekends are not tinted (MASTER §1.2).
- **Sticky on all four sides:** header row (top), "Total del día" row (bottom), Categoría
  column (left) and Total column (right, `border-l`), all `bg-card` (group rows keep
  `bg-muted`), z 10; corner cells stick on both axes.
- **Cells:** `text-[13px] tabular-nums`, right-aligned, `display` as received. An empty day
  is blank (no "0"), in the rows and in "Total del día", so the month reads like the
  spreadsheet. Credits as budget.md.
- Row and day totals come from the DTO; the UI never sums.

## Quick-add (DEC-29 Q6)

- Activating a **day cell** (click or Enter) opens the existing transaction dialog
  (`TxDialog`, create mode) with `date` = that day and the row's category preselected; the type
  follows the category's kind (expense/income). A **day header** opens it with that date and
  no category. Nothing else changes in the dialog: same fields, validation and errors.
- Cells and headers are buttons with `aria-label` "Agregar movimiento el {día} de {mes}" (with
  ", {categoría}" for a cell). Hover/focus shows a ghost `Plus` (12px, `muted-foreground`)
  at the cell's top-left; the amount stays right-aligned.
- On save the dialog closes with the dialog's usual confirmation, the grid refetches and
  focus returns to the cell that opened it.

## Drill-down

- A **Total** cell opens Transacciones filtered by the row's `categoryId` (with descendants)
  and the month; a **Total del día** cell opens it for that day (all categories). Link-style
  buttons as budget.md; ranges come from the DTO.

## States (MASTER §7.2)

- **Loading:** "Cargando…" with `aria-busy` on the grid region.
- **Empty month:** the grid renders (blank cells are the quick-add targets) with one
  `text-sm text-muted-foreground` line above it: "Sin movimientos en {mes}. Elige un día
  para agregar uno."
- **Not yet available:** Hourglass block in the region; stepper disabled.
- **Load error:** `ErrorAlert` + outline "Reintentar".

## Keyboard

- One tab stop, roving focus: arrows move by cell, Home/End to the row's first/last day,
  PageUp/PageDown move by 7 days (a week). Enter activates (quick-add or drill-down).
- Group disclosure and collapse as budget.md. Esc in the dialog closes it (MASTER §5).
