import { ApiError } from "./errors";

/**
 * The ONLY place in the frontend allowed to talk to a backend.
 * - Inside the Tauri app: IPC via `@tauri-apps/api` `invoke`.
 * - In the browser (docker compose dev): HTTP to the devserver, `POST /api/invoke/<command>`.
 */
export type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export type TransportKind = "tauri" | "http";

export function detectTransport(g: object = globalThis): TransportKind {
  return "__TAURI_INTERNALS__" in g ? "tauri" : "http";
}

export function createHttpInvoke(baseUrl = "", fetchImpl: typeof fetch = fetch): Invoke {
  return async <T>(command: string, args: Record<string, unknown> = {}): Promise<T> => {
    let res: Response;
    try {
      res = await fetchImpl(`${baseUrl}/api/invoke/${encodeURIComponent(command)}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(args),
      });
    } catch (e) {
      throw ApiError.from(e);
    }
    const text = await res.text();
    let body: unknown = null;
    if (text) {
      try {
        body = JSON.parse(text);
      } catch {
        body = text;
      }
    }
    if (!res.ok) throw ApiError.from(body ?? `HTTP ${res.status}`);
    return body as T;
  };
}

export const tauriInvoke: Invoke = async <T>(command: string, args?: Record<string, unknown>) => {
  const { invoke } = await import("@tauri-apps/api/core");
  try {
    return await invoke<T>(command, args);
  } catch (e) {
    throw ApiError.from(e);
  }
};
