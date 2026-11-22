import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { CircleAlert, Hourglass, Inbox, Plus } from "lucide-react";
import { toast } from "sonner";
import {
  api,
  ApiError,
  type NewTxInput,
  type TxDetailDto,
  type TxRowDto,
} from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";

import { StateBlock } from "./StateBlock";
import { TransactionsTable } from "./TransactionsTable";
import { errorCopyKey } from "./txErrors";
import { TxDialog } from "./TxDialog";

export interface CategoryOption {
  id: string;
  name: string;
}

/** Category choices per transaction type, until `category_tree` lands (DEC-13). */
export interface CategoryOptions {
  expense: CategoryOption[];
  income: CategoryOption[];
}

export interface TransactionsPageProps {
  accountId?: string;
  onAccountIdChange: (accountId: string | undefined) => void;
  categories: CategoryOptions;
}

const PAGE_SIZE = 100; // design Q8
const UNDO_MS = 8000; // MASTER §7.2 undo toast

type DialogState = { mode: "create" } | { mode: "edit"; detail: TxDetailDto } | null;

export function TransactionsPage({
  accountId,
  onAccountIdChange,
  categories,
}: TransactionsPageProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [dialog, setDialog] = useState<DialogState>(null);
  const accounts = useQuery({
    queryKey: ["account", "list", "combined"],
    queryFn: () => api.account.list({ view: "combined" }),
  });
  const accountOptions = accounts.data?.map((s) => s.account) ?? [];
  const list = useInfiniteQuery({
    queryKey: ["tx", "list", { accountId }],
    queryFn: ({ pageParam }) =>
      api.tx.list({ view: "combined", accountId, limit: PAGE_SIZE, cursor: pageParam }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.nextCursor,
  });
  const rows = list.data?.pages.flatMap((p) => p.items) ?? [];
  // Q12: the core answers NOT_IMPLEMENTED until the transactions slice lands.
  const notAvailable = list.error && ApiError.from(list.error).code === "NOT_IMPLEMENTED";

  // Balances and lists change with every write (sidebar balances live under "account").
  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ["tx"] }),
      queryClient.invalidateQueries({ queryKey: ["account"] }),
    ]);

  // Failures of actions outside a form (MASTER §7.2): an error toast with the code's copy.
  const toastError = (raw: unknown) => toast.error(t(errorCopyKey(ApiError.from(raw).code)));

  const open = async (row: TxRowDto) => {
    try {
      const detail = await queryClient.fetchQuery({
        queryKey: ["tx", "detail", row.id],
        queryFn: () => api.tx.get(row.id),
      });
      setDialog({ mode: "edit", detail });
    } catch (raw) {
      toastError(raw);
    }
  };

  // A failed next page is reported once; paging stops until the list is refetched.
  const loadMore = async () => {
    const result = await list.fetchNextPage();
    if (result.isError) toastError(result.error);
  };

  const save = async (input: NewTxInput) => {
    if (dialog?.mode === "edit") await api.tx.update(dialog.detail.id, input);
    else await api.tx.create(input);
    setDialog(null);
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

  const remove = async (id: string) => {
    try {
      await api.tx.delete(id);
    } catch (raw) {
      toastError(raw); // the dialog stays open, nothing changed
      return;
    }
    setDialog(null);
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
          {accountOptions.map((a) => (
            <option key={a.id} value={a.id}>
              {a.name}
            </option>
          ))}
        </select>
        <Button onClick={() => setDialog({ mode: "create" })} disabled={!!notAvailable}>
          <Plus aria-hidden />
          {t("tx.new")}
        </Button>
      </header>
      {notAvailable ? (
        <StateBlock icon={Hourglass} title={t("notAvailable.title")} body={t("notAvailable.body")} />
      ) : list.isPending ? (
        <p aria-busy className="text-muted-foreground text-sm">
          {t("loading")}
        </p>
      ) : list.isError && !list.data ? (
        <div className="flex flex-col items-start gap-3">
          <p role="alert" className="text-destructive flex items-center gap-2 text-sm">
            <CircleAlert aria-hidden className="size-4" />
            {t(errorCopyKey(ApiError.from(list.error).code))}
          </p>
          <Button variant="outline" onClick={() => void list.refetch()}>
            {t("tx.retry")}
          </Button>
        </div>
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
          onOpenChange={(open) => !open && setDialog(null)}
          mode={dialog.mode}
          initial={dialog.mode === "edit" ? dialog.detail : undefined}
          accounts={accountOptions}
          categories={categories}
          onSubmit={save}
          onDelete={dialog.mode === "edit" ? () => remove(dialog.detail.id) : undefined}
        />
      )}
    </section>
  );
}
