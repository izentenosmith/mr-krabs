import { createHttpInvoke, detectTransport, tauriInvoke, type Invoke } from "./transport";
import type {
  AccountDetailDto,
  AccountDto,
  AccountInput,
  AccountListQuery,
  AccountSummaryDto,
  BootstrapDto,
  HealthDto,
  HouseholdDto,
  HouseholdInput,
  MoneyDto,
  MoneyInput,
  NewTxInput,
  Page,
  SettingsDto,
  TxDetailDto,
  TxQuery,
  TxRowDto,
} from "./types";

export * from "./types";
export { ApiError, type FieldErrors } from "./errors";
export { createHttpInvoke, detectTransport, type Invoke, type TransportKind } from "./transport";

export function createApi(invoke: Invoke) {
  // One method per command in `COMMANDS` (crates/mr-krabs-core/src/commands.rs); argument
  // keys are the Tauri parameter names. M1 commands answer NOT_IMPLEMENTED until built.
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
