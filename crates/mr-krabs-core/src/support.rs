//! What every command shares: input checks that become `ApiError.fields` (X-8), the
//! `DomainError` → `ApiError` mapping (design Q6), the per-command span (Q10), wire enums as
//! their stored codes, and the conversion between stored postings and domain legs.

use std::time::Instant;

use mrk_domain::{DomainError, Leg, LegStatus, Posting, StoredLeg};
use mrk_money::{Currency, CurrencyCode, Money, MoneyError, parse_es_cl};
use mrk_store::{PostingRow, StoreError};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{ApiError, ErrorCode};

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        ApiError::internal(e)
    }
}

/// Runs one command inside its `tracing` span and logs its duration and result code at
/// `info` — never its input (no amounts, names or descriptions).
pub(crate) fn command<T>(
    name: &'static str,
    run: impl FnOnce() -> Result<T, ApiError>,
) -> Result<T, ApiError> {
    let span = tracing::info_span!("command", command = name);
    let _entered = span.enter();
    let started = Instant::now();
    let out = run();
    let result = match &out {
        Ok(_) => "OK".to_owned(),
        Err(e) => wire(e.code),
    };
    tracing::info!(
        elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        result,
        "command finished"
    );
    out
}

/// Maps a ledger rule's refusal to its wire code (design Q6). Logged at debug only (X-6).
pub(crate) fn domain_error(e: DomainError) -> ApiError {
    tracing::debug!(error = %e, "ledger rule refused the change");
    let code = match e {
        DomainError::UnknownAccount(_) => ErrorCode::NotFound,
        DomainError::CurrencyMismatch { .. } => ErrorCode::CurrencyMismatch,
        DomainError::Unbalanced { .. } => ErrorCode::UnbalancedEntry,
        DomainError::ReconciledEntryLocked { .. } => ErrorCode::Conflict,
        _ => ErrorCode::Validation,
    };
    ApiError::new(code, e.to_string())
}

/// Field errors collected while checking one input; `finish` fails when there are any.
#[derive(Default)]
pub(crate) struct Fields(Option<ApiError>);

impl Fields {
    pub(crate) fn add(&mut self, field: &str, reason: &str) {
        let e = self
            .0
            .take()
            .unwrap_or_else(|| ApiError::new(ErrorCode::Validation, "invalid input"));
        self.0 = Some(e.with_field(field, format!("errors.field.{reason}")));
    }

    /// A required name: trimmed, non-empty, at most `max` characters.
    pub(crate) fn name(&mut self, field: &str, value: &str, max: usize) -> String {
        let v = value.trim();
        if v.is_empty() {
            self.add(field, "required");
        } else if v.chars().count() > max {
            self.add(field, "tooLong");
        }
        v.to_owned()
    }

    /// Optional text: trimmed, blank becomes `None`, at most `max` characters.
    pub(crate) fn optional(
        &mut self,
        field: &str,
        value: Option<&str>,
        max: usize,
    ) -> Option<String> {
        let v = value.map(str::trim).filter(|v| !v.is_empty())?;
        if v.chars().count() > max {
            self.add(field, "tooLong");
        }
        Some(v.to_owned())
    }

    /// A built-in currency.
    pub(crate) fn currency(&mut self, field: &str, code: &str) -> Option<Currency> {
        let c = Currency::built_in(code);
        if c.is_none() {
            self.add(field, "unsupportedCurrency");
        }
        c
    }

    pub(crate) fn finish(self) -> Result<(), ApiError> {
        self.0.map_or(Ok(()), Err)
    }
}

/// Parses user-typed money in `currency`; any failure (text, decimals, overflow) is
/// `errors.field.invalidAmount` on `field`.
pub(crate) fn parse_amount(field: &str, text: &str, currency: Currency) -> Result<Money, ApiError> {
    parse_es_cl(text, currency)
        .map(|minor| Money::new(minor, currency.code))
        .map_err(|e| field_error(ErrorCode::Validation, field, "invalidAmount", e))
}

pub(crate) fn field_error(
    code: ErrorCode,
    field: &str,
    reason: &str,
    message: impl std::fmt::Display,
) -> ApiError {
    ApiError::new(code, message.to_string()).with_field(field, format!("errors.field.{reason}"))
}

/// Ids are UUIDs; anything else is refused before it reaches SQL.
pub(crate) fn check_id(id: &str) -> Result<(), ApiError> {
    uuid::Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| ApiError::validation("id is not a UUID"))
}

pub(crate) fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// A wire enum as the code SQLite stores (`shared_full`, `es-CL`, …).
pub(crate) fn wire<T: Serialize>(v: T) -> String {
    match serde_json::to_value(v) {
        Ok(serde_json::Value::String(s)) => s,
        other => unreachable!("wire enums serialize to strings, got {other:?}"),
    }
}

/// A stored code read back as its wire enum.
pub(crate) fn from_wire<T: DeserializeOwned>(code: &str) -> Result<T, ApiError> {
    serde_json::from_value(serde_json::Value::String(code.to_owned()))
        .map_err(|_| ApiError::internal(format!("unknown stored code {code:?}")))
}

pub(crate) fn currency_code(code: &str) -> Result<CurrencyCode, ApiError> {
    CurrencyCode::new(code).map_err(|e: MoneyError| ApiError::internal(e))
}

pub(crate) fn sort_order(v: i64) -> Result<i32, ApiError> {
    i32::try_from(v).map_err(ApiError::internal)
}

// ───────────────────────── postings ⇄ domain legs ─────────────────────────

/// A stored posting as the domain sees it.
pub(crate) fn stored_leg(p: &PostingRow) -> Result<StoredLeg, ApiError> {
    let leg = match (p.leg.as_str(), &p.account_id, &p.category_id) {
        ("account", Some(a), None) => Leg::Account(a.clone()),
        ("category", None, Some(c)) => Leg::Category(c.clone()),
        ("fx", None, None) => Leg::FxTrading,
        _ => return Err(ApiError::internal(format!("malformed posting {}", p.id))),
    };
    let status = match p.status.as_deref() {
        None => None,
        Some("pending") => Some(LegStatus::Pending),
        Some("cleared") => Some(LegStatus::Cleared),
        Some("reconciled") => Some(LegStatus::Reconciled),
        Some(other) => return Err(ApiError::internal(format!("unknown status {other:?}"))),
    };
    Ok(StoredLeg {
        id: p.id.clone(),
        posting: Posting {
            leg,
            amount: Money::new(p.amount_minor, currency_code(&p.currency_code)?),
        },
        status,
    })
}

/// A domain leg as the posting row to store; account legs get `status`.
pub(crate) fn posting_row(
    id: String,
    entry_id: &str,
    posting: &Posting,
    status: LegStatus,
) -> PostingRow {
    let (leg, account_id, category_id) = match &posting.leg {
        Leg::Account(a) => ("account", Some(a.clone()), None),
        Leg::Category(c) => ("category", None, Some(c.clone())),
        Leg::FxTrading => ("fx", None, None),
    };
    let status = (leg == "account").then(|| {
        match status {
            LegStatus::Pending => "pending",
            LegStatus::Cleared => "cleared",
            LegStatus::Reconciled => "reconciled",
        }
        .to_owned()
    });
    PostingRow {
        id,
        entry_id: entry_id.to_owned(),
        leg: leg.to_owned(),
        account_id,
        category_id,
        amount_minor: posting.amount.minor,
        currency_code: posting.amount.currency.to_string(),
        status,
        deleted: false,
    }
}
