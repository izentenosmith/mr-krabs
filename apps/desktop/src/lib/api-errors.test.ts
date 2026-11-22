import { ApiError } from "@mr-krabs/api-client";

import i18n from "@/i18n";

import { applyServerErrors, errorCopyKey, fieldMessage, shouldRetry } from "./api-errors";

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

describe("applyServerErrors for codes other than VALIDATION (X-13)", () => {
  it("a code with every field mapped also sets its own form-level copy", () => {
    const setError = vi.fn();
    applyServerErrors(
      new ApiError("CURRENCY_MISMATCH", "x", { "openingBalance.amount": "errors.field.currencyMismatch" }),
      setError,
      { "openingBalance.amount": "openingAmount" },
    );
    expect(setError.mock.calls).toEqual([
      ["openingAmount", { type: "server", message: "errors.field.currencyMismatch" }],
      ["root.server", { type: "CURRENCY_MISMATCH" }],
    ]);
  });

  it("VALIDATION with every field mapped stays field-level only", () => {
    const setError = vi.fn();
    applyServerErrors(new ApiError("VALIDATION", "x", { name: "errors.field.required" }), setError, {
      name: "name",
    });
    expect(setError).toHaveBeenCalledTimes(1);
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

describe("fieldMessage", () => {
  it("renders only errors.field.* keys; any other key gets the generic copy (S7 F17)", () => {
    const t = i18n.t.bind(i18n);
    expect(fieldMessage(t, "errors.field.invalidAmount")).toBe("Monto no válido");
    expect(fieldMessage(t, "tx.title")).toBe("Revisa este campo");
    expect(fieldMessage(t, "errors")).toBe("Revisa este campo");
    expect(fieldMessage(t, undefined)).toBeUndefined();
  });
});

describe("errors.field copy (X-13)", () => {
  // Every `errors.field.<reason>` the core sends: #29's table (with `invalidDate`) and S5's tx_* reasons.
  const reasons = [
    "required",
    "tooLong",
    "unsupportedCurrency",
    "invalidAmount",
    "invalidDate",
    "currencyMismatch",
    "notAllowed",
    "fullNumber",
    "lockedByPostings",
    // The transactions commands (S5).
    "sameAccount",
    "wrongKind",
    "archived",
    "notFound",
    "outOfRange",
    "invalid",
  ];

  it.each(["es-CL", "en"])("has its own copy for every reason in %s", (lng) => {
    const t = i18n.getFixedT(lng);
    const generic = t("errors.field.unknown");
    for (const reason of reasons) {
      const key = `errors.field.${reason}`;
      expect(i18n.getResource(lng, "translation", key), key).toEqual(expect.any(String));
      expect(fieldMessage(t, key), key).not.toBe(generic);
    }
  });
});
