//! Plan vs real for a year (FR-BU-08): category rows × months of planned, actual and diff.

use std::collections::BTreeMap;

use mrk_money::Money;

use super::flows::flows;
use super::period::{DateRange, Month, year_months};
use super::{
    BudgetMatrix, CategoryTree, Cell, MatrixMonth, MatrixRow, MatrixTotal, ReportData, ReportError,
    Viewer, add_to, excluded_list, sum_roots,
};
use crate::category::CategoryKind;

/// The year's matrix for `viewer`: one row per category in tree order with twelve monthly
/// cells (planned and actual rolled up through the tree, diff = planned − actual) and a year
/// total, plus the monthly income, expense, saved and free cash flow and their year total
/// (X-17; the "total expenses" line is `year_total.expense.actual` / each month's `expense.actual`). Actuals are attributed
/// to the entry date (DEC-29 Q6). Year 1970–9999, else `InvalidYear`.
pub fn budget_matrix(
    data: &ReportData,
    viewer: &Viewer,
    year: i32,
) -> Result<BudgetMatrix, ReportError> {
    let months = year_months(year)?;
    let base = data.base;
    let tree = CategoryTree::new(&data.categories);
    let range = DateRange::of_months(months[0], months[11]);
    let flows = flows(data, &tree, viewer, &range)?;

    let mut actual_own = vec![BTreeMap::new(); 12];
    for f in &flows.categories {
        add_to(&mut actual_own[month_index(f.month)], f.category, f.actual)?;
    }
    let mut saved = vec![Money::zero(base); 12];
    for f in &flows.saved {
        let i = month_index(f.month);
        saved[i] = saved[i].checked_add(f.amount)?;
    }
    let mut excluded = flows.left_out_entries(|_| true);
    let mut planned_own = vec![BTreeMap::new(); 12];
    for line in data.budget_lines.iter().filter(|l| viewer.sees(&l.scope)) {
        let Ok(m) = Month::parse(&line.period) else {
            continue;
        };
        if !months.contains(&m) || tree.get(&line.category_id).is_none() {
            continue;
        }
        if line.amount.currency == base {
            add_to(
                &mut planned_own[month_index(m)],
                &line.category_id,
                line.amount,
            )?;
        } else {
            let n = excluded.entry(line.amount.currency).or_default();
            *n = n.saturating_add(1);
        }
    }
    let rolled = |own: &[BTreeMap<String, Money>]| {
        own.iter()
            .map(|o| tree.rollup(o, base))
            .collect::<Result<Vec<_>, _>>()
    };
    let (planned, actual) = (rolled(&planned_own)?, rolled(&actual_own)?);

    let mut rows = Vec::with_capacity(tree.rows().len());
    for row in tree.rows() {
        let cells = (0..12)
            .map(|i| Cell::new(planned[i][&row.id], actual[i][&row.id]))
            .collect::<Result<Vec<_>, _>>()?;
        let mut year_total = Cell::zero(base);
        for c in &cells {
            year_total = year_total.checked_add(c)?;
        }
        rows.push(MatrixRow {
            category: row.clone(),
            months: cells,
            year_total,
        });
    }

    let mut summary = Vec::with_capacity(12);
    for (i, m) in months.iter().enumerate() {
        let total = |kind| -> Result<Cell, ReportError> {
            Cell::new(
                sum_roots(&tree, &planned[i], kind, base)?,
                sum_roots(&tree, &actual[i], kind, base)?,
            )
        };
        let (income, expense) = (total(CategoryKind::Income)?, total(CategoryKind::Expense)?);
        let saved = saved[i];
        let free_cash_flow = income
            .actual
            .checked_sub(expense.actual)?
            .checked_sub(saved)?;
        summary.push(MatrixMonth {
            month: m.to_string(),
            income,
            expense,
            saved,
            free_cash_flow,
        });
    }

    let zero = Money::zero(base);
    let mut year_total = MatrixTotal {
        income: Cell::zero(base),
        expense: Cell::zero(base),
        saved: zero,
        free_cash_flow: zero,
    };
    for m in &summary {
        year_total = MatrixTotal {
            income: year_total.income.checked_add(&m.income)?,
            expense: year_total.expense.checked_add(&m.expense)?,
            saved: year_total.saved.checked_add(m.saved)?,
            free_cash_flow: year_total.free_cash_flow.checked_add(m.free_cash_flow)?,
        };
    }

    Ok(BudgetMatrix {
        year,
        rows,
        months: summary,
        year_total,
        excluded: excluded_list(excluded),
    })
}

fn month_index(m: Month) -> usize {
    (m.month - 1) as usize
}
