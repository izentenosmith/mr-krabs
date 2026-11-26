//! Monthly cash flow (DEC-29 Q7): income, expense, saved, free cash flow and savings rate.

use mrk_money::{Money, MoneyError};

use super::flows::flows;
use super::period::DateRange;
use super::{
    CashFlow, CashFlowLine, CashFlowMonth, CategoryTree, ReportData, ReportError, Viewer,
    excluded_list,
};
use crate::category::CategoryKind;

/// Cash flow over the civil dates `from`–`to` (inclusive; `InvalidDate`, `InvalidRange`) for
/// `viewer`: one line per calendar month the range touches, counting only entries dated inside
/// the range, and a total line.
///
/// - income and expense: the net actuals of the income and expense categories;
/// - saved: net transfers into savings and investment accounts;
/// - free cash flow = income − expense − saved;
/// - savings rate = saved × 10 000 ÷ income in basis points, rounded half away from zero;
///   `None` when income ≤ 0.
pub fn cash_flow(
    data: &ReportData,
    viewer: &Viewer,
    from: &str,
    to: &str,
) -> Result<CashFlow, ReportError> {
    let range = DateRange::parse(from, to)?;
    let base = data.base;
    let tree = CategoryTree::new(&data.categories);
    let flows = flows(data, &tree, viewer, &range)?;
    let months = range.months();

    let zero = Money::zero(base);
    let mut sums = vec![(zero, zero, zero); months.len()];
    let index = |m| months.binary_search(&m).unwrap_or(0);
    for f in &flows.categories {
        let s = &mut sums[index(f.month)];
        match f.kind {
            CategoryKind::Income => s.0 = s.0.checked_add(f.actual)?,
            CategoryKind::Expense => s.1 = s.1.checked_add(f.actual)?,
        }
    }
    for f in &flows.saved {
        let s = &mut sums[index(f.month)];
        s.2 = s.2.checked_add(f.amount)?;
    }

    let mut total = (zero, zero, zero);
    let mut lines = Vec::with_capacity(months.len());
    for (m, (income, expense, saved)) in months.iter().zip(sums) {
        total = (
            total.0.checked_add(income)?,
            total.1.checked_add(expense)?,
            total.2.checked_add(saved)?,
        );
        lines.push(CashFlowMonth {
            month: m.to_string(),
            flow: line(income, expense, saved)?,
        });
    }

    Ok(CashFlow {
        months: lines,
        total: line(total.0, total.1, total.2)?,
        excluded: excluded_list(flows.left_out_entries(|_| true)),
    })
}

fn line(income: Money, expense: Money, saved: Money) -> Result<CashFlowLine, ReportError> {
    Ok(CashFlowLine {
        income,
        expense,
        saved,
        free_cash_flow: income.checked_sub(expense)?.checked_sub(saved)?,
        savings_rate_bp: savings_rate_bp(saved, income)?,
    })
}

/// saved × 10 000 ÷ income, rounded half away from zero; `None` when income ≤ 0.
fn savings_rate_bp(saved: Money, income: Money) -> Result<Option<i64>, ReportError> {
    if income.minor <= 0 {
        return Ok(None);
    }
    let n = i128::from(saved.minor) * 10_000;
    let d = i128::from(income.minor);
    let (q, r) = (n / d, n % d);
    let rounded = if 2 * r.abs() >= d { q + n.signum() } else { q };
    i64::try_from(rounded)
        .map(Some)
        .map_err(|_| MoneyError::Overflow.into())
}
