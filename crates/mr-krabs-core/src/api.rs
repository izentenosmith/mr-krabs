//! Serializable DTOs and errors that cross the shell boundary.
//! Field names are camelCase on the wire to match TypeScript conventions.
//!
//! Every type here derives `specta::Type`; `packages/api-client/src/bindings.ts` is generated
//! from them by the `bindings_are_fresh` test in `commands.rs` (docs/04, design Q4).
//! Ids are UUID strings and dates are civil `YYYY-MM-DD` strings; both are validated in core.
//! Optional *input* fields are `#[specta(optional)]` (`field?: T | null`): serde reads a
//! missing `Option` as `None`. Optional *output* fields are always present, as `null`.

use std::collections::BTreeMap;
use std::fmt::Display;

use mrk_money::Money;
use serde::{Deserialize, Serialize};
use specta::Type;

// ───────────────────────────── errors ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Validation,
    NotFound,
    UnknownCommand,
    /// The command is in the catalogue but its slice has not landed yet (M1).
    NotImplemented,
    /// An amount's currency differs from its account's.
    CurrencyMismatch,
    /// The postings of an entry do not sum to zero per currency.
    UnbalancedEntry,
    /// The change collides with existing state (e.g. editing a reconciled leg).
    Conflict,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct ApiError {
    pub code: ErrorCode,
    /// Developer message (English); the UI shows i18n text, not this.
    pub message: String,
    /// Field → i18n key, for forms. `null` when the error is not about specific fields.
    pub fields: Option<BTreeMap<String, String>>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            fields: None,
        }
    }
    pub fn validation(e: impl Display) -> Self {
        Self::new(ErrorCode::Validation, e.to_string())
    }
    pub fn internal(e: impl Display) -> Self {
        Self::new(ErrorCode::Internal, e.to_string())
    }
    /// A catalogued command whose slice has not landed yet.
    pub fn not_implemented(command: &str) -> Self {
        Self::new(
            ErrorCode::NotImplemented,
            format!("{command} is not implemented yet"),
        )
    }
    /// Adds a field error (field name as on the wire → i18n key).
    pub fn with_field(mut self, field: impl Into<String>, i18n_key: impl Into<String>) -> Self {
        self.fields
            .get_or_insert_with(BTreeMap::new)
            .insert(field.into(), i18n_key.into());
        self
    }
}

// ───────────────────────────── app & money ─────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    pub status: String,
    pub version: String,
    pub schema_version: u32,
    pub latest_schema_version: u32,
    pub db_path: Option<String>,
}

/// Money on the wire: minor units as a **string** (no JS number precision issues)
/// plus a display string formatted in Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MoneyDto {
    pub minor: String,
    pub currency: String,
    pub display: String,
}

impl MoneyDto {
    pub fn from_money(m: Money) -> Self {
        Self {
            minor: m.minor.to_string(),
            currency: m.currency.to_string(),
            display: m.display_es_cl(),
        }
    }
}

/// User-typed money as text; parsed in Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MoneyInput {
    pub amount: String,
    pub currency: String,
}

// ───────────────────────────── household ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum Locale {
    #[serde(rename = "es-CL")]
    EsCl,
    #[serde(rename = "en")]
    En,
}

/// How installment purchases count against budgets (docs/03).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum BudgetMode {
    AtPurchase,
    PerInstallment,
}

/// What the app needs at start: `household` is `null` until onboarding creates it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDto {
    pub household: Option<HouseholdDto>,
    pub members: Vec<MemberDto>,
    /// The member this device belongs to; `null` before onboarding.
    pub me: Option<MemberDto>,
    pub settings: Option<SettingsDto>,
    pub schema_version: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdDto {
    pub id: String,
    pub name: String,
    pub base_currency: String,
    pub locale: Locale,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MemberDto {
    pub id: String,
    pub display_name: String,
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub base_currency: String,
    pub locale: Locale,
    pub budget_mode: BudgetMode,
}

/// Onboarding: creates the household, its first member (me) and the default categories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdInput {
    pub name: String,
    pub my_name: String,
    pub base_currency: String,
    /// Defaults to es-CL.
    #[specta(optional)]
    pub locale: Option<Locale>,
}

// ───────────────────────────── accounts ─────────────────────────────

