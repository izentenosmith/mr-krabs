import type { ErrorCode } from "./types";

export class ApiError extends Error {
  readonly code: ErrorCode;

  constructor(code: ErrorCode, message: string) {
    super(message);
    this.name = "ApiError";
    this.code = code;
  }

  /** Normalizes anything thrown by a transport into an ApiError. */
  static from(raw: unknown): ApiError {
    if (raw instanceof ApiError) return raw;
    if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
      const r = raw as { code: unknown; message: unknown };
      return new ApiError(String(r.code) as ErrorCode, String(r.message));
    }
    if (typeof raw === "string") {
      try {
        return ApiError.from(JSON.parse(raw));
      } catch {
        return new ApiError("INTERNAL", raw);
      }
    }
    return new ApiError("TRANSPORT", raw instanceof Error ? raw.message : String(raw));
  }
}
