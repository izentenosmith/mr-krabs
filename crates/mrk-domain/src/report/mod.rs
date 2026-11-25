//! Reports (M2, DEC-29/DEC-30): pure functions over rows the store loads with plain
//! `SELECT … WHERE` queries. Money is never aggregated in SQL (ADR-0003); every total here is
//! computed in the household's base currency with checked arithmetic.

use std::collections::BTreeMap;

use mrk_money::{CurrencyCode, Money, MoneyError};

use crate::category::CategoryKind;
use crate::ledger::Leg;

mod calendar;
mod cash_flow;
mod flows;
mod matrix;
mod period;
mod spending;
mod tree;

pub use calendar::month_calendar;
pub use cash_flow::cash_flow;
pub use matrix::budget_matrix;
pub use spending::spending;
pub use tree::{CategoryTree, TreeRow};

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
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "checking" => Self::Checking,
            "savings" => Self::Savings,
            "cuenta_vista" => Self::CuentaVista,
            "cash" => Self::Cash,
            "credit_card" => Self::CreditCard,
            "loan" => Self::Loan,
            "mortgage" => Self::Mortgage,
            "investment" => Self::Investment,
            "other_asset" => Self::OtherAsset,
            "other_liability" => Self::OtherLiability,
            _ => return None,
        })
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

impl Viewer {
    /// The view filter (DEC-30 Q9): household sees household accounts and lines, personal sees
    /// the personal ones I own, combined sees all. M3 extends this with shared-entry shares.
    pub fn sees(&self, scope: &Scope) -> bool {
        match (self.view, scope) {
            (View::Combined, _) | (View::Household, Scope::Household) => true,
            (View::Personal, Scope::Personal { owner }) => *owner == self.me,
            _ => false,
        }
    }
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

/// Planned vs actual for one category and period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub planned: Money,
    pub actual: Money,
    /// planned − actual.
    pub diff: Money,
}

impl Cell {
    pub(crate) fn new(planned: Money, actual: Money) -> Result<Self, ReportError> {
        Ok(Self {
            planned,
            actual,
            diff: planned.checked_sub(actual)?,
        })
    }

    pub(crate) fn zero(base: CurrencyCode) -> Self {
        let zero = Money::zero(base);
        Self {
            planned: zero,
            actual: zero,
            diff: zero,
        }
    }

    pub(crate) fn checked_add(&self, other: &Cell) -> Result<Self, ReportError> {
        Self::new(
            self.planned.checked_add(other.planned)?,
            self.actual.checked_add(other.actual)?,
        )
    }
}

/// The `excluded` list of a report: counts per currency, in currency-code order.
pub(crate) fn excluded_list(counts: BTreeMap<CurrencyCode, u32>) -> Vec<Excluded> {
    counts
        .into_iter()
        .map(|(currency, count)| Excluded { currency, count })
        .collect()
}

/// Adds `amount` to `map[key]` (absent = zero), refusing overflow and mixed currencies.
pub(crate) fn add_to(
    map: &mut BTreeMap<String, Money>,
    key: &str,
    amount: Money,
) -> Result<(), ReportError> {
    match map.get_mut(key) {
        Some(sum) => *sum = sum.checked_add(amount)?,
        None => {
            map.insert(key.to_owned(), amount);
        }
    }
    Ok(())
}

/// The checked sum of `amounts` (zero when empty).
pub(crate) fn sum_money(amounts: &[Money], base: CurrencyCode) -> Result<Money, ReportError> {
    amounts
        .iter()
        .try_fold(Money::zero(base), |s, a| Ok(s.checked_add(*a)?))
}

/// The total of a kind's root rows in a rolled-up map: everything of that kind in the tree.
pub(crate) fn sum_roots(
    tree: &CategoryTree,
    rolled: &BTreeMap<String, Money>,
    kind: CategoryKind,
    base: CurrencyCode,
) -> Result<Money, ReportError> {
    tree.rows()
        .iter()
        .filter(|r| r.depth == 0 && r.kind == kind)
        .try_fold(Money::zero(base), |sum, r| {
            Ok(sum.checked_add(rolled[&r.id])?)
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `account.kind` code in schema v2 (`account_kind` table) maps to its kind, and back
    /// through `is_saving` only savings and investment are saving accounts.
    #[test]
    fn account_kind_reads_every_stored_code() {
        let codes = [
            ("checking", AccountKind::Checking),
            ("savings", AccountKind::Savings),
            ("cuenta_vista", AccountKind::CuentaVista),
            ("cash", AccountKind::Cash),
            ("credit_card", AccountKind::CreditCard),
            ("loan", AccountKind::Loan),
            ("mortgage", AccountKind::Mortgage),
            ("investment", AccountKind::Investment),
            ("other_asset", AccountKind::OtherAsset),
            ("other_liability", AccountKind::OtherLiability),
        ];
        for (code, kind) in codes {
            assert_eq!(AccountKind::from_code(code), Some(kind), "{code}");
        }
        let saving: Vec<_> = codes
            .iter()
            .filter(|(_, k)| k.is_saving())
            .map(|(c, _)| *c)
            .collect();
        assert_eq!(saving, ["savings", "investment"]);
        for unknown in ["", "Savings", "brokerage", " savings"] {
            assert_eq!(AccountKind::from_code(unknown), None, "{unknown:?}");
        }
    }
}
