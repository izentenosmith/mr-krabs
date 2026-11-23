import { createHttpInvoke, detectTransport, tauriInvoke, type Invoke } from "./transport";
import type {
  AccountDetailDto,
  AccountDto,
  AccountInput,
  AccountListQuery,
  AccountSummaryDto,
  BootstrapDto,
  BreakdownDto,
  BudgetCellDto,
  BudgetCopyInput,
  BudgetMatrixDto,
  BudgetMatrixQuery,
  BudgetSetInput,
  CalendarDto,
  CashFlowDto,
  CashFlowQuery,
  CategoryArchiveInput,
  CategoryDeleteInput,
  CategoryDto,
  CategoryInput,
  CategoryMoveInput,
  CategoryNodeDto,
  CategoryTreeQuery,
  CategoryUpdateInput,
  HealthDto,
  HouseholdDto,
  HouseholdInput,
  MoneyDto,
  MoneyInput,
  MonthCalendarQuery,
  NewTxInput,
  Page,
  SettingsDto,
  SpendingQuery,
  TxDetailDto,
  TxQuery,
  TxRowDto,
} from "./types";

export * from "./types";
export { ApiError, type FieldErrors } from "./errors";
export { createHttpInvoke, detectTransport, type Invoke, type TransportKind } from "./transport";

export function createApi(invoke: Invoke) {
  // One method per command in `COMMANDS` (crates/mr-krabs-core/src/commands.rs); argument
  // keys are the Tauri parameter names. A command whose slice has not landed answers
  // NOT_IMPLEMENTED (M2: everything below `category.tree` except `category.delete`).
  return {
    app: {
      health: () => invoke<HealthDto>("app_health"),
      bootstrap: () => invoke<BootstrapDto>("app_bootstrap"),
    },
    household: {
      create: (input: HouseholdInput) => invoke<HouseholdDto>("household_create", { input }),
    },
    settings: {
      get: () => invoke<SettingsDto>("settings_get"),
    },
    money: {
      parse: (input: MoneyInput) => invoke<MoneyDto>("money_parse", { input }),
    },
    account: {
      list: (query: AccountListQuery) => invoke<AccountSummaryDto[]>("account_list", { query }),
      get: (id: string) => invoke<AccountDetailDto>("account_get", { id }),
      create: (input: AccountInput) => invoke<AccountDto>("account_create", { input }),
      update: (id: string, input: AccountInput) =>
        invoke<AccountDto>("account_update", { id, input }),
      archive: (id: string) => invoke<AccountDto>("account_archive", { id }),
    },
    category: {
      tree: (query: CategoryTreeQuery) => invoke<CategoryNodeDto[]>("category_tree", { query }),
      create: (input: CategoryInput) => invoke<CategoryDto>("category_create", { input }),
      update: (input: CategoryUpdateInput) => invoke<CategoryDto>("category_update", { input }),
      move: (input: CategoryMoveInput) => invoke<CategoryDto>("category_move", { input }),
      archive: (input: CategoryArchiveInput) => invoke<CategoryDto>("category_archive", { input }),
      delete: (input: CategoryDeleteInput) => invoke<null>("category_delete", { input }),
    },
    budget: {
      /** `null` when an empty or zero amount deleted the line. */
      set: (input: BudgetSetInput) => invoke<BudgetCellDto | null>("budget_set", { input }),
      /** Resolves to the number of lines written. */
      copyMonth: (input: BudgetCopyInput) => invoke<number>("budget_copy_month", { input }),
      matrix: (query: BudgetMatrixQuery) => invoke<BudgetMatrixDto>("budget_matrix", { query }),
    },
    report: {
      monthCalendar: (query: MonthCalendarQuery) =>
        invoke<CalendarDto>("report_month_calendar", { query }),
      cashFlow: (query: CashFlowQuery) => invoke<CashFlowDto>("report_cash_flow", { query }),
      spending: (query: SpendingQuery) => invoke<BreakdownDto>("report_spending", { query }),
    },
    tx: {
      create: (input: NewTxInput) => invoke<TxDetailDto>("tx_create", { input }),
      get: (id: string) => invoke<TxDetailDto>("tx_get", { id }),
      list: (query: TxQuery) => invoke<Page<TxRowDto>>("tx_list", { query }),
      update: (id: string, input: NewTxInput) => invoke<TxDetailDto>("tx_update", { id, input }),
      delete: (id: string) => invoke<null>("tx_delete", { id }),
      restore: (id: string) => invoke<null>("tx_restore", { id }),
    },
  };
}

export type Api = ReturnType<typeof createApi>;

const baseUrl =
  (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.VITE_API_URL ?? "";

export const transport = detectTransport();

/** Default client: Tauri IPC in the desktop app, HTTP devserver in the browser. */
export const api: Api = createApi(transport === "tauri" ? tauriInvoke : createHttpInvoke(baseUrl));