/// Which accounts and entries a screen shows (Personal / Household / Combined tabs).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum View {
    Personal,
    Household,
    Combined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Checking,
    Savings,
    CuentaVista,
    Cash,
    CreditCard,
    Loan,
    Mortgage,
    Investment,
    OtherAsset,
    OtherLiability,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AccountScope {
    Personal,
    Household,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Private,
    SharedSummary,
    SharedFull,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub name: String,
    pub kind: AccountKind,
    pub currency: String,
    pub scope: AccountScope,
    /// Set for personal accounts only.
    pub owner_member_id: Option<String>,
    pub visibility: Visibility,
    pub institution: Option<String>,
    /// e.g. `•••• 4417`; never a full number.
    pub number_mask: Option<String>,
    pub is_archived: bool,
    pub sort_order: i32,
}

/// A sidebar row: the account and its balance, computed in Rust from its live legs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummaryDto {
    pub account: AccountDto,
    pub balance: MoneyDto,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetailDto {
    pub account: AccountDto,
    pub balance: MoneyDto,
    /// The opening-balance entry, when the account has one.
    pub opening_balance: Option<OpeningBalanceDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpeningBalanceDto {
    pub amount: MoneyDto,
    pub date: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountListQuery {
    pub view: View,
    /// Defaults to `false`.
    #[specta(optional)]
    pub include_archived: Option<bool>,
}

/// Create or edit an account. Defaults: scope `household` (visibility `shared_full`);
/// a personal account is owned by me (visibility `shared_summary`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub name: String,
    pub kind: AccountKind,
    pub currency: String,
    #[specta(optional)]
    pub scope: Option<AccountScope>,
    #[specta(optional)]
    pub visibility: Option<Visibility>,
    #[specta(optional)]
    pub institution: Option<String>,
    #[specta(optional)]
    pub number_mask: Option<String>,
    #[specta(optional)]
    pub opening_balance: Option<OpeningBalanceInput>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpeningBalanceInput {
    /// In the account's currency, or `CURRENCY_MISMATCH`.
    pub amount: MoneyInput,
    pub date: String,
}

// ───────────────────────────── transactions ─────────────────────────────

/// A new (or edited) two-leg transaction; core expands it into balanced postings (design Q1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NewTxInput {
    Expense(CategoryTxInput),
    Income(CategoryTxInput),
    Transfer(TransferInput),
}

/// An expense or income: money out of / into `accountId`, against `categoryId`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTxInput {
    pub account_id: String,
    pub category_id: String,
    /// Positive amount as typed; the type gives the direction.
    pub amount: MoneyInput,
    pub date: String,
    #[specta(optional)]
    pub description: Option<String>,
    #[specta(optional)]
    pub memo: Option<String>,
}

/// Money moved between two of the household's accounts in the same currency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TransferInput {
    pub from_account_id: String,
    pub to_account_id: String,
    pub amount: MoneyInput,
    pub date: String,
    #[specta(optional)]
    pub description: Option<String>,
    #[specta(optional)]
    pub memo: Option<String>,
}

/// What an entry is, derived in core from its legs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TxKind {
    Expense,
    Income,
    Transfer,
    OpeningBalance,
}

/// Status of an account leg (ADR-0009); a reconciled leg can't be edited (`CONFLICT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum LegStatus {
    Pending,
    Cleared,
    Reconciled,
}

/// One row of the transactions table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TxRowDto {
    pub id: String,
    pub date: String,
    pub kind: TxKind,
    pub description: String,
    /// The account the row is seen from: the queried account, else the entry's first
    /// account leg (a transfer's source).
    pub account_id: String,
    pub account_name: String,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    /// A transfer's other account.
    pub counter_account_id: Option<String>,
    pub counter_account_name: Option<String>,
    /// Signed, as seen from `accountId` (an expense is negative).
    pub amount: MoneyDto,
    pub status: LegStatus,
}

/// A transaction shaped for the edit dialog (mirrors `NewTxInput`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TxDetailDto {
    pub id: String,
    pub kind: TxKind,
    pub date: String,
    pub description: String,
    pub memo: String,
    /// Expense/income/opening balance: the account; transfer: the source account.
    pub account_id: String,
    pub category_id: Option<String>,
    /// Transfer only: the destination account.
    pub to_account_id: Option<String>,
    /// Positive, as typed in the dialog.
    pub amount: MoneyDto,
    pub status: LegStatus,
    pub deleted: bool,
}

/// `tx_list` filter, keyset-paginated on `(date DESC, id DESC)` (design Q8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TxQuery {
    pub view: View,
    #[specta(optional)]
    pub account_id: Option<String>,
    /// Inclusive civil dates.
    #[specta(optional)]
    pub from: Option<String>,
    #[specta(optional)]
    pub to: Option<String>,
    /// Opaque: pass back a page's `nextCursor` unchanged.
    #[specta(optional)]
    pub cursor: Option<String>,
    /// 1–500, default 100.
    #[specta(optional)]
    pub limit: Option<u32>,
}

