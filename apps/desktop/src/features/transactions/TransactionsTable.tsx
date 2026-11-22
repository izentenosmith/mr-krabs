import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { createColumnHelper, tableFeatures, useTable } from "@tanstack/react-table";
import { useWindowVirtualizer } from "@tanstack/react-virtual";
import { ArrowLeftRight, Check, CircleCheck, Clock } from "lucide-react";
import type { LegStatus, TxKind, TxRowDto } from "@mr-krabs/api-client";

import { cn } from "@/lib/utils";

export interface TransactionsTableProps {
  rows: TxRowDto[];
  hasMore: boolean;
  loadingMore: boolean;
  onLoadMore: () => void;
  onOpen: (row: TxRowDto) => void;
}

const ROW_HEIGHT = 36; // MASTER §4: table rows 36px.

const features = tableFeatures({});
const column = createColumnHelper<typeof features, TxRowDto>();

type ColumnKey = "date" | "description" | "category" | "account" | "status" | "amount";

function Header({ k }: { k: ColumnKey }) {
  const { t } = useTranslation();
  return <>{t(`tx.col.${k}`)}</>;
}

/** Rows a user can open in the dialog: opening balances belong to the account dialog. */
export function isEditable(row: TxRowDto) {
  return row.kind !== "opening_balance";
}

function CategoryCell({ row }: { row: TxRowDto }) {
  const { t } = useTranslation();
  if (row.kind === "opening_balance") return <>{t("tx.openingBalance")}</>;
  if (row.kind === "transfer") {
    return (
      <span className="text-muted-foreground inline-flex items-center gap-1">
        <ArrowLeftRight aria-hidden className="size-3.5" />
        <span className="sr-only">{t("tx.transferTo")}</span>
        {row.counterAccountName}
      </span>
    );
  }
  return <>{row.categoryName}</>;
}

const STATUS_ICON: Record<LegStatus, typeof Clock> = {
  pending: Clock,
  cleared: Check,
  reconciled: CircleCheck,
};

function StatusCell({ status }: { status: LegStatus }) {
  const { t } = useTranslation();
  const Icon = STATUS_ICON[status];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 text-xs",
        status === "reconciled" ? "text-success" : "text-muted-foreground",
      )}
    >
      <Icon aria-hidden className="size-3.5" />
      {t(`tx.status.${status}`)}
    </span>
  );
}

const AMOUNT_TONE: Record<TxKind, string> = {
  expense: "text-foreground",
  income: "text-success",
  transfer: "text-muted-foreground",
  opening_balance: "text-foreground",
};

/** Renders the core's `display` string verbatim (MASTER §2/§7, X-7): no arithmetic here. */
function AmountCell({ row }: { row: TxRowDto }) {
  return <span className={cn("tabular-nums", AMOUNT_TONE[row.kind])}>{row.amount.display}</span>;
}

const columns = column.columns([
  column.accessor("date", { header: () => <Header k="date" /> }),
  column.accessor("description", { header: () => <Header k="description" /> }),
  column.display({
    id: "category",
    header: () => <Header k="category" />,
    cell: ({ row }) => <CategoryCell row={row.original} />,
  }),
  column.accessor("accountName", { id: "account", header: () => <Header k="account" /> }),
  column.display({
    id: "status",
    header: () => <Header k="status" />,
    cell: ({ row }) => <StatusCell status={row.original.status} />,
  }),
  column.display({
    id: "amount",
    header: () => <Header k="amount" />,
    cell: ({ row }) => <AmountCell row={row.original} />,
  }),
]);

// Fixed column template so header and virtualized rows line up (rows are CSS grid rows).
const GRID = "grid grid-cols-[7rem_minmax(0,2fr)_minmax(0,1.5fr)_minmax(0,1.2fr)_7rem_8rem]";

/** Virtualized transactions table (Q8, NFR-05). Presentational: rows in, events out. */
export function TransactionsTable({
  rows,
  hasMore,
  loadingMore,
  onLoadMore,
  onOpen,
}: TransactionsTableProps) {
  const table = useTable({ features, columns, data: rows, getRowId: (r) => r.id });
  const modelRows = table.getRowModel().rows;
  // The app shell scrolls the window (MASTER §5 layout, `min-h-screen`), so rows are
  // virtualized against the window; the body's offset from the page top is the margin (F7).
  const bodyRef = useRef<HTMLTableSectionElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  useLayoutEffect(() => {
    const top = bodyRef.current?.getBoundingClientRect().top ?? 0;
    setScrollMargin(top + window.scrollY);
  }, []);
  const virtualizer = useWindowVirtualizer({
    count: modelRows.length,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
    scrollMargin,
  });
  const items = virtualizer.getVirtualItems();
  const lastIndex = items.at(-1)?.index ?? -1;

  // Keyset paging: ask for the next page once the last loaded row is in view.
  useEffect(() => {
    if (hasMore && !loadingMore && lastIndex >= modelRows.length - 1) onLoadMore();
  }, [hasMore, loadingMore, lastIndex, modelRows.length, onLoadMore]);

  return (
    <div className="bg-card rounded-lg border">
      <table role="table" className="w-full text-[13px]" aria-rowcount={modelRows.length + 1}>
        <thead role="rowgroup" className="bg-card sticky top-0 z-10 border-b">
          {table.getHeaderGroups().map((group) => (
            <tr key={group.id} role="row" className={cn(GRID, "h-9 items-center px-3")}>
              {group.headers.map((header) => (
                <th
                  key={header.id}
                  role="columnheader"
                  className={cn(
                    "text-muted-foreground truncate px-2 text-left text-xs font-medium",
                    header.column.id === "amount" && "text-right",
                  )}
                >
                  <table.FlexRender header={header} />
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody
          ref={bodyRef}
          role="rowgroup"
          className="relative block"
          style={{ height: virtualizer.getTotalSize() }}
        >
          {items.map((item) => {
            const row = modelRows[item.index];
            if (!row) return null;
            const editable = isEditable(row.original);
            return (
              <tr
                key={row.id}
                role="row"
                aria-rowindex={item.index + 2}
                tabIndex={editable ? 0 : undefined}
                onClick={editable ? () => onOpen(row.original) : undefined}
                onKeyDown={
                  editable
                    ? (e) => {
                        if (e.key === "Enter") onOpen(row.original);
                      }
                    : undefined
                }
                className={cn(
                  GRID,
                  "absolute inset-x-0 h-9 items-center border-b px-3",
                  editable &&
                    "hover:bg-accent focus-visible:ring-ring/50 cursor-pointer outline-none focus-visible:ring-[3px]",
                )}
                style={{ transform: `translateY(${item.start - scrollMargin}px)` }}
              >
                {row.getAllCells().map((cell) => (
                  <td
                    key={cell.id}
                    role="cell"
                    className={cn("truncate px-2", cell.column.id === "amount" && "text-right")}
                  >
                    <table.FlexRender cell={cell} />
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
