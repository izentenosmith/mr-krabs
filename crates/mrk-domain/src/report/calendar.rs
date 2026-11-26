//! The month calendar: category rows × days, like the spreadsheet's day grid.

use std::collections::BTreeMap;

use mrk_money::Money;

use super::flows::{Counts, flows};
use super::period::{DateRange, Month};
use super::{
    CalendarRow, CategoryTree, MonthCalendar, ReportData, ReportError, Viewer, add_to,
    excluded_list, sum_money, sum_roots,
};
use crate::category::CategoryKind;

/// The calendar of `month` (`YYYY-MM`, year 1970–9999, else `InvalidMonth`) for `viewer`:
/// one row per category in tree order with its actual per day (rolled up through the tree)
/// and its month total, plus income and expense per day and for the month.
pub fn month_calendar(
    data: &ReportData,
    viewer: &Viewer,
    month: &str,
) -> Result<MonthCalendar, ReportError> {
    let m = Month::parse(month)?;
    let base = data.base;
    let tree = CategoryTree::new(&data.categories);
    let flows = flows(data, &tree, viewer, &DateRange::of_months(m, m))?;
    let days = m.days() as usize;

    let mut own = vec![BTreeMap::new(); days];
    for f in &flows.categories {
        add_to(&mut own[(f.day - 1) as usize], f.category, f.actual)?;
    }
    let by_day = own
        .iter()
        .map(|o| tree.rollup(o, base))
        .collect::<Result<Vec<_>, _>>()?;

    let rows = tree
        .rows()
        .iter()
        .map(|row| {
            let days: Vec<Money> = by_day.iter().map(|d| d[&row.id]).collect();
            let total = sum_money(&days, base)?;
            Ok(CalendarRow {
                category: row.clone(),
                days,
                total,
            })
        })
        .collect::<Result<Vec<_>, ReportError>>()?;
    let per_day = |kind| {
        by_day
            .iter()
            .map(|d| sum_roots(&tree, d, kind, base))
            .collect::<Result<Vec<_>, _>>()
    };
    let income_by_day = per_day(CategoryKind::Income)?;
    let expense_by_day = per_day(CategoryKind::Expense)?;

    Ok(MonthCalendar {
        month: m.to_string(),
        days_in_month: m.days(),
        income_total: sum_money(&income_by_day, base)?,
        expense_total: sum_money(&expense_by_day, base)?,
        rows,
        income_by_day,
        expense_by_day,
        excluded: excluded_list(flows.left_out_entries(|c| matches!(c, Counts::Category(_)))),
    })
}