/// One page of a keyset-paginated list; `nextCursor` is `null` on the last page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn money(minor: &str, display: &str) -> MoneyDto {
        MoneyDto {
            minor: minor.into(),
            currency: "CLP".into(),
            display: display.into(),
        }
    }

    fn account() -> AccountDto {
        AccountDto {
            id: "acc-1".into(),
            name: "Cuenta corriente".into(),
            kind: AccountKind::CuentaVista,
            currency: "CLP".into(),
            scope: AccountScope::Household,
            owner_member_id: None,
            visibility: Visibility::SharedFull,
            institution: None,
            number_mask: Some("•••• 0000".into()),
            is_archived: false,
            sort_order: 0,
        }
    }

    #[test]
    fn account_summary_wire_shape() {
        let dto = AccountSummaryDto {
            account: account(),
            balance: money("-12500", "-$12.500"),
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "account": {
            "id": "acc-1",
            "name": "Cuenta corriente",
            "kind": "cuenta_vista",
            "currency": "CLP",
            "scope": "household",
            "ownerMemberId": null,
            "visibility": "shared_full",
            "institution": null,
            "numberMask": "•••• 0000",
            "isArchived": false,
            "sortOrder": 0
          },
          "balance": {
            "minor": "-12500",
            "currency": "CLP",
            "display": "-$12.500"
          }
        }
        "#);
    }

    #[test]
    fn tx_page_wire_shape() {
        let page = Page {
            items: vec![TxRowDto {
                id: "tx-1".into(),
                date: "2026-11-21".into(),
                kind: TxKind::Transfer,
                description: "Ahorro".into(),
                account_id: "acc-1".into(),
                account_name: "Cuenta corriente".into(),
                category_id: None,
                category_name: None,
                counter_account_id: Some("acc-2".into()),
                counter_account_name: Some("Ahorro".into()),
                amount: money("-100000", "-$100.000"),
                status: LegStatus::Cleared,
            }],
            next_cursor: Some("opaque".into()),
        };
        insta::assert_json_snapshot!(page, @r#"
        {
          "items": [
            {
              "id": "tx-1",
              "date": "2026-11-21",
              "kind": "transfer",
              "description": "Ahorro",
              "accountId": "acc-1",
              "accountName": "Cuenta corriente",
              "categoryId": null,
              "categoryName": null,
              "counterAccountId": "acc-2",
              "counterAccountName": "Ahorro",
              "amount": {
                "minor": "-100000",
                "currency": "CLP",
                "display": "-$100.000"
              },
              "status": "cleared"
            }
          ],
          "nextCursor": "opaque"
        }
        "#);
    }

    #[test]
    fn bootstrap_before_onboarding_wire_shape() {
        let dto = BootstrapDto {
            household: None,
            members: vec![],
            me: None,
            settings: None,
            schema_version: 2,
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "household": null,
          "members": [],
          "me": null,
          "settings": null,
          "schemaVersion": 2
        }
        "#);
    }

    #[test]
    fn bootstrap_after_onboarding_wire_shape() {
        let me = MemberDto {
            id: "mem-1".into(),
            display_name: "Yo".into(),
            color: None,
        };
        let dto = BootstrapDto {
            household: Some(HouseholdDto {
                id: "hh-1".into(),
                name: "Casa".into(),
                base_currency: "CLP".into(),
                locale: Locale::EsCl,
            }),
            members: vec![me.clone()],
            me: Some(me),
            settings: Some(SettingsDto {
                base_currency: "CLP".into(),
                locale: Locale::EsCl,
                budget_mode: BudgetMode::PerInstallment,
            }),
            schema_version: 2,
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "household": {
            "id": "hh-1",
            "name": "Casa",
            "baseCurrency": "CLP",
            "locale": "es-CL"
          },
          "members": [
            {
              "id": "mem-1",
              "displayName": "Yo",
              "color": null
            }
          ],
          "me": {
            "id": "mem-1",
            "displayName": "Yo",
            "color": null
          },
          "settings": {
            "baseCurrency": "CLP",
            "locale": "es-CL",
            "budgetMode": "per_installment"
          },
          "schemaVersion": 2
        }
        "#);
    }

    #[test]
    fn tx_detail_wire_shape() {
        let dto = TxDetailDto {
            id: "tx-1".into(),
            kind: TxKind::Expense,
            date: "2026-11-21".into(),
            description: "Supermercado".into(),
            memo: String::new(),
            account_id: "acc-1".into(),
            category_id: Some("cat-1".into()),
            to_account_id: None,
            amount: money("27990", "$27.990"),
            status: LegStatus::Pending,
            deleted: false,
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "id": "tx-1",
          "kind": "expense",
          "date": "2026-11-21",
          "description": "Supermercado",
          "memo": "",
          "accountId": "acc-1",
          "categoryId": "cat-1",
          "toAccountId": null,
          "amount": {
            "minor": "27990",
            "currency": "CLP",
            "display": "$27.990"
          },
          "status": "pending",
          "deleted": false
        }
        "#);
    }

    #[test]
    fn new_tx_input_is_tagged_by_type() {
        let expense: NewTxInput = serde_json::from_value(json!({
            "type": "expense",
            "accountId": "acc-1",
            "categoryId": "cat-1",
            "amount": { "amount": "27.990", "currency": "CLP" },
            "date": "2026-11-21"
        }))
        .unwrap();
        assert_eq!(
            expense,
            NewTxInput::Expense(CategoryTxInput {
                account_id: "acc-1".into(),
                category_id: "cat-1".into(),
                amount: MoneyInput {
                    amount: "27.990".into(),
                    currency: "CLP".into()
                },
                date: "2026-11-21".into(),
                description: None,
                memo: None,
            })
        );
        let income: NewTxInput = serde_json::from_value(json!({
            "type": "income", "accountId": "a", "categoryId": "c",
            "amount": { "amount": "1", "currency": "CLP" }, "date": "2026-11-21",
            "description": "Sueldo", "memo": null
        }))
        .unwrap();
        assert!(
            matches!(income, NewTxInput::Income(ref i) if i.description.as_deref() == Some("Sueldo"))
        );
        let transfer: NewTxInput = serde_json::from_value(json!({
            "type": "transfer", "fromAccountId": "a", "toAccountId": "b",
            "amount": { "amount": "100.000", "currency": "CLP" }, "date": "2026-11-21"
        }))
        .unwrap();
        assert!(matches!(transfer, NewTxInput::Transfer(ref t) if t.to_account_id == "b"));
        // And it serializes back to the same tagged shape.
        assert_eq!(serde_json::to_value(&transfer).unwrap()["type"], "transfer");
    }

    #[test]
    fn new_tx_input_rejects_unknown_type_or_missing_fields() {
        for bad in [
            json!({ "type": "split", "accountId": "a" }),
            json!({ "accountId": "a", "categoryId": "c", "amount": { "amount": "1", "currency": "CLP" }, "date": "2026-11-21" }),
            json!({ "type": "transfer", "fromAccountId": "a", "date": "2026-11-21" }),
        ] {
            assert!(
                serde_json::from_value::<NewTxInput>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn optional_inputs_may_be_omitted() {
        let q: TxQuery = serde_json::from_value(json!({ "view": "combined" })).unwrap();
        assert_eq!(q.limit, None);
        assert_eq!(q.cursor, None);
        let a: AccountInput = serde_json::from_value(json!({
            "name": "Ahorro", "kind": "savings", "currency": "CLP"
        }))
        .unwrap();
        assert_eq!(a.opening_balance, None);
        let h: HouseholdInput = serde_json::from_value(json!({
            "name": "Casa", "myName": "Yo", "baseCurrency": "CLP"
        }))
        .unwrap();
        assert_eq!(h.locale, None);
    }

    #[test]
    fn errors_carry_codes_and_fields() {
        let e = ApiError::new(ErrorCode::CurrencyMismatch, "USD amount on a CLP account")
            .with_field("amount", "errors.currencyMismatch");
        insta::assert_json_snapshot!(e, @r#"
        {
          "code": "CURRENCY_MISMATCH",
          "message": "USD amount on a CLP account",
          "fields": {
            "amount": "errors.currencyMismatch"
          }
        }
        "#);
        let codes: Vec<_> = [
            ErrorCode::NotImplemented,
            ErrorCode::UnbalancedEntry,
            ErrorCode::Conflict,
        ]
        .iter()
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();
        assert_eq!(
            codes,
            vec![
                json!("NOT_IMPLEMENTED"),
                json!("UNBALANCED_ENTRY"),
                json!("CONFLICT")
            ]
        );
        assert_eq!(
            serde_json::to_value(ApiError::validation("x")).unwrap(),
            json!({ "code": "VALIDATION", "message": "x", "fields": null })
        );
        // A client that omits `fields` (M0 shape) still deserializes.
        let old: ApiError =
            serde_json::from_value(json!({ "code": "NOT_FOUND", "message": "nope" })).unwrap();
        assert_eq!(old.fields, None);
    }
}
