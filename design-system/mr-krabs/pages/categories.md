# Page: Categorías

Overrides and extends [MASTER.md](../MASTER.md) for category management (FR-BU-01, FR-BU-07;
DEC-30 Q10). Data: `category_tree` → `CategoryNodeDto[]`, `category_create`, `category_update`,
`category_move`, `category_archive`, `category_delete` (C0 contract). Grid rules not stated
here are budget.md's **Report grid (shared)**; the departures are listed under Keyboard.

## Layout

```
Categorías                                [Gastos | Ingresos]   [+ Nueva categoría]
┌──────────────────────────────────────────────────────────────────────────┐
│ ▾ Comida                                              [+] [✎] [⋯]        │
│     Supermercado                                                         │
│     Delivery                                                             │
│ ▸ Transporte                                                             │
│   Ocio  [Archive icon]                                                   │  archived, muted
└──────────────────────────────────────────────────────────────────────────┘
                                                        Mostrar archivadas
```

- **Header row:** MASTER §5.4. Controls: the kind `ToggleGroup` "Gastos | Ingresos" (default
  Gastos); the page's one primary button "Nueva categoría" (`Plus`).
- One `Card` (border, no shadow) holds the selected kind's tree, max width 720px.
- Under the card, right-aligned: the ghost small toggle "Mostrar archivadas" / "Ocultar
  archivadas" with `aria-pressed`, as MASTER §5.1.

## The tree

- A `role="treegrid"` (rows have actions, so not a `tree`) with four columns: the name
  (`role="rowheader"`) and one gridcell per action. No section rows (one kind per tab).
