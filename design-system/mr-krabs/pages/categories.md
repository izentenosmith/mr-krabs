# Page: Categorías

Overrides and extends [MASTER.md](../MASTER.md) for category management (FR-BU-01, FR-BU-07;
DEC-30 Q10). Data: `category_tree`, `category_create`, `category_update`, `category_move`,
`category_archive`, `category_delete`.

## Layout

```
Categorías                                [Gastos | Ingresos]   [+ Nueva categoría]
┌──────────────────────────────────────────────────────────────────────────┐
│ ▾ Comida                                              [+] [✎] [⋯]        │
│     Supermercado                                                         │
│     Delivery                                                             │
│ ▸ Transporte                                                             │
│   Ocio  [Archive]                                                        │  archived: muted + icon
└──────────────────────────────────────────────────────────────────────────┘
                                                        Mostrar archivadas
```

- **Header row** (44px): title, the kind `ToggleGroup` "Gastos | Ingresos" (default Gastos),
  and the page's primary button "Nueva categoría" (`Plus`). One primary per view (MASTER §5).
- One `Card` (border, no shadow) holds the tree of the selected kind, max width 720px.
- Below the card, right-aligned: the ghost small toggle "Mostrar archivadas" / "Ocultar
  archivadas" with `aria-pressed`, as MASTER §5.1.

## The tree

- `role="tree"`; each row a `treeitem` with `aria-level`, `aria-expanded` on parents,
  36px, `text-sm`, indentation `12px + 16px × depth`, order as `category_tree` returns.
  Parents start expanded; the disclosure is MASTER's chevron (as budget.md).
- **Row actions** on hover or focus (ghost icon buttons, in the tab order only for the
  focused row): `Plus` "Nueva subcategoría en {name}", `Pencil` "Renombrar {name}", and
  `MoreHorizontal` "Más acciones para {name}", a menu with "Mover…", "Archivar" or
  "Desarchivar", a separator, and "Eliminar…" in `destructive` text (MASTER §5: destructive
  actions live in menus).
- **Archived** rows (hidden unless "Mostrar archivadas"): name `muted-foreground`, Lucide
  `Archive` with a visually hidden "Archivada" (MASTER §5.1). An archived parent's subtree
  shows archived too (DEC-30 Q10: archiving cascades for pickers). Archived rows offer only
  "Desarchivar" and "Eliminar…".
- Categories have no icon in M2's UI; `category_update`'s `icon` is not edited here.

## Create

"Nueva categoría" or a row's `Plus` opens a `Dialog` (MASTER §7.1 fields):
- **Nombre** (text, required).
- **Dentro de** — searchable `Select` of the same kind's live categories, plus "Nivel
  superior (grupo)"; preset to the row when opened from a row's `Plus`.
- The kind is the selected tab, shown as read-only text "Tipo: Gasto" (kind is immutable,
  DEC-30 Q10).
- Footer: "Cancelar" (outline) and "Crear" (primary). On success: the dialog closes, the
  tree refetches, the new row is focused and scrolled into view; no toast.
- `VALIDATION` on `name` (empty, too long, a live sibling with that name) shows under Nombre
  (MASTER §7.1); other errors as the form-level message.

## Rename

`Pencil`, or Enter on a focused row (Finder's convention), turns the name into an inline
input (`h-8`, text selected). Enter saves (`category_update {id, name}`), Esc cancels,
blur saves. `VALIDATION` keeps the input open with the field error under it (MASTER §7.1).
A rename shows everywhere at once, past periods included (FR-BU-07).

## Move and reorder

- **Keyboard:** ⌥↑ / ⌥↓ move the focused row one place among its siblings
  (`category_move {id, parentId, index}`); focus stays on it. No drag and drop in M2.
- **"Mover…"** opens a `Dialog`: "Dentro de" (same kind; the category itself and its
  descendants are not offered) and "Posición" (native `<select>`: "Al principio", "Después de
  {hermano}" for each sibling of the chosen parent, "Al final", default "Al final"). "Mover"
  (primary). A rejected move (`VALIDATION`) shows as the form-level message.

## Archive and unarchive

Menu "Archivar" calls `category_archive {id, archived: true}` with no confirmation (it is
reversible) and shows MASTER §7.2's undo toast: "Categoría archivada" (or "Categoría y
subcategorías archivadas" for a parent), `Deshacer` = unarchive, 8 s. Archived categories
leave every picker but stay in history and reports. "Desarchivar" restores without a toast.

## Delete with merge (FR-BU-07)

"Eliminar…" opens a `Dialog`:
- Title "Eliminar «{name}»"; body "Sus movimientos y presupuestos pasarán a la categoría
  que elijas. No se puede deshacer."
- **Mover a** — searchable `Select` of the same kind's live categories, excluding the
  category and its descendants (merge rules, DEC-30 Q10); optional when the category has
  nothing to move, and then the body's first sentence is still shown.
- Footer: "Cancelar" (outline, default focus) and "Eliminar" (`destructive` variant).
  `category_delete {id, mergeIntoId?}`.
- When Rust answers that a target is needed (`VALIDATION` on `mergeIntoId`), the field shows
  "Esta categoría tiene movimientos o presupuestos: elige dónde moverlos."
- On success: the dialog closes, the tree refetches, focus moves to the previous row, and a
  Sonner default toast "Categoría eliminada" (no undo).

## States (MASTER §7.2)

- **Loading:** "Cargando…" in the card with `aria-busy`.
- **Empty** (no live categories of the kind): `Inbox` block "Aún no hay categorías de
  {gastos|ingresos}. Crea la primera con «Nueva categoría»."
- **Not yet available:** Hourglass block in the card for the read; for a command it is the
  dialog's form-level message (dialog stays open) or, for archive/move outside a dialog, a
  `toast` with the same text; the primary button is disabled when the read is unavailable.
- **Errors outside a form** (archive, keyboard move): `toast.error` per MASTER §7.2.

## Keyboard

Tree pattern (WAI-ARIA): ↑ ↓ move, → expands or enters, ← collapses or goes to the parent,
Home/End first/last row, Enter renames, ⌥↑ / ⌥↓ reorder, ⌘⌫ opens "Eliminar…", Esc closes
any dialog or cancels a rename.
