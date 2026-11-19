import { createHttpInvoke, detectTransport, tauriInvoke, type Invoke } from "./transport";
import type { HealthDto, MoneyDto, MoneyInput } from "./types";

export * from "./types";
export { ApiError } from "./errors";
export { createHttpInvoke, detectTransport, type Invoke, type TransportKind } from "./transport";

export function createApi(invoke: Invoke) {
  return {
    app: {
      health: () => invoke<HealthDto>("app_health"),
    },
    money: {
      parse: (input: MoneyInput) => invoke<MoneyDto>("money_parse", { input }),
    },
  };
}

export type Api = ReturnType<typeof createApi>;

const baseUrl =
  (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.VITE_API_URL ?? "";

export const transport = detectTransport();

/** Default client: Tauri IPC in the desktop app, HTTP devserver in the browser. */
export const api: Api = createApi(transport === "tauri" ? tauriInvoke : createHttpInvoke(baseUrl));
