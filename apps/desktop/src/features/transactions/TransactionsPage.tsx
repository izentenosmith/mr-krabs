// Declared by acceptance-tests (S7); tdd builds it. Renders nothing yet.

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

export function TransactionsPage(props: TransactionsPageProps) {
  void props;
  return null;
}
