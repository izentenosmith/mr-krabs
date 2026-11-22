// Wire types. Generated from `crates/mr-krabs-core/src/api.rs` into `bindings.ts`
// (design Q4); this file only adds what exists on the client side.

import type { ErrorCode as CoreErrorCode } from "./bindings";

export type * from "./bindings";

/** Core error codes plus the client-only `TRANSPORT` (IPC/HTTP failure). */
export type ErrorCode = CoreErrorCode | "TRANSPORT";
