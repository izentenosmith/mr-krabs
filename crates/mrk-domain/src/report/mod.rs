//! Reports (M2, DEC-29/DEC-30): pure functions over rows the store loads with plain
//! `SELECT … WHERE` queries. Money is never aggregated in SQL (ADR-0003); every total here is
//! computed in the household's base currency with checked arithmetic.

use mrk_money::{CurrencyCode, Money, MoneyError};

use crate::category::CategoryKind;
use crate::ledger::Leg;

/// An account's kind, as stored in `account.kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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

impl AccountKind {
    /// The kind for a stored `account.kind` code; `None` for an unknown code.
    pub fn from_code(_code: &str) -> Option<Self> {
        unimplemented!("R0: AccountKind::from_code")
    }

    /// Savings and investment accounts: money moved into them is "saved" (DEC-29 Q7).
    pub const fn is_saving(self) -> bool {
        matches!(self, Self::Savings | Self::Investment)
    }
}

/// Who an account or budget line belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Household,
    Personal { owner: String },
}

/// The Personal / Household / Combined tabs (DEC-30 Q9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Household,
    Personal,
    Combined,
}

/// The view a report is computed for, and the member it is computed by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Viewer {
    pub view: View,
    pub me: String,
}

/// A live category row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportCategory {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: CategoryKind,
    pub archived: bool,
    pub sort_order: i64,
    pub name: String,
}

/// An account row (archived ones included: history keeps them).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportAccount {
    pub id: String,
    pub kind: AccountKind,
    pub scope: Scope,
}

/// A live posting of a live entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportPosting {
    pub entry_id: String,
    /// The entry's civil date, `YYYY-MM-DD`.
    pub entry_date: String,
    /// `journal_entry.kind = 'opening_balance'`: never counted (M1/DEC-4, ADR-0011).
    pub opening_balance: bool,
    pub leg: Leg,
    pub amount: Money,
}

/// A live budget line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportBudgetLine {
    /// `YYYY-MM`.
    pub period: String,
    pub category_id: String,
    pub scope: Scope,
    pub amount: Money,
}

/// Everything a report reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportData {
    pub base: CurrencyCode,
    pub categories: Vec<ReportCategory>,
    pub accounts: Vec<ReportAccount>,
    pub postings: Vec<ReportPosting>,
    pub budget_lines: Vec<ReportBudgetLine>,
}

/// Entries (and budget lines) left out of a report because they are not in the base currency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Excluded {
    pub currency: CurrencyCode,
    pub count: u32,
}

/// Why a report was refused. Core maps every variant to `VALIDATION`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReportError {
    #[error("year {0} is outside 1970–9999")]
    InvalidYear(i32),
    #[error("invalid month {0:?}, expected YYYY-MM")]
    InvalidMonth(String),
    #[error("invalid date {0:?}, expected YYYY-MM-DD")]
    InvalidDate(String),
    #[error("the range starts ({from}) after it ends ({to})")]
    InvalidRange { from: String, to: String },
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// A category as a report row: its place in the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub kind: CategoryKind,
    pub name: String,
    pub archived: bool,
    /// Has at least one child in the tree.
    pub is_group: bool,
}

/// The live category tree in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryTree {
    rows: Vec<TreeRow>,
}

impl CategoryTree {
    pub fn new(_categories: &[ReportCategory]) -> Self {
        unimplemented!("R0: CategoryTree::new")
    }

    pub fn rows(&self) -> &[TreeRow] {
        &self.rows
    }

    pub fn rollup(
        &self,
        _own: &std::collections::BTreeMap<String, Money>,
        _base: CurrencyCode,
    ) -> Result<std::collections::BTreeMap<String, Money>, ReportError> {
        unimplemented!("R0: CategoryTree::rollup")
    }
}

/// Planned vs actual for one category and period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub planned: Money,
    pub actual: Money,
    /// planned − actual.
    pub diff: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatrixRow {
    pub category: TreeRow,
    /// January … December.
    pub months: Vec<Cell>,
    pub year_total: Cell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatrixMonth {
    /// `YYYY-MM`.
    pub month: String,
    pub income: Cell,
    pub expense: Cell,
    pub saved: Money,
    pub free_cash_flow: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetMatrix {
    pub year: i32,
    pub rows: Vec<MatrixRow>,
    pub months: Vec<MatrixMonth>,
    pub excluded: Vec<Excluded>,
}

pub fn budget_matrix(
    _data: &ReportData,
    _viewer: &Viewer,
    _year: i32,
) -> Result<BudgetMatrix, ReportError> {
    unimplemented!("R0: budget_matrix")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarRow {
    pub category: TreeRow,
    /// Day 1 … `days_in_month`.
    pub days: Vec<Money>,
    pub total: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MonthCalendar {
    pub month: String,
    pub days_in_month: u32,
    pub rows: Vec<CalendarRow>,
    pub income_by_day: Vec<Money>,
    pub expense_by_day: Vec<Money>,
    pub income_total: Money,
    pub expense_total: Money,
    pub excluded: Vec<Excluded>,
}

pub fn month_calendar(
    _data: &ReportData,
    _viewer: &Viewer,
    _month: &str,
) -> Result<MonthCalendar, ReportError> {
    unimplemented!("R0: month_calendar")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CashFlowLine {
    pub income: Money,
    pub expense: Money,
    pub saved: Money,
    pub free_cash_flow: Money,
    pub savings_rate_bp: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CashFlowMonth {
    pub month: String,
    pub flow: CashFlowLine,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CashFlow {
    pub months: Vec<CashFlowMonth>,
    pub total: CashFlowLine,
    pub excluded: Vec<Excluded>,
}

pub fn cash_flow(
    _data: &ReportData,
    _viewer: &Viewer,
    _from: &str,
    _to: &str,
) -> Result<CashFlow, ReportError> {
    unimplemented!("R0: cash_flow")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendingRow {
    pub category: TreeRow,
    pub amount: Money,
    pub share_bp: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spending {
    pub rows: Vec<SpendingRow>,
    pub total: Money,
    pub excluded: Vec<Excluded>,
}

pub fn spending(
    _data: &ReportData,
    _viewer: &Viewer,
    _from: &str,
    _to: &str,
) -> Result<Spending, ReportError> {
    unimplemented!("R0: spending")
}
