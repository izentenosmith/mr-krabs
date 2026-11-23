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

// ───────────────────────────── categories ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CategoryKind {
    Expense,
    Income,
}

/// `category_tree` filter: one kind's tree, or both.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTreeQuery {
    #[specta(optional)]
    pub kind: Option<CategoryKind>,
}

/// One node of the live category tree (any depth): seeded and user categories, archived
/// ones flagged, deleted and merged ones left out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryNodeDto {
    pub id: String,
    /// `null` at a root.
    pub parent_id: Option<String>,
    pub kind: CategoryKind,
    pub name: String,
    pub icon: Option<String>,
    pub is_archived: bool,
    pub sort_order: i32,
    pub children: Vec<CategoryNodeDto>,
}

/// One category as written by `category_create`/`update`/`move`/`archive` (no children).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: String,
    /// `null` at a root.
    pub parent_id: Option<String>,
    pub kind: CategoryKind,
    pub name: String,
    pub icon: Option<String>,
    pub is_archived: bool,
    pub sort_order: i32,
}

/// `category_create`: a new category of `kind`, at the root or under `parentId` (same kind).
/// `name` is trimmed, 1–80 characters, unique among its live siblings (DEC-30 Q10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub kind: CategoryKind,
    #[specta(optional)]
    pub parent_id: Option<String>,
    pub name: String,
    #[specta(optional)]
    pub icon: Option<String>,
}

/// `category_update`: rename and/or change the icon; kind is immutable. An omitted (or
/// `null`) field is left unchanged; an empty `icon` clears it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUpdateInput {
    pub id: String,
    #[specta(optional)]
    pub name: Option<String>,
    #[specta(optional)]
    pub icon: Option<String>,
}

/// `category_move`: reparent (`parentId` omitted or `null` = root) and place at `index`
/// (0-based) among the new siblings. Cycles and a different kind are `VALIDATION`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryMoveInput {
    pub id: String,
    #[specta(optional)]
    pub parent_id: Option<String>,
    pub index: u32,
}

/// `category_archive`: archive (cascades to the subtree for pickers) or unarchive. History
/// and reports keep archived categories.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryArchiveInput {
    pub id: String,
    pub archived: bool,
}

/// `category_delete`: without `mergeIntoId` the category must be unused (else
/// `VALIDATION`); with it, its entries, budget lines and sub-categories move there first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDeleteInput {
    pub id: String,
    #[specta(optional)]
    pub merge_into_id: Option<String>,
}

// ───────────────────────────── budgets ─────────────────────────────

/// Whose budget line: the household's, or my personal one (the member is always me).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum BudgetScope {
    Personal,
    Household,
}

/// `budget_set`: upserts the one live line for `(period, categoryId, scope, me)` (DEC-30
/// Q9). `amount` is typed text in the household's base currency; `""` or zero deletes the
/// line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetSetInput {
    /// `YYYY-MM`.
    pub period: String,
    pub category_id: String,
    pub scope: BudgetScope,
    pub amount: String,
}

/// A budget line as stored. `budget_set` answers `null` instead when it deleted the line
/// (or there was none to delete).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCellDto {
    pub id: String,
    pub period: String,
    pub category_id: String,
    pub scope: BudgetScope,
    /// Set for personal lines only.
    pub member_id: Option<String>,
    pub amount: MoneyDto,
}

/// `budget_copy_month`: copies `from`'s live lines into `to` (both `YYYY-MM`). Only empty
/// slots are filled unless `overwrite` (default `false`). Answers the number of lines
/// written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetCopyInput {
    pub from: String,
    pub to: String,
    #[specta(optional)]
    pub overwrite: Option<bool>,
}

// ───────────────────────────── reports ─────────────────────────────
//
// Every report totals the household's base currency only; postings in other currencies are
// left out and counted in `excluded` (DEC-29 Q8). Actuals are by entry date, net of refunds,
// and positive in the row's own direction (spent for expense, received for income); transfers
// are never income or expense (DEC-29 Q7). A group row's amounts are its own plus all its
// descendants' (roll-up, DEC-30 Q9). Rows are in category-tree order (depth-first, siblings
// by sort order), archived categories included. Percentages are integer basis points
// (10000 = 100 %), never floats.

