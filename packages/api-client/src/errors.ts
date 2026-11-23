import type { ErrorCode } from "./types";

/** Field → i18n key, for forms (`ApiError.fields` on the wire). */
export type FieldErrors = Partial<Record<string, string>>;

export class ApiError extends Error {
  readonly code: ErrorCode;
  readonly fields: FieldErrors | null;

  constructor(code: ErrorCode, message: string, fields: FieldErrors | null = null) {
    super(message);
    this.name = "ApiError";
    this.code = code;
    this.fields = fields;
  }

  /** Normalizes anything thrown by a transport into an ApiError. */
  static from(raw: unknown): ApiError {
    if (raw instanceof ApiError) return raw;
    if (raw && typeof raw === "object" && "code" in raw && "message" in raw) {
      const r = raw as { code: unknown; message: unknown; fields?: unknown };
      const fields =
        r.fields && typeof r.fields === "object" ? (r.fields as FieldErrors) : null;
      return new ApiError(String(r.code) as ErrorCode, String(r.message), fields);
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
