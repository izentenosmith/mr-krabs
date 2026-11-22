import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import { api, type NewTxInput, type TxDetailDto, type TxRowDto } from "@mr-krabs/api-client";

import { Button } from "@/components/ui/button";

import { TransactionsTable } from "./TransactionsTable";
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

type DialogState = { mode: "create" } | { mode: "edit"; detail: TxDetailDto } | null;

export function TransactionsPage({ accountId, categories }: TransactionsPageProps) {
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

  // Balances and lists change with every write (sidebar balances live under "account").
  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ["tx"] }),
      queryClient.invalidateQueries({ queryKey: ["account"] }),
    ]);

  const open = async (row: TxRowDto) => {
    const detail = await queryClient.fetchQuery({
      queryKey: ["tx", "detail", row.id],
      queryFn: () => api.tx.get(row.id),
    });
    setDialog({ mode: "edit", detail });
  };

  const save = async (input: NewTxInput) => {
    if (dialog?.mode === "edit") await api.tx.update(dialog.detail.id, input);
    else await api.tx.create(input);
    setDialog(null);
    void refresh();
  };

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <header className="flex h-11 items-center justify-between gap-3">
        <h2 className="text-xl font-semibold">{t("tx.title")}</h2>
        <Button onClick={() => setDialog({ mode: "create" })}>
          <Plus aria-hidden />
          {t("tx.new")}
        </Button>
      </header>
      <TransactionsTable
        rows={rows}
        hasMore={list.hasNextPage}
        loadingMore={list.isFetchingNextPage}
        onLoadMore={() => void list.fetchNextPage()}
        onOpen={(row) => void open(row)}
      />
      {dialog && (
        <TxDialog
          open
          onOpenChange={(open) => !open && setDialog(null)}
          mode={dialog.mode}
          initial={dialog.mode === "edit" ? dialog.detail : undefined}
          accounts={accountOptions}
          categories={categories}
          onSubmit={save}
        />
      )}
    </section>
  );
}