/// Postings left out of a report because they are not in the base currency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedDto {
    pub currency: String,
    pub count: u32,
}

/// A category row of a report: where it sits in the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportCategoryDto {
    pub id: String,
    pub parent_id: Option<String>,
    /// 0 at a root.
    pub depth: u32,
    pub kind: CategoryKind,
    pub name: String,
    pub is_archived: bool,
    /// Has sub-categories: its amounts are roll-up subtotals.
    pub is_group: bool,
}

/// Planned vs actual for one month; `diff` = planned − actual.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanActualDto {
    pub planned: MoneyDto,
    pub actual: MoneyDto,
    pub diff: MoneyDto,
}

/// `budget_matrix`: one calendar year.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetMatrixQuery {
    /// 1970–9999.
    pub year: u16,
    pub view: View,
}

/// One category row of the matrix: 12 months, January first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetMatrixRowDto {
    pub category: ReportCategoryDto,
    /// Planned (rolled up), actual (rolled up) and diff, per month.
    pub months: Vec<PlanActualDto>,
    /// This category's own planned line per month (no descendants), for in-cell editing.
    pub own_planned: Vec<MoneyDto>,
}

/// The matrix's summary rows, 12 months each, January first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetMatrixSummaryDto {
    pub income: Vec<PlanActualDto>,
    pub expense: Vec<PlanActualDto>,
    pub saved: Vec<PlanActualDto>,
    pub free_cash_flow: Vec<PlanActualDto>,
    pub total_expenses: Vec<PlanActualDto>,
}

/// `budget_matrix`: a year of plan vs real (FR-BU-08).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetMatrixDto {
    pub year: u16,
    pub view: View,
    pub base_currency: String,
    pub rows: Vec<BudgetMatrixRowDto>,
    pub summary: BudgetMatrixSummaryDto,
    pub excluded: Vec<ExcludedDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthCalendarQuery {
    /// `YYYY-MM`.
    pub month: String,
    pub view: View,
}

/// One category row of the calendar: an actual per day of the month.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRowDto {
    pub category: ReportCategoryDto,
    /// `daysInMonth` entries, day 1 first.
    pub days: Vec<MoneyDto>,
    pub total: MoneyDto,
}

/// Per-day totals of the calendar's root rows, by kind (`daysInMonth` entries each).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDayTotalsDto {
    pub income: Vec<MoneyDto>,
    pub expense: Vec<MoneyDto>,
}

/// `report_month_calendar`: categories × days for one month.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDto {
    pub month: String,
    pub view: View,
    pub base_currency: String,
    pub days_in_month: u32,
    pub rows: Vec<CalendarRowDto>,
    pub day_totals: CalendarDayTotalsDto,
    pub excluded: Vec<ExcludedDto>,
}

/// `report_cash_flow`: monthly, `from`..=`to` (both `YYYY-MM`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashFlowQuery {
    pub from: String,
    pub to: String,
    pub view: View,
}

/// One month of cash flow: free cash flow = income − expense − saved (DEC-29 Q7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashFlowMonthDto {
    /// `YYYY-MM`.
    pub month: String,
    pub income: MoneyDto,
    pub expense: MoneyDto,
    pub saved: MoneyDto,
    pub free_cash_flow: MoneyDto,
    /// saved ÷ income in basis points; `null` when income is 0. Clamped to the `i32`
    /// range (an extreme ratio is reported at the bound, never wrapped).
    pub savings_rate_bp: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CashFlowDto {
    pub view: View,
    pub base_currency: String,
    /// One entry per month, `from` first.
    pub months: Vec<CashFlowMonthDto>,
    pub excluded: Vec<ExcludedDto>,
}

/// `report_spending`: expense by category, `from`..=`to` (both `YYYY-MM`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpendingQuery {
    pub from: String,
    pub to: String,
    pub view: View,
}