- **Rows:** 36px, `text-sm`, in `category_tree`'s nesting and order (X-16: orphan and cycle rows
  come back as roots and are shown as roots). `aria-level` is the row's nesting depth + 1 in
  that tree; `aria-setsize`/`aria-posinset` count **visible** siblings (archived rows hidden by
  the toggle don't count). Parents start expanded; `aria-expanded`, the decorative chevron and
  indentation `12px + 16px × (aria-level − 1)` as the shared rule. Clicking the chevron toggles.
- **Action cells:** each holds one ghost icon button, visible on row hover or when the row has
  focus: `Plus` "Nueva subcategoría en {name}", `Pencil` "Renombrar {name}", and
  `MoreHorizontal` "Más acciones para {name}", a menu with "Mover…", "Archivar" or
  "Desarchivar", a separator, and "Eliminar…" in `destructive` text (MASTER §5: destructive
  actions live in menus).
- **Archived** rows (hidden unless "Mostrar archivadas"): name in `muted-foreground` + Lucide
  `Archive` with a visually hidden "Archivada" (MASTER §5.1). A row is shown archived when its
  own `isArchived` is true or an ancestor's is (DEC-30 Q10: archiving cascades for pickers).
  "Desarchivar" sits on a row whose own `isArchived` is true; a row archived only through an
  ancestor offers "Desarchivar «{ancestro}»" instead, which unarchives that ancestor. Archived
  rows offer only that and "Eliminar…".
- Categories show no icon in M2's UI; `category_update`'s `icon` is not edited here.

## Create

"Nueva categoría" or a row's `Plus` opens a `Dialog` (MASTER §7.1 fields):
- **Nombre** (text, required).
- **Dentro de** — searchable `Select`: "Nivel superior (grupo)" and the kind's live categories;
  preset to the row when opened from its `Plus`.
- The kind is the selected tab, shown as read-only text "Tipo: Gasto" / "Tipo: Ingreso" (kind
  is immutable, DEC-30 Q10).
- Footer: "Cancelar" (outline) and "Crear" (primary). On success: the dialog closes, the tree
  refetches, and the row with the returned `CategoryDto.id` gets focus and is scrolled into
  view; no toast.
- `VALIDATION` on `name` (empty, over 80 characters, a live sibling with that name) shows under
  Nombre; on `parentId` (an unknown or archived parent, or one of the other kind) under Dentro
  de (MASTER §7.1). Other errors are the form-level message; `NOT_IMPLEMENTED` is MASTER §7.2's
  dialog message.

## Rename

`Pencil`, or Enter on a focused name cell (Finder's convention), turns the name into an inline
input (`h-8`, text selected). Enter or blur saves (`category_update {id, name}`); unchanged
text sends nothing; Esc cancels. Results follow budget.md's Planificado rules: `VALIDATION`
keeps the input open with focus and MASTER §7.2's cell error popover; `NOT_IMPLEMENTED` closes
it with the not-available toast; other errors close it with `toast.error`. A rename shows
everywhere at once, past periods included (FR-BU-07).

## Move and reorder

- **Keyboard:** ⌥↑ / ⌥↓ on a focused name cell move the row one place among its siblings
  (`category_move {id, parentId, index}`, `index` = the row's position in the resulting sibling
  list, 0-based). At the first or last sibling it does nothing. After Rust acknowledges, the
  tree refetches, focus stays on the row, and a visually hidden `aria-live="polite"` region says
  "{name}: posición {n} de {m}". A failure says nothing there and shows `toast.error` (or the
  not-available toast). No drag and drop in M2.
- **"Mover…"** opens a `Dialog`: "Dentro de" ("Nivel superior (grupo)" and the kind's live
  categories; the category itself and its descendants are not offered) and "Posición" (native
  `<select>`: "Al principio", "Después de {hermano}" for each other sibling under the chosen
  parent, default "Al final"). "Mover" (primary). A rejected move (`VALIDATION`) is the
  form-level message.

## Archive and unarchive

"Archivar" calls `category_archive {id, archived: true}` with no confirmation (it is
reversible) and shows MASTER §7.2's undo toast: "Categoría archivada" (or "Categoría y
subcategorías archivadas" for a parent), `Deshacer` = `archived: false` on the same id, 8 s.
If the row disappears (archived rows hidden), focus moves to the next visible row, else the
previous, else the "Nueva categoría" button. Archived categories leave every picker but stay
in history and reports. "Desarchivar" restores without a toast.

## Delete with merge (FR-BU-07)

"Eliminar…" opens a `Dialog`:
- Title "Eliminar «{name}»"; body "Todo lo que usa esta categoría (subcategorías, movimientos,
  también los eliminados que aún se pueden restaurar, presupuestos y otras referencias) pasará a
  la categoría que elijas. No se puede deshacer." Deleted transactions count as uses so they stay
  restorable (X-18).
- **Mover a** — searchable `Select` of the kind's live categories, excluding the category and
  its descendants (DEC-30 Q10). It may be left empty when the category is unused.
- Footer: "Cancelar" (outline, default focus) and "Eliminar" (`destructive` variant):
  `category_delete {id, mergeIntoId?}`.
- When Rust answers `VALIDATION` because the category is in use — whether `fields` names
  `mergeIntoId` or carries no field — the message shows under Mover a: "Esta categoría está en
  uso (puede ser por movimientos eliminados): elige dónde mover lo que la usa." (cross-slice ask to C0/K1: attach the error to
  `mergeIntoId`).
- On success: the dialog closes, the tree refetches, focus moves to the previous row (else the
  next, else the "Nueva categoría" button), and a Sonner default toast "Categoría eliminada"
  (no undo).

## States (MASTER §7.2)

- **Loading:** "Cargando…" in the card with `aria-busy`.
- **Empty** (no live categories of the kind): `Inbox` block "Aún no hay categorías de
  {gastos|ingresos}. Crea la primera con «Nueva categoría»."
- **Not yet available:** for the read, the Hourglass block in the card and the primary button
  disabled; for a command in a dialog, the dialog's message; for archive, rename or keyboard
  move, the not-available toast.
- **Errors outside a form** (archive, keyboard move): `toast.error` per MASTER §7.2.

## Keyboard

The shared treegrid keys (budget.md), with these page-specific departures, stated as overrides:
- **Enter on a name cell renames** (instead of toggling); toggling is → / ← and the chevron.
- **⌥↑ / ⌥↓** reorder (above).
- **⌘Z** runs the visible archive toast's `Deshacer`, if one is showing; otherwise it does
  nothing on this page (category deletes cannot be undone).
- Every row shortcut **ignores key events that come from an input** (the inline rename), so
  text editing keeps the macOS keys. There is no delete shortcut.
