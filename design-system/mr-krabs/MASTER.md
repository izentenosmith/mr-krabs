# Mr Krabs — Design System (MASTER)

Global source of truth for the desktop UI (`apps/desktop`). Tokens live in
[`apps/desktop/src/index.css`](../../apps/desktop/src/index.css); this file explains
what they mean and how to use them.

**Page overrides:** when building a page, also check `design-system/mr-krabs/pages/<page>.md`.
If it exists, its rules override this file for that page; otherwise this file applies.

Stack: Tauri v2 · React 19 · Tailwind v4 · shadcn/ui (new-york) · Radix · Lucide.

---

## 1. Principles

1. **Numbers first.** The ledger is the product. Amounts are legible, aligned and exact.
2. **Color has meaning, never decoration.** Red means a problem, not "an expense".
3. **Never color alone.** Every semantic color is paired with a sign, icon or label.
4. **Keyboard first, native feel.** It is a Mac app, not a website.
5. **Private by default.** No remote fonts, images or scripts (NFR-02).

Style: *Swiss minimal, data-dense* — clean grid, borders over shadows, no gradients,
no decorative motion.

---

## 2. Color

All components use **semantic tokens** (`bg-primary`, `text-success`, …). Never put raw hex
in a component. Light and dark follow the macOS appearance (`prefers-color-scheme`).

| Token | Light | Dark | Meaning |
|---|---|---|---|
| `background` | `#F8FAFC` | `#0F172A` | App surface (never pure white) |
| `card` / `popover` | `#FFFFFF` | `#192134` | Tables, panels, menus |
| `foreground` | `#0F172A` | `#F1F5F9` | Body text **and ordinary amounts** |
| `muted` / `secondary` / `accent` | `#F1F5F9` | `#1E293B` | Sidebar, hover, subtle fills |
| `muted-foreground` | `#475569` | `#94A3B8` | Labels, currency codes, metadata |
| `border` / `input` | `#E2E8F0` | `white / 10–15%` | Hairlines |
| `primary` | `#1E40AF` | `#60A5FA` | The single main action, focus ring, links, selection |
| `brand` (Krab coral) | `#E5533D` | `#FB7B6B` | Logo, active-nav marker, onboarding art. **Never text, never amounts** (3.7:1 — non-text only) |
| `success` (positive) | `#047857` | `#34D399` | Income, under budget, reconciled, synced |
| `destructive` (negative) | `#B91C1C` | `#F87171` | Over budget, errors, destructive actions |
| `warning` | `#B45309` | `#FBBF24` | Bill outside expected range, card near limit, sync conflict |
| `scope-personal` | `#6D28D9` | `#A78BFA` | Personal scope badge |
| `scope-household` | `#0F766E` | `#2DD4BF` | Household scope badge |

Each fill token has a `-foreground` pair for text placed on it (white in light,
`#0F172A` in dark).

**Contrast (WCAG):** every text pair above is ≥ 4.5:1 on `background` and `card` in its
mode (lowest: warning on white, 5.0:1). Re-check when adding a token.

### Money semantics

| Kind | Rendering |
|---|---|
| Expense | `foreground`, leading `−` |
| Income | `success`, leading `+` |
| Transfer between own accounts | `muted-foreground`, `⇄` icon, no sign — never styled as spending (FR-TX-02) |
| Over budget | `destructive` + alert icon + "excedido" text |
| Out-of-range bill | `warning` + alert icon + "Fuera de rango" |

### Status & scope badges (icon + label, always)

| Status | Icon (Lucide) | Color |
|---|---|---|
| Pendiente | `Clock` | `muted-foreground` |
| Liquidado | `Check` | `muted-foreground` |
| Conciliado | `CircleCheck` | `success` |
| Propuesto (AI, ADR-0006) | `Sparkles` | `primary`, row tinted `primary/5` until approved |

| Scope | Icon | Color |
|---|---|---|
| Personal | `User` | `scope-personal` |
| Hogar | `House` | `scope-household` |

---

## 3. Typography

- **Font:** macOS system stack (`font-sans`): `-apple-system, BlinkMacSystemFont, "SF Pro Text", system-ui, sans-serif`.
  Mono (`font-mono`): `ui-monospace, "SF Mono", Menlo` — IDs and raw import text only.
  **No web fonts** (offline, NFR-02).
- **Amounts:** always `tabular-nums`, right-aligned in tables. The string comes
  pre-formatted from Rust (`MoneyDto.display`); the UI only styles it.