/// One category's spending (rolled up) and its share of `total`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownRowDto {
    pub category: ReportCategoryDto,
    pub amount: MoneyDto,
    /// Basis points of `total`; `null` when `total` is 0.
    pub share_bp: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BreakdownDto {
    pub view: View,
    pub base_currency: String,
    pub rows: Vec<BreakdownRowDto>,
    pub total: MoneyDto,
    pub excluded: Vec<ExcludedDto>,
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
    /// Entries with a leg in this category or any of its descendants (drill-down).
    #[specta(optional)]
    pub category_id: Option<String>,
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

    fn category() -> ReportCategoryDto {
        ReportCategoryDto {
            id: "cat-1".into(),
            parent_id: None,
            depth: 0,
            kind: CategoryKind::Expense,
            name: "Alimentación".into(),
            is_archived: false,
            is_group: true,
        }
    }

    #[test]
    fn m2_inputs_parse_with_optionals_omitted() {
        let c: CategoryInput =
            serde_json::from_value(json!({ "kind": "income", "name": "Bono" })).unwrap();
        assert_eq!((c.parent_id, c.icon), (None, None));
        let u: CategoryUpdateInput = serde_json::from_value(json!({ "id": "c" })).unwrap();
        assert_eq!((u.name, u.icon), (None, None));
        let m: CategoryMoveInput =
            serde_json::from_value(json!({ "id": "c", "index": 2 })).unwrap();
        assert_eq!((m.parent_id, m.index), (None, 2));
        let d: CategoryDeleteInput = serde_json::from_value(json!({ "id": "c" })).unwrap();
        assert_eq!(d.merge_into_id, None);
        let b: BudgetCopyInput =
            serde_json::from_value(json!({ "from": "2026-10", "to": "2026-11" })).unwrap();
        assert_eq!(b.overwrite, None);
        let s: BudgetSetInput = serde_json::from_value(json!({
            "period": "2026-11", "categoryId": "c", "scope": "personal", "amount": ""
        }))
        .unwrap();
        assert_eq!(s.scope, BudgetScope::Personal);
        let q: TxQuery =
            serde_json::from_value(json!({ "view": "combined", "categoryId": "c" })).unwrap();
        assert_eq!(q.category_id.as_deref(), Some("c"));
        assert!(
            serde_json::from_value::<BudgetMatrixQuery>(
                json!({ "year": 70000, "view": "combined" })
            )
            .is_err()
        );
    }

    /// #37 F4: every M2 input field is read from its camelCase wire name (an optional field
    /// read under the wrong name would silently become `None`).
    #[test]
    fn m2_inputs_read_every_field_by_its_wire_name() {
        let c: CategoryInput = serde_json::from_value(json!({
            "kind": "expense", "parentId": "p", "name": "Cine", "icon": "film"
        }))
        .unwrap();
        assert_eq!(
            c,
            CategoryInput {
                kind: CategoryKind::Expense,
                parent_id: Some("p".into()),
                name: "Cine".into(),
                icon: Some("film".into()),
            }
        );
        let u: CategoryUpdateInput =
            serde_json::from_value(json!({ "id": "c", "name": "Cine", "icon": "" })).unwrap();
        assert_eq!(
            (u.name.as_deref(), u.icon.as_deref()),
            (Some("Cine"), Some(""))
        );
        let m: CategoryMoveInput =
            serde_json::from_value(json!({ "id": "c", "parentId": "p", "index": 1 })).unwrap();
        assert_eq!((m.parent_id.as_deref(), m.index), (Some("p"), 1));
        let a: CategoryArchiveInput =
            serde_json::from_value(json!({ "id": "c", "archived": true })).unwrap();
        assert!(a.archived);
        let d: CategoryDeleteInput =
            serde_json::from_value(json!({ "id": "c", "mergeIntoId": "d" })).unwrap();
        assert_eq!(d.merge_into_id.as_deref(), Some("d"));
        let s: BudgetSetInput = serde_json::from_value(json!({
            "period": "2026-11", "categoryId": "c", "scope": "household", "amount": "150.000"
        }))
        .unwrap();
        assert_eq!(
            (
                s.period.as_str(),
                s.category_id.as_str(),
                s.scope,
                s.amount.as_str()
            ),
            ("2026-11", "c", BudgetScope::Household, "150.000")
        );
        let b: BudgetCopyInput = serde_json::from_value(json!({
            "from": "2026-10", "to": "2026-11", "overwrite": true
        }))
        .unwrap();
        assert_eq!(b.overwrite, Some(true));
        let q: BudgetMatrixQuery =
            serde_json::from_value(json!({ "year": 2026, "view": "household" })).unwrap();
        assert_eq!((q.year, q.view), (2026, View::Household));
        let q: MonthCalendarQuery =
            serde_json::from_value(json!({ "month": "2026-02", "view": "personal" })).unwrap();
        assert_eq!(q.month, "2026-02");
        let q: CashFlowQuery = serde_json::from_value(
            json!({ "from": "2026-01", "to": "2026-03", "view": "combined" }),
        )
        .unwrap();
        assert_eq!((q.from.as_str(), q.to.as_str()), ("2026-01", "2026-03"));
        let q: SpendingQuery = serde_json::from_value(
            json!({ "from": "2026-01", "to": "2026-03", "view": "combined" }),
        )
        .unwrap();
        assert_eq!((q.from.as_str(), q.to.as_str()), ("2026-01", "2026-03"));
    }

    #[test]
    fn category_wire_shape() {
        let dto = CategoryDto {
            id: "cat-2".into(),
            parent_id: Some("cat-1".into()),
            kind: CategoryKind::Expense,
            name: "Supermercado".into(),
            icon: None,
            is_archived: false,
            sort_order: 3,
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "id": "cat-2",
          "parentId": "cat-1",
          "kind": "expense",
          "name": "Supermercado",
          "icon": null,
          "isArchived": false,
          "sortOrder": 3
        }
        "#);
    }

    #[test]
    fn budget_cell_wire_shape() {
        let dto = BudgetCellDto {
            id: "bl-1".into(),
            period: "2026-11".into(),
            category_id: "cat-1".into(),
            scope: BudgetScope::Household,
            member_id: None,
            amount: money("150000", "$150.000"),
        };
        insta::assert_json_snapshot!(dto, @r#"
        {
          "id": "bl-1",
          "period": "2026-11",
          "categoryId": "cat-1",
          "scope": "household",
          "memberId": null,
          "amount": {
            "minor": "150000",
            "currency": "CLP",
            "display": "$150.000"
          }
        }
        "#);
    }

    #[test]
    fn budget_matrix_wire_shape() {
        let cell = PlanActualDto {
            planned: money("150000", "$150.000"),
            actual: money("120000", "$120.000"),
            diff: money("30000", "$30.000"),
        };
        // One month instead of twelve keeps the snapshot readable; the shape is the same.
        let dto = BudgetMatrixDto {
            year: 2026,
            view: View::Combined,
            base_currency: "CLP".into(),
            rows: vec![BudgetMatrixRowDto {
                category: category(),
                months: vec![cell.clone()],
                own_planned: vec![money("0", "$0")],
            }],
            summary: BudgetMatrixSummaryDto {
                income: vec![cell.clone()],
                expense: vec![cell.clone()],
                saved: vec![cell.clone()],
                free_cash_flow: vec![cell.clone()],
                total_expenses: vec![cell],
            },
            excluded: vec![ExcludedDto {
                currency: "USD".into(),
                count: 2,
            }],
        };
        let v = serde_json::to_value(&dto).unwrap();
        insta::assert_json_snapshot!(v["rows"], @r#"
        [
          {
            "category": {
              "depth": 0,
              "id": "cat-1",
              "isArchived": false,
              "isGroup": true,
              "kind": "expense",
              "name": "Alimentación",
              "parentId": null
            },
            "months": [
              {
                "actual": {
                  "currency": "CLP",
                  "display": "$120.000",
                  "minor": "120000"
                },
                "diff": {
                  "currency": "CLP",
                  "display": "$30.000",
                  "minor": "30000"
                },
                "planned": {
                  "currency": "CLP",
                  "display": "$150.000",
                  "minor": "150000"
                }
              }
            ],
            "ownPlanned": [
              {
                "currency": "CLP",
                "display": "$0",
                "minor": "0"
              }
            ]
          }
        ]
        "#);
        let keys = |o: &serde_json::Value| -> Vec<String> {
            o.as_object().unwrap().keys().cloned().collect()
        };
        assert_eq!(
            keys(&v),
            [
                "baseCurrency",
                "excluded",
                "rows",
                "summary",
                "view",
                "year"
            ]
        );
        assert_eq!(
            keys(&v["summary"]),
            [
                "expense",
                "freeCashFlow",
                "income",
                "saved",
                "totalExpenses"
            ]
        );
        assert_eq!(v["excluded"], json!([{ "currency": "USD", "count": 2 }]));
        assert_eq!(
            (v["year"].clone(), v["view"].clone()),
            (json!(2026), json!("combined"))
        );
    }

    #[test]
    fn calendar_wire_shape() {
        let dto = CalendarDto {
            month: "2026-02".into(),
            view: View::Household,
            base_currency: "CLP".into(),
            days_in_month: 28,
            rows: vec![CalendarRowDto {
                category: category(),
                days: vec![money("5000", "$5.000")],
                total: money("5000", "$5.000"),
            }],
            day_totals: CalendarDayTotalsDto {
                income: vec![money("0", "$0")],
                expense: vec![money("5000", "$5.000")],
            },
            excluded: vec![],
        };
        let v = serde_json::to_value(&dto).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "baseCurrency",
                "dayTotals",
                "daysInMonth",
                "excluded",
                "month",
                "rows",
                "view"
            ]
        );
        assert_eq!(v["daysInMonth"], 28);
        assert_eq!(v["rows"][0]["days"][0]["minor"], "5000");
        assert_eq!(v["rows"][0]["total"]["display"], "$5.000");
        assert_eq!(v["dayTotals"]["income"][0]["minor"], "0");
        assert_eq!(v["dayTotals"]["expense"][0]["minor"], "5000");
    }

    #[test]
    fn cash_flow_wire_shape() {
        let dto = CashFlowDto {
            view: View::Combined,
            base_currency: "CLP".into(),
            months: vec![CashFlowMonthDto {
                month: "2026-11".into(),
                income: money("2000000", "$2.000.000"),
                expense: money("1500000", "$1.500.000"),
                saved: money("300000", "$300.000"),
                free_cash_flow: money("200000", "$200.000"),
                savings_rate_bp: Some(1500),
            }],
            excluded: vec![],
        };
        insta::assert_json_snapshot!(serde_json::to_value(&dto).unwrap()["months"][0], @r#"
        {
          "expense": {
            "currency": "CLP",
            "display": "$1.500.000",
            "minor": "1500000"
          },
          "freeCashFlow": {
            "currency": "CLP",
            "display": "$200.000",
            "minor": "200000"
          },
          "income": {
            "currency": "CLP",
            "display": "$2.000.000",
            "minor": "2000000"
          },
          "month": "2026-11",
          "saved": {
            "currency": "CLP",
            "display": "$300.000",
            "minor": "300000"
          },
          "savingsRateBp": 1500
        }
        "#);
    }

    #[test]
    fn breakdown_wire_shape() {
        let dto = BreakdownDto {
            view: View::Personal,
            base_currency: "CLP".into(),
            rows: vec![BreakdownRowDto {
                category: category(),
                amount: money("0", "$0"),
                share_bp: None,
            }],
            total: money("0", "$0"),
            excluded: vec![ExcludedDto {
                currency: "USD".into(),
                count: 1,
            }],
        };
        let v = serde_json::to_value(&dto).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["baseCurrency", "excluded", "rows", "total", "view"]);
        assert_eq!(v["rows"][0]["shareBp"], serde_json::Value::Null);
        assert_eq!(v["rows"][0]["category"]["isGroup"], true);
        assert_eq!(v["total"]["minor"], "0");
    }

    #[test]
    fn errors_carry_codes_and_fields() {
        let e = ApiError::new(ErrorCode::CurrencyMismatch, "USD amount on a CLP account")
            .with_field("amount", "errors.field.currencyMismatch");
        insta::assert_json_snapshot!(e, @r#"
        {
          "code": "CURRENCY_MISMATCH",
          "message": "USD amount on a CLP account",
          "fields": {
            "amount": "errors.field.currencyMismatch"
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
