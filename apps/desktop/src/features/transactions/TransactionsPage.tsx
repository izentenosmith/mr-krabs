import { useInfiniteQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { api } from "@mr-krabs/api-client";

import { TransactionsTable } from "./TransactionsTable";

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

export function TransactionsPage({ accountId }: TransactionsPageProps) {
  const { t } = useTranslation();
  const list = useInfiniteQuery({
    queryKey: ["tx", "list", { accountId }],
    queryFn: ({ pageParam }) =>
      api.tx.list({ view: "combined", accountId, limit: PAGE_SIZE, cursor: pageParam }),
    initialPageParam: null as string | null,
    getNextPageParam: (last) => last.nextCursor,
  });
  const rows = list.data?.pages.flatMap((p) => p.items) ?? [];

  return (
    <section className="flex min-h-0 flex-1 flex-col gap-4">
      <h2 className="text-xl font-semibold">{t("tx.title")}</h2>
      <TransactionsTable
        rows={rows}
        hasMore={list.hasNextPage}
        loadingMore={list.isFetchingNextPage}
        onLoadMore={() => void list.fetchNextPage()}
        onOpen={() => {}}
      />
    </section>
  );
}