| Role | Size / line-height | Weight | Tailwind |
|---|---|---|---|
| Caption, column headers | 12 / 16 | 500 | `text-xs` |
| Table cell | 13 / 20 | 400 | `text-[13px]` |
| Body (default) | 14 / 20 | 400 | `text-sm` |
| Section title | 16 / 24 | 600 | `text-base font-semibold` |
| Page title | 20 / 28 | 600 | `text-xl font-semibold` |
| Hero figure (net worth, cash flow) | 28 / 36 | 600 | `text-[28px] font-semibold tabular-nums` |

Minimum 12px anywhere. Body is 14px — between macOS native (13) and the web minimum (16).

---

## 4. Space, shape, elevation

- **Spacing:** 4px grid. Component gaps 8/12/16; section gaps 24/32.
- **Density:** table rows 36px; toolbar 44px; sidebar 240px; min window 1024×700.
- **Radius:** `--radius: 0.5rem` (8px) — controls `rounded-md`, cards `rounded-lg`.
- **Elevation:** borders separate in-flow content. Shadows only for floating layers:
  popover/menu `shadow-md`, dialog/sheet `shadow-lg`. Max two floating layers at once.
- **Z-index:** content 0 · sticky header 10 · sidebar 20 · popover 40 · dialog 50 · toast 100.

---

## 5. Layout & navigation

```
┌────────────┬──────────────────────────────────────────┐
│ ● Mr Krabs │  ‹ Noviembre 2026 ›   [search]  [+ Nuevo] │  toolbar
│ [Mío|Hogar|Todo]  lens switcher                        │
│ Resumen    │                                          │
│ Transacc.  │  content (tables, matrices, charts)      │
│ Presupuesto│                                          │
│ Calendario │                                          │
│ Importar   │                                          │
│ Reportes   │                                          │
│ ─ Cuentas ─│                                          │
│ balances…  │                                          │
│ ✓ Sincron. │                                          │
└────────────┴──────────────────────────────────────────┘
```

- Lens switcher (FR-HH-03) is a `ToggleGroup` at the top of the sidebar; the active lens
  persists per window.
- Active nav item: `accent` fill + 3px `brand` left marker + 500 weight.
- One primary button per view (`+ Nuevo` in the toolbar). Others are `outline`/`ghost`.
- Destructive actions live in menus or dialogs, never next to the primary button.

### Keyboard (NFR-11)

| Shortcut | Action |
|---|---|
| ⌘K | Command palette |
| ⌘N | New transaction |
| ⌘1 / ⌘2 / ⌘3 | Lens: Personal / Household / Combined |
| ⌘F | Focus search |
| ↑ ↓ / Enter / Esc | Move in tables / open / close |
| ⌘Z | Undo last delete or import (FR-TX-08, FR-IM-05) |

Focus ring always visible: `ring-[3px] ring-ring/50` (Tailwind `focus-visible:`).

---

