import { useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Inbox, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  api,
  ApiError,
  type NewTxInput,
  type TxDetailDto,
  type TxRowDto,
  type View,
} from "@mr-krabs/api-client";

import { ErrorAlert } from "@/components/ErrorAlert";
import { NotAvailable } from "@/components/NotAvailable";
import { Button } from "@/components/ui/button";
import { accountsKey } from "@/features/accounts/AccountsSidebarContainer";
import { errorCopyKey, isNotImplemented } from "@/lib/api-errors";

import { StateBlock } from "./StateBlock";
import { categoryOptions } from "./categoryOptions";
import { TransactionsTable } from "./TransactionsTable";
import { AccountOptions, TxDialog, type CategoryOptions } from "./TxDialog";

export interface TransactionsPageProps {
  accountId?: string;
  onAccountIdChange: (accountId: string | undefined) => void;
}

const PAGE_SIZE = 100; // design Q8
const NO_ROWS: TxRowDto[] = [];
// M1 has one member, so the combined view is everything (Q1); the lens switcher (M3) sets it.
const VIEW: View = "combined";
const UNDO_MS = 8000; // MASTER §7.2 undo toast

/** A failed load: the code's copy (MASTER §7.1) and a retry. */
function LoadError({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-col items-start gap-3">
      <ErrorAlert>
        <p>{t(errorCopyKey(ApiError.from(error).code))}</p>
      </ErrorAlert>
      <Button variant="outline" onClick={onRetry}>
        {t("shell.retry")}
      </Button>
    </div>
  );
}

/** The open dialog; `key` identifies one opening so late answers can't touch a newer one. */
type OpenDialog = ({ mode: "create" } | { mode: "edit"; detail: TxDetailDto }) & { key: number };

