import { ApiError } from "@mr-krabs/api-client";

import { applyServerErrors, errorCopyKey, shouldRetry } from "./api-errors";

describe("applyServerErrors", () => {
  it("puts mapped fields on their form fields and nothing at form level", () => {
    const setError = vi.fn();
    applyServerErrors(
      new ApiError("VALIDATION", "x", { "openingBalance.amount": "errors.field.invalidAmount" }),
      setError,
      { "openingBalance.amount": "openingAmount" },
    );
    expect(setError).toHaveBeenCalledTimes(1);
    expect(setError).toHaveBeenCalledWith("openingAmount", {
      type: "server",
      message: "errors.field.invalidAmount",
    });
  });

  it("a field the form doesn't have also raises the form-level error", () => {
    const setError = vi.fn();
    applyServerErrors(new ApiError("VALIDATION", "x", { scope: "errors.field.required" }), setError, {});
    expect(setError).toHaveBeenCalledWith("root.server", { type: "VALIDATION" });
  });

  it("an error without fields, or a non-ApiError, is form-level with its code", () => {
    const setError = vi.fn();
    applyServerErrors(new ApiError("CONFLICT", "x"), setError, {});
    applyServerErrors(new Error("network down"), setError, {});
    expect(setError.mock.calls).toEqual([
      ["root.server", { type: "CONFLICT" }],
      ["root.server", { type: "TRANSPORT" }],
    ]);
  });
});

describe("errorCopyKey", () => {
  it("maps a known code to its copy and an unknown one to the generic copy", () => {
    expect(errorCopyKey("CURRENCY_MISMATCH")).toBe("errors.code.CURRENCY_MISMATCH");
    expect(errorCopyKey("SOMETHING_NEW")).toBe("errors.code.INTERNAL");
  });
});

describe("shouldRetry", () => {
  it("retries only transport failures, at most 3 times; core answers are final", () => {
    expect(shouldRetry(0, new ApiError("TRANSPORT", "down"))).toBe(true);
    expect(shouldRetry(2, new Error("fetch failed"))).toBe(true);
    expect(shouldRetry(3, new ApiError("TRANSPORT", "down"))).toBe(false);
    for (const code of ["NOT_IMPLEMENTED", "VALIDATION", "NOT_FOUND", "INTERNAL"] as const) {
      expect(shouldRetry(0, new ApiError(code, "x"))).toBe(false);
    }
  });
});