### 5.1 Accounts sidebar
- Section header "Cuentas" (`text-xs font-medium text-muted-foreground`) with a ghost icon
  button `Plus` (`aria-label` "Nueva cuenta"). Not a primary button (the view's primary is
  the toolbar's "+ Nuevo").
- Accounts are grouped by kind, in this order, empty groups hidden; group caption `text-xs
  text-muted-foreground`:
  1. Bancos y efectivo — checking, cuenta_vista, savings, cash
  2. Tarjetas de crédito — credit_card
  3. Créditos — loan, mortgage
  4. Inversiones — investment
  5. Otros — other_asset, other_liability
  Within a group, rows keep the order `account_list` returns.
- Row (32px): name left (`text-sm`, truncate), balance right as `Amount` (`MoneyDto.display`
  as received, `tabular-nums`, `foreground`; no sign styling — a balance is not an expense).
  A personal account shows the `User` icon in `scope-personal` with a visually hidden
  "Personal" label. An edit button (ghost icon `Pencil`, `aria-label` "Editar {name}") is
  in the tab order and visible on row hover or focus.
- Archived toggle: ghost small button at the end of the section, "Mostrar archivadas" /
  "Ocultar archivadas", `aria-pressed`. Archived rows: name in `muted-foreground` plus the
  `Archive` icon with a visually hidden "Archivada" label.
- Empty: "Crea tu primera cuenta" text and an outline small button "Nueva cuenta".
- M1 has one member, so there is no lens switcher yet; the sidebar asks for `view: combined`.
  The switcher arrives with FR-HH-03.

### 5.2 Onboarding
- Full window on `background`; one centered `card` (max-w-md, border, no shadow, p-6).
- Brand mark (`/favicon.png`, 40px) above the page title "Te damos la bienvenida a Mr Krabs"
  (`text-xl font-semibold`) and a one-line `muted-foreground` description.
- Fields stacked, 16px apart; one primary full-width button "Comenzar".

## 6. Motion

- CSS transitions only (no animation library).
- Durations: micro feedback 120ms · enter 200ms · exit 140ms. Easing `ease-out` on enter,
  `ease-in` on exit.
- Animate `opacity` and `transform` only. Never block input.
- `prefers-reduced-motion: reduce` → transitions off (global rule in `index.css`).

---

## 7. Components

**shadcn/ui primitives** (add with the shadcn CLI as features need them): Button, Badge,
Card, Table, Dialog, Sheet, Command, ToggleGroup, Select, Input, Tooltip, Sonner (toast).

**Domain components** (built with the features that need them, in `src/components/`):

| Component | Contract |
|---|---|
| `Amount` | Renders `MoneyDto.display` with sign rules from §2, `tabular-nums`, muted currency code. No arithmetic. |
| `ScopeBadge` | Personal/Hogar icon + label + scope color. |
| `StatusBadge` | Pendiente/Liquidado/Conciliado/Propuesto per §2. |
| `BudgetBar` | Progress bar + "% usado" + "$X restante/excedido" text; over budget → `destructive` + icon. |
| `VarianceCell` | Signed Δ with ▲/▼ icon; for the plan-vs-real matrix (FR-RP-07). |
| `ProposedRow` | AI-proposed import row: Sparkles + "Propuesto", tinted until approved. |

Lists over 50 rows are virtualized (NFR-05).

---

### 7.1 Form fields
- Label above the control (`text-sm font-medium`); control `h-9 rounded-md border
  border-input bg-transparent px-3 text-sm`, focus ring per §5.
- Short fixed lists (currency, account kind) use a native `<select>` with the same styling:
  the macOS popup menu is the native feel (§1.4). shadcn `Select` is for searchable or long lists.
- `MoneyInput`: text input, `inputMode="decimal"`, right-aligned `tabular-nums`, currency code
  as a `muted-foreground` suffix. Text goes to Rust as typed.
- Field error: `text-xs text-destructive` line under the control with the `CircleAlert` icon;
  control gets `aria-invalid` and `aria-describedby` to it. Server `ApiError.fields` map to
  the same place.
- Form-level error (no field): `role="alert"` block above the footer, `CircleAlert` +
  `text-destructive` text from the code → copy map (§9, never the raw code).

### 7.2 States and feedback
- **Loading:** one `muted-foreground` "Cargando…" line with `aria-busy` on the region; no spinners in rows or tables.
- **Empty:** a block with the Lucide `Inbox` icon in `muted-foreground` and inviting text pointing at the view's primary action (§9). No second primary button.
- **Not yet available** (`NOT_IMPLEMENTED`, design Q12): never styled as an error, because nothing is wrong. `Hourglass` icon + "Aún no disponible" + "Esta función llega en una próxima versión.", all `muted-foreground`.
  - In a region (sidebar, page) it replaces the content, and the view's primary button is disabled.
  - In a dialog it is the form-level message, and the form stays open with its values.
- **Errors from an action outside a form** (delete, restore, loading more rows): Sonner `toast.error` with the same per-code text as §7.1's form-level error.
- **Undo toast:** Sonner default toast. The text says what happened ("Transacción eliminada"), with one `Deshacer` action and an 8 s duration.

## 8. Charts

- Budget vs. actual → bars with target marker. Cash flow → bars above and below zero
  (in/out). Net worth → line.
- Every chart ships with: a table view of the same series, an `aria-label` summary,
  visible legend, tooltips reachable by keyboard.
- Series values arrive pre-computed from Rust; the UI renders only.
- The categorical chart palette is defined (and contrast-checked) when the first chart
  lands (M2 dashboard); it must not reuse `destructive`/`success` for non-semantic series.

---

## 9. Copy

- Spanish (es-CL) first, sentence case, verb-first buttons ("Nuevo", "Importar cartola").
- Errors say what happened and what to do; no raw error codes (UI maps `ApiError.code`).
- Empty states invite an action ("Importa tu primera cartola").

---

## 10. Anti-patterns

- Red for ordinary expenses; coral for anything that isn't the brand mark.
- Color-only status; emoji as icons; icon-only buttons without `aria-label`.
- Remote fonts/CDNs; raw hex in components; money math in React.
- Pure white app background; gradients; decorative shadows on in-flow cards.
- More than one primary button per view.

## Pre-delivery checklist

- [ ] Only semantic tokens used; both appearances checked.
- [ ] Amounts `tabular-nums`, right-aligned, sign rules from §2.
- [ ] Every status/scope/variance has icon or text besides color.
- [ ] Full keyboard path; visible focus; Esc closes overlays.
- [ ] Reduced motion respected.
- [ ] Works at 1024×700.