export function TransactionsPage({ accountId, onAccountIdChange }: TransactionsPageProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [dialog, setDialog] = useState<OpenDialog | null>(null);
  // Bumped by every request to open a dialog; an answer for an older request is dropped (F1).
  const lastOpen = useRef(0);
  const close = (key: number) => setDialog((d) => (d?.key === key ? null : d));
  const accounts = useQuery({
    // Same key and arguments as the accounts sidebar (S6), so both share one cache entry.
    queryKey: [...accountsKey, "list", { includeArchived: false }],
    queryFn: () => api.account.list({ view: VIEW, includeArchived: false }),
  });
  const accountOptions = accounts.data?.map((s) => s.account) ?? [];
  // The category picker's live trees, one per kind (REQ-13).
  const expenseTree = useQuery({
    queryKey: ["categories", "tree", "expense"],
    queryFn: () => api.category.tree({ kind: "expense" }),
  });
  const incomeTree = useQuery({
    queryKey: ["categories", "tree", "income"],
    queryFn: () => api.category.tree({ kind: "income" }),
  });
  const categories = useMemo<CategoryOptions>(
    () => ({
      expense: categoryOptions(expenseTree.data ?? []),
      income: categoryOptions(incomeTree.data ?? []),
    }),
    [expenseTree.data, incomeTree.data],
  );
  const list = useInfiniteQuery({
    queryKey: ["tx", "list", { accountId }],
    queryFn: ({ pageParam }) =>
      api.tx.list({ view: VIEW, accountId, limit: PAGE_SIZE, cursor: pageParam }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.nextCursor,
  });
  // Stable across renders that don't change the pages, so the table keeps its row model (F31).
  const rows = useMemo(() => list.data?.pages.flatMap((p) => p.items) ?? NO_ROWS, [list.data]);
  // What the toolbar and dialog need besides the list; a failure (not NOT_IMPLEMENTED,
  // which isn't an error) shows one alert instead of silently empty selects.
  const support = [accounts, expenseTree, incomeTree];
  const supportFailure = support.find((q) => q.isError && !isNotImplemented(q.error));
  // Q12: the core answers NOT_IMPLEMENTED until the transactions slice lands.
  const notAvailable = list.isError && isNotImplemented(list.error);

  // Balances and lists change with every write (sidebar balances live under "accounts").
  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ["tx"] }),
      queryClient.invalidateQueries({ queryKey: accountsKey }),
    ]);

  // Failures of actions outside a form (MASTER §7.2): an error toast with the code's copy.
  const toastError = (raw: unknown) => toast.error(t(errorCopyKey(ApiError.from(raw).code)));

  const openCreate = () => setDialog({ mode: "create", key: ++lastOpen.current });

  const open = async (row: TxRowDto) => {
    const key = ++lastOpen.current;
    try {
      const detail = await queryClient.fetchQuery({
        queryKey: ["tx", "detail", row.id],
        queryFn: () => api.tx.get(row.id),
      });
      if (key === lastOpen.current) setDialog({ mode: "edit", detail, key });
    } catch (raw) {
      if (key === lastOpen.current) toastError(raw); // an abandoned opening stays silent (F44)
    }
  };

  // A failed next page is reported once; paging stops until the list is refetched.
  const loadMore = async () => {
    const result = await list.fetchNextPage();
    if (result.isError) toastError(result.error);
  };

  const save = async (target: OpenDialog, input: NewTxInput) => {
    if (target.mode === "edit") await api.tx.update(target.detail.id, input);
    else await api.tx.create(input);
    close(target.key);
    void refresh();
  };

  const restore = async (id: string) => {
    try {
      await api.tx.restore(id);
    } catch (raw) {
      toastError(raw);
      return;
    }
    void refresh();
  };

  const remove = async (key: number, id: string) => {
    try {
      await api.tx.delete(id);
    } catch (raw) {
      toastError(raw); // the dialog stays open, nothing changed
      return;
    }
    close(key);
    void refresh();
    toast(t("tx.deleted"), {
      duration: UNDO_MS,
      action: { label: t("tx.undo"), onClick: () => void restore(id) },
    });
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <header className="flex h-11 items-center gap-3">
        <h2 className="text-xl font-semibold">{t("tx.title")}</h2>
        <label htmlFor="tx-filter" className="sr-only">
          {t("tx.filterLabel")}
        </label>
        <select
          id="tx-filter"
          value={accountId ?? ""}
          onChange={(e) => onAccountIdChange(e.target.value || undefined)}
          className="border-input focus-visible:ring-ring/50 ml-auto h-9 rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:ring-[3px]"
        >
          <option value="">{t("tx.allAccounts")}</option>
          <AccountOptions accounts={accountOptions} />
        </select>
        <Button onClick={openCreate} disabled={notAvailable}>
          <Plus aria-hidden />
          {t("tx.new")}
        </Button>
      </header>
      {supportFailure && !notAvailable && (
        // One alert for every failed load; its retry retries them all (F43).
        <LoadError
          error={supportFailure.error}
          onRetry={() => {
            for (const q of support) if (q.isError) void q.refetch();
            if (list.isError) void list.refetch();
          }}
        />
      )}
      {notAvailable ? (
        <NotAvailable className="justify-center rounded-lg border p-8" />
      ) : list.isPending ? (
        <p aria-busy className="text-muted-foreground text-sm">
          {t("shell.loading")}
        </p>
      ) : list.isError && !list.data ? (
        supportFailure ? null : (
          <LoadError error={list.error} onRetry={() => void list.refetch()} />
        )
      ) : rows.length === 0 ? (
        <StateBlock icon={Inbox} title={t("tx.emptyTitle")} body={t("tx.emptyBody")} />
      ) : (
        <TransactionsTable
          rows={rows}
          hasMore={list.hasNextPage && !list.isFetchNextPageError}
          loadingMore={list.isFetchingNextPage}
          onLoadMore={() => void loadMore()}
          onOpen={(row) => void open(row)}
        />
      )}
      {dialog && (
        <TxDialog
          open
          key={dialog.key}
          onOpenChange={(isOpen) => !isOpen && close(dialog.key)}
          mode={dialog.mode}
          initial={dialog.mode === "edit" ? dialog.detail : undefined}
          accounts={accountOptions}
          categories={categories}
          onSubmit={(input) => save(dialog, input)}
          onDelete={dialog.mode === "edit" ? () => remove(dialog.key, dialog.detail.id) : undefined}
        />
      )}
    </section>
  );
}
