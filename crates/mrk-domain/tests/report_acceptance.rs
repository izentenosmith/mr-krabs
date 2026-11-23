//! Acceptance tests for M2 slice R0 (report domain), written from `.workflow/brief.md`
//! through `mrk_domain`'s public API only. One test per acceptance criterion, in the brief's
//! order; the REQ-33 invariants are property tests.

use std::collections::BTreeMap;

use mrk_domain::category::CategoryKind;
use mrk_domain::{
    AccountKind, BudgetMatrix, CategoryTree, Cell, Excluded, Leg, MatrixRow, ReportAccount,
    ReportBudgetLine, ReportCategory, ReportData, ReportError, ReportPosting, Scope, Spending,
    View, Viewer, budget_matrix, cash_flow, month_calendar, spending,
};
use mrk_money::{Currency, CurrencyCode, Money, MoneyError};
use proptest::prelude::*;

const CLP: CurrencyCode = Currency::CLP.code;
const USD: CurrencyCode = Currency::USD.code;
const EUR: CurrencyCode = Currency::EUR.code;
const ME: &str = "me";

fn clp(v: i64) -> Money {
    Money::new(v, CLP)
}

fn usd(v: i64) -> Money {
    Money::new(v, USD)
}

fn cat(
    id: &str,
    parent: Option<&str>,
    kind: CategoryKind,
    sort: i64,
    name: &str,
) -> ReportCategory {
    ReportCategory {
        id: id.into(),
        parent_id: parent.map(Into::into),
        kind,
        archived: false,
        sort_order: sort,
        name: name.into(),
    }
}

/// salary (income) · food > {super > organic, delivery} · housing.
fn categories() -> Vec<ReportCategory> {
    use CategoryKind::{Expense, Income};
    vec![
        // Deliberately out of display order: the tree sorts them.
        cat("housing", None, Expense, 2, "Vivienda"),
        cat("delivery", Some("food"), Expense, 2, "Delivery"),
        cat("organic", Some("super"), Expense, 1, "Orgánico"),
        cat("super", Some("food"), Expense, 1, "Supermercado"),
        cat("food", None, Expense, 1, "Comida"),
        cat("salary", None, Income, 0, "Sueldo"),
    ]
}

fn personal(owner: &str) -> Scope {
    Scope::Personal {
        owner: owner.into(),
    }
}

fn accounts() -> Vec<ReportAccount> {
    let a = |id: &str, kind, scope| ReportAccount {
        id: id.into(),
        kind,
        scope,
    };
    vec![
        a("chk", AccountKind::Checking, Scope::Household),
        a("sav", AccountKind::Savings, Scope::Household),
        a("inv", AccountKind::Investment, Scope::Household),
        a("card", AccountKind::CreditCard, Scope::Household),
        a("mychk", AccountKind::Checking, personal(ME)),
        a("mysav", AccountKind::Savings, personal(ME)),
        a("other", AccountKind::Checking, personal("other")),
    ]
}

fn viewer(view: View) -> Viewer {
    Viewer {
        view,
        me: ME.into(),
    }
}

/// Builds `ReportData` the way the store would load it.
struct Book {
    data: ReportData,
    next: usize,
}

fn book() -> Book {
    Book {
        data: ReportData {
            base: CLP,
            categories: categories(),
            accounts: accounts(),
            postings: Vec::new(),
            budget_lines: Vec::new(),
        },
        next: 0,
    }
}

impl Book {
    fn entry(&mut self, date: &str, opening_balance: bool, legs: Vec<(Leg, Money)>) -> &mut Self {
        self.next += 1;
        let entry_id = format!("e{}", self.next);
        for (leg, amount) in legs {
            self.data.postings.push(ReportPosting {
                entry_id: entry_id.clone(),
                entry_date: date.into(),
                opening_balance,
                leg,
                amount,
            });
        }
        self
    }
    fn neg(m: Money) -> Money {
        Money::new(-m.minor, m.currency)
    }
    fn expense(&mut self, date: &str, account: &str, category: &str, m: Money) -> &mut Self {
        let legs = vec![
            (Leg::Account(account.into()), Self::neg(m)),
            (Leg::Category(category.into()), m),
        ];
        self.entry(date, false, legs)
    }
    /// A refund of an expense, or a claw-back of income: money back into the account
    /// reduces the category on the expense side, out of the account reduces income.
    fn refund(&mut self, date: &str, account: &str, category: &str, m: Money) -> &mut Self {
        let legs = vec![
            (Leg::Account(account.into()), m),
            (Leg::Category(category.into()), Self::neg(m)),
        ];
        self.entry(date, false, legs)
    }
    fn income(&mut self, date: &str, account: &str, category: &str, m: Money) -> &mut Self {
        self.refund(date, account, category, m)
    }
    fn claw_back(&mut self, date: &str, account: &str, category: &str, m: Money) -> &mut Self {
        self.expense(date, account, category, m)
    }
    fn transfer(&mut self, date: &str, from: &str, to: &str, m: Money) -> &mut Self {
        let legs = vec![
            (Leg::Account(from.into()), Self::neg(m)),
            (Leg::Account(to.into()), m),
        ];
        self.entry(date, false, legs)
    }
    fn opening(&mut self, date: &str, account: &str, m: Money) -> &mut Self {
        let legs = vec![
            (Leg::Account(account.into()), m),
            (Leg::FxTrading, Self::neg(m)),
        ];
        self.entry(date, true, legs)
    }
    fn budget(&mut self, period: &str, category: &str, scope: Scope, m: Money) -> &mut Self {
        self.data.budget_lines.push(ReportBudgetLine {
            period: period.into(),
            category_id: category.into(),
            scope,
            amount: m,
        });
        self
    }
}

fn row<'a>(m: &'a BudgetMatrix, id: &str) -> &'a MatrixRow {
    m.rows
        .iter()
        .find(|r| r.category.id == id)
        .unwrap_or_else(|| panic!("no row {id}"))
}

fn cell(planned: i64, actual: i64) -> Cell {
    Cell {
        planned: clp(planned),
        actual: clp(actual),
        diff: clp(planned - actual),
    }
}

// ───────────────────────── generators ─────────────────────────

const ACCOUNTS: [&str; 6] = ["chk", "sav", "inv", "card", "mychk", "mysav"];
const EXPENSE_CATS: [&str; 5] = ["food", "super", "organic", "delivery", "housing"];
const ALL_CATS: [&str; 6] = ["salary", "food", "super", "organic", "delivery", "housing"];

#[derive(Clone, Debug)]
enum Op {
    Expense(usize, usize, u32, u32, i64),
    Refund(usize, usize, u32, u32, i64),
    Income(usize, u32, u32, i64),
    ClawBack(usize, u32, u32, i64),
    Transfer(usize, usize, u32, u32, i64),
    Opening(usize, u32, i64),
    Budget(usize, u32, bool, i64),
}

fn date(month: u32, day: u32) -> String {
    format!("2026-{month:02}-{day:02}")
}

fn op() -> impl Strategy<Value = Op> {
    let acc = 0..ACCOUNTS.len();
    let month = 1u32..=12;
    let day = 1u32..=28;
    let amount = 1i64..1_000_000;
    prop_oneof![
        (
            acc.clone(),
            0..EXPENSE_CATS.len(),
            month.clone(),
            day.clone(),
            amount.clone()
        )
            .prop_map(|(a, c, m, d, v)| Op::Expense(a, c, m, d, v)),
        (
            acc.clone(),
            0..EXPENSE_CATS.len(),
            month.clone(),
            day.clone(),
            amount.clone()
        )
            .prop_map(|(a, c, m, d, v)| Op::Refund(a, c, m, d, v)),
        (acc.clone(), month.clone(), day.clone(), amount.clone())
            .prop_map(|(a, m, d, v)| Op::Income(a, m, d, v)),
        (acc.clone(), month.clone(), day.clone(), amount.clone())
            .prop_map(|(a, m, d, v)| Op::ClawBack(a, m, d, v)),
        transfer_op(),
        (acc, month.clone(), amount.clone()).prop_map(|(a, m, v)| Op::Opening(a, m, v)),
        (0..ALL_CATS.len(), month, any::<bool>(), 0i64..1_000_000)
            .prop_map(|(c, m, p, v)| Op::Budget(c, m, p, v)),
    ]
}

fn transfer_op() -> impl Strategy<Value = Op> {
    (
        0..ACCOUNTS.len(),
        0..ACCOUNTS.len(),
        1u32..=12,
        1u32..=28,
        1i64..1_000_000,
    )
        .prop_filter("two accounts", |(f, t, ..)| f != t)
        .prop_map(|(f, t, m, d, v)| Op::Transfer(f, t, m, d, v))
}

fn apply(b: &mut Book, ops: &[Op], money: fn(i64) -> Money) {
    for op in ops {
        match *op {
            Op::Expense(a, c, m, d, v) => {
                b.expense(&date(m, d), ACCOUNTS[a], EXPENSE_CATS[c], money(v))
            }
            Op::Refund(a, c, m, d, v) => {
                b.refund(&date(m, d), ACCOUNTS[a], EXPENSE_CATS[c], money(v))
            }
            Op::Income(a, m, d, v) => b.income(&date(m, d), ACCOUNTS[a], "salary", money(v)),
            Op::ClawBack(a, m, d, v) => b.claw_back(&date(m, d), ACCOUNTS[a], "salary", money(v)),
            Op::Transfer(f, t, m, d, v) => {
                b.transfer(&date(m, d), ACCOUNTS[f], ACCOUNTS[t], money(v))
            }
            Op::Opening(a, m, v) => b.opening(&date(m, 1), ACCOUNTS[a], money(v)),
            Op::Budget(c, m, p, v) => {
                let scope = if p { personal(ME) } else { Scope::Household };
                b.budget(&format!("2026-{m:02}"), ALL_CATS[c], scope, money(v))
            }
        };
    }
}

fn data_from(ops: &[Op]) -> ReportData {
    let mut b = book();
    apply(&mut b, ops, clp);
    b.data
}

// ───────────────────────── criteria ─────────────────────────

/// Criterion 1 (tracer bullet).
#[test]
fn tracer_leaf_cell_and_its_groups_show_planned_actual_and_diff() {
    let mut b = book();
    b.expense("2026-03-10", "chk", "organic", clp(45_990))
        .budget("2026-03", "organic", Scope::Household, clp(100_000));
    let m = budget_matrix(&b.data, &viewer(View::Household), 2026).unwrap();
    assert_eq!(m.year, 2026);
    for id in ["organic", "super", "food"] {
        let r = row(&m, id);
        assert_eq!(r.months.len(), 12);
        assert_eq!(r.months[2], cell(100_000, 45_990), "{id} in March");
        assert_eq!(r.months[1], cell(0, 0), "{id} in February");
        assert_eq!(r.year_total, cell(100_000, 45_990), "{id} for the year");
    }
    assert_eq!(row(&m, "delivery").months[2], cell(0, 0));
}

/// Criterion 2: tree order, depth, groups; roll-up = own + all descendants.
#[test]
fn tree_rows_are_in_display_order_with_depth_and_groups() {
    let tree = CategoryTree::new(&categories());
    let shape: Vec<(&str, u32, bool)> = tree
        .rows()
        .iter()
        .map(|r| (r.id.as_str(), r.depth, r.is_group))
        .collect();
    assert_eq!(
        shape,
        [
            ("salary", 0, false),
            ("food", 0, true),
            ("super", 1, true),
            ("organic", 2, false),
            ("delivery", 1, false),
            ("housing", 0, false),
        ]
    );
    let super_row = &tree.rows()[2];
    assert_eq!(super_row.parent_id.as_deref(), Some("food"));
    assert_eq!(super_row.kind, CategoryKind::Expense);
    assert_eq!(super_row.name, "Supermercado");

    // Same sort order: the (accent-insensitive) name decides.
    let ties = vec![
        cat("b", None, CategoryKind::Expense, 0, "Órgano"),
        cat("a", None, CategoryKind::Expense, 0, "Zapatos"),
        cat("c", None, CategoryKind::Expense, 0, "auto"),
    ];
    let tied = CategoryTree::new(&ties);
    let order: Vec<&str> = tied.rows().iter().map(|r| r.id.as_str()).collect();
    assert_eq!(order, ["c", "b", "a"]);
}

proptest! {
    /// Criterion 2 (REQ-33): every node equals its own amount plus its children's roll-ups,
    /// at any depth, and the roots add up to every own amount.
    #[test]
    fn rollup_node_equals_own_plus_descendants(
        nodes in proptest::collection::vec((any::<prop::sample::Index>(), any::<bool>(), -1_000_000i64..1_000_000), 1..16)
    ) {
        let mut cats = Vec::new();
        let mut own = BTreeMap::new();
        for (i, (parent, root, amount)) in nodes.iter().enumerate() {
            let parent_id = (i > 0 && !root).then(|| format!("n{}", parent.index(i)));
            cats.push(cat(&format!("n{i}"), parent_id.as_deref(), CategoryKind::Expense, 0, &format!("n{i}")));
            own.insert(format!("n{i}"), clp(*amount));
        }
        let tree = CategoryTree::new(&cats);
        prop_assert_eq!(tree.rows().len(), cats.len());
        let rolled = tree.rollup(&own, CLP).unwrap();
        for c in &cats {
            let children: i64 = cats
                .iter()
                .filter(|k| k.parent_id.as_deref() == Some(c.id.as_str()))
                .map(|k| rolled[&k.id].minor)
                .sum();
            prop_assert_eq!(rolled[&c.id].minor, own[&c.id].minor + children);
        }
        let roots: i64 = cats.iter().filter(|c| c.parent_id.is_none()).map(|c| rolled[&c.id].minor).sum();
        let all: i64 = own.values().map(|m| m.minor).sum();
        prop_assert_eq!(roots, all);
    }
}

/// Criterion 3: actuals are net of refunds and claw-backs; splits go leg by leg.
#[test]
fn category_actuals_are_net_of_refunds_and_claw_backs() {
    let mut b = book();
    b.expense("2026-05-02", "chk", "super", clp(10_000))
        .refund("2026-05-03", "chk", "super", clp(3_000))
        .income("2026-05-01", "chk", "salary", clp(1_000_000))
        .claw_back("2026-05-20", "chk", "salary", clp(50_000))
        .entry(
            "2026-05-04",
            false,
            vec![
                (Leg::Account("chk".into()), clp(-15_000)),
                (Leg::Category("super".into()), clp(10_000)),
                (Leg::Category("delivery".into()), clp(5_000)),
            ],
        );
    let m = budget_matrix(&b.data, &viewer(View::Household), 2026).unwrap();
    assert_eq!(row(&m, "super").months[4].actual, clp(17_000));
    assert_eq!(row(&m, "delivery").months[4].actual, clp(5_000));
    assert_eq!(row(&m, "food").months[4].actual, clp(22_000));
    assert_eq!(row(&m, "salary").months[4].actual, clp(950_000));
    assert_eq!(m.months[4].month, "2026-05");
    assert_eq!(m.months[4].income.actual, clp(950_000));
    assert_eq!(m.months[4].expense.actual, clp(22_000));
}

proptest! {
    /// Criterion 4 (REQ-33): transfers never touch income, expense or any category row.
    #[test]
    fn transfers_never_touch_income_expense_or_categories(
        ops in proptest::collection::vec(op(), 0..30),
        transfers in proptest::collection::vec(transfer_op(), 1..10),
    ) {
        let before = budget_matrix(&data_from(&ops), &viewer(View::Combined), 2026).unwrap();
        let mut b = book();
        apply(&mut b, &ops, clp);
        apply(&mut b, &transfers, clp);
        let after = budget_matrix(&b.data, &viewer(View::Combined), 2026).unwrap();
        prop_assert_eq!(&before.rows, &after.rows);
        for (x, y) in before.months.iter().zip(&after.months) {
            prop_assert_eq!(x.income, y.income);
            prop_assert_eq!(x.expense, y.expense);
        }
    }
}

/// Criterion 5: saved = net transfers into savings/investment; opening balances never count.
#[test]
fn saved_counts_net_transfers_into_savings_and_never_opening_balances() {
    let mut b = book();
    b.transfer("2026-03-01", "chk", "sav", clp(100_000))
        .transfer("2026-03-02", "sav", "chk", clp(30_000))
        .transfer("2026-03-03", "sav", "inv", clp(50_000))
        .transfer("2026-03-04", "chk", "card", clp(200_000))
        .transfer("2026-03-05", "chk", "inv", clp(5_000))
        .opening("2026-03-01", "sav", clp(1_000_000))
        .opening("2026-03-01", "chk", clp(2_000_000))
        .opening("2026-03-01", "card", clp(-300_000))
        .opening("2026-03-01", "sav", usd(10_000));
    let v = viewer(View::Household);
    let m = budget_matrix(&b.data, &v, 2026).unwrap();
    assert_eq!(m.months[2].saved, clp(75_000));
    assert_eq!(m.months[2].income.actual, clp(0));
    assert_eq!(m.months[2].expense.actual, clp(0));
    assert!(m.rows.iter().all(|r| r.year_total == cell(0, 0)));
    assert_eq!(m.excluded, []);
    let cf = cash_flow(&b.data, &v, "2026-03-01", "2026-03-31").unwrap();
    assert_eq!(cf.total.saved, clp(75_000));
    assert_eq!(cf.excluded, []);
}

proptest! {
    /// Criterion 6 (REQ-33): diff = planned − actual everywhere; free = income − expense − saved.
    #[test]
    fn free_cash_flow_and_diff_identities(ops in proptest::collection::vec(op(), 0..40)) {
        let data = data_from(&ops);
        for view in [View::Household, View::Personal, View::Combined] {
            let m = budget_matrix(&data, &viewer(view), 2026).unwrap();
            let diff_ok = |c: &Cell| c.diff.minor == c.planned.minor - c.actual.minor;
            for r in &m.rows {
                prop_assert!(r.months.iter().all(diff_ok));
                prop_assert!(diff_ok(&r.year_total));
            }
            prop_assert_eq!(m.months.len(), 12);
            for mm in &m.months {
                prop_assert!(diff_ok(&mm.income) && diff_ok(&mm.expense));
                prop_assert_eq!(
                    mm.free_cash_flow.minor,
                    mm.income.actual.minor - mm.expense.actual.minor - mm.saved.minor
                );
            }
        }
    }
}

/// Criterion 7: household = household accounts/lines, personal = mine, combined = all.
#[test]
fn view_filter_selects_accounts_and_lines_by_scope() {
    let mut b = book();
    b.expense("2026-01-05", "chk", "super", clp(1_000))
        .expense("2026-01-05", "mychk", "super", clp(200))
        .expense("2026-01-05", "other", "super", clp(30))
        .budget("2026-01", "super", Scope::Household, clp(5_000))
        .budget("2026-01", "super", personal(ME), clp(600))
        .budget("2026-01", "super", personal("other"), clp(70))
        .transfer("2026-01-06", "chk", "mysav", clp(4_000));
    let jan = |view| {
        let m = budget_matrix(&b.data, &viewer(view), 2026).unwrap();
        (row(&m, "super").months[0], m.months[0].saved)
    };
    assert_eq!(jan(View::Household), (cell(5_000, 1_000), clp(0)));
    assert_eq!(jan(View::Personal), (cell(600, 200), clp(4_000)));
    assert_eq!(jan(View::Combined), (cell(5_670, 1_230), clp(4_000)));
}

proptest! {
    /// Criterion 7 (REQ-33): with one member, household + personal = combined, every amount.
    #[test]
    fn household_plus_personal_is_combined(ops in proptest::collection::vec(op(), 0..40)) {
        let data = data_from(&ops);
        let get = |view| budget_matrix(&data, &viewer(view), 2026).unwrap();
        let (h, p, c) = (get(View::Household), get(View::Personal), get(View::Combined));
        let add = |x: &Cell, y: &Cell| (x.planned.minor + y.planned.minor, x.actual.minor + y.actual.minor);
        let parts = |c: &Cell| (c.planned.minor, c.actual.minor);
        for ((rh, rp), rc) in h.rows.iter().zip(&p.rows).zip(&c.rows) {
            for i in 0..12 {
                prop_assert_eq!(add(&rh.months[i], &rp.months[i]), parts(&rc.months[i]));
            }
        }
        for ((mh, mp), mc) in h.months.iter().zip(&p.months).zip(&c.months) {
            prop_assert_eq!(add(&mh.income, &mp.income), parts(&mc.income));
            prop_assert_eq!(add(&mh.expense, &mp.expense), parts(&mc.expense));
            prop_assert_eq!(mh.saved.minor + mp.saved.minor, mc.saved.minor);
        }
    }
}

/// Criterion 8: other currencies are left out and counted once per entry and currency.
#[test]
fn non_base_currency_is_excluded_and_counted() {
    let mut b = book();
    b.expense("2026-02-01", "chk", "super", clp(1_000))
        .expense("2026-02-02", "chk", "super", usd(2_500))
        .transfer("2026-02-03", "chk", "sav", usd(10_000))
        .expense("2026-02-04", "chk", "delivery", Money::new(700, EUR))
        // A USD purchase paid from CLP through FX legs: the category leg is USD.
        .entry(
            "2026-02-05",
            false,
            vec![
                (Leg::Account("chk".into()), clp(-23_750)),
                (Leg::FxTrading, clp(23_750)),
                (Leg::FxTrading, usd(-2_500)),
                (Leg::Category("super".into()), usd(2_500)),
            ],
        )
        // A CLP → USD card payment contributes nothing, so nothing is left out.
        .entry(
            "2026-02-06",
            false,
            vec![
                (Leg::Account("chk".into()), clp(-23_750)),
                (Leg::FxTrading, clp(23_750)),
                (Leg::FxTrading, usd(-2_500)),
                (Leg::Account("card".into()), usd(2_500)),
            ],
        )
        .budget("2026-02", "super", Scope::Household, usd(9_999))
        // Outside the year: not counted in this report.
        .expense("2025-12-31", "chk", "super", usd(1));
    let m = budget_matrix(&b.data, &viewer(View::Household), 2026).unwrap();
    assert_eq!(row(&m, "super").months[1], cell(0, 1_000));
    assert_eq!(m.months[1].saved, clp(0));
    assert_eq!(
        m.excluded,
        [
            Excluded {
                currency: EUR,
                count: 1
            },
            Excluded {
                currency: USD,
                count: 4
            },
        ]
    );
}

proptest! {
    /// Criterion 8 (REQ-33): adding non-base entries changes nothing but `excluded`.
    #[test]
    fn non_base_postings_change_only_excluded(
        ops in proptest::collection::vec(op(), 0..30),
        foreign in proptest::collection::vec(op(), 1..15),
    ) {
        let v = viewer(View::Combined);
        let before = budget_matrix(&data_from(&ops), &v, 2026).unwrap();
        let mut b = book();
        apply(&mut b, &ops, clp);
        apply(&mut b, &foreign, usd);
        let after = budget_matrix(&b.data, &v, 2026).unwrap();
        prop_assert_eq!(&before.rows, &after.rows);
        prop_assert_eq!(&before.months, &after.months);
        prop_assert!(after.excluded.iter().all(|e| e.currency == USD && e.count >= 1));
        let cf_before = cash_flow(&data_from(&ops), &v, "2026-01-01", "2026-12-31").unwrap();
        let cf_after = cash_flow(&b.data, &v, "2026-01-01", "2026-12-31").unwrap();
        prop_assert_eq!(cf_before.months, cf_after.months);
    }
}

/// Criterion 9: each leg on its day, rolled up; day totals per kind.
#[test]
fn calendar_places_each_leg_on_its_day_rolled_up() {
    let mut b = book();
    b.expense("2026-03-03", "chk", "organic", clp(1_000))
        .expense("2026-03-03", "chk", "delivery", clp(500))
        .expense("2026-03-31", "chk", "super", clp(200))
        .income("2026-03-01", "chk", "salary", clp(1_000_000))
        .expense("2026-04-01", "chk", "super", clp(9));
    let cal = month_calendar(&b.data, &viewer(View::Household), "2026-03").unwrap();
    assert_eq!(cal.month, "2026-03");
    assert_eq!(cal.days_in_month, 31);
    let r = |id: &str| cal.rows.iter().find(|r| r.category.id == id).unwrap();
    assert_eq!(r("organic").days[2], clp(1_000));
    assert_eq!(r("super").days[2], clp(1_000));
    assert_eq!(r("super").days[30], clp(200));
    assert_eq!(r("food").days[2], clp(1_500));
    assert_eq!(r("food").total, clp(1_700));
    assert_eq!(r("salary").days[0], clp(1_000_000));
    assert!(cal.rows.iter().all(|r| r.days.len() == 31));
    assert_eq!(cal.expense_by_day[2], clp(1_500));
    assert_eq!(cal.expense_by_day[30], clp(200));
    assert_eq!(cal.income_by_day[0], clp(1_000_000));
    assert_eq!(cal.expense_total, clp(1_700));
    assert_eq!(cal.income_total, clp(1_000_000));

    let days = |month| {
        month_calendar(&b.data, &viewer(View::Household), month)
            .unwrap()
            .days_in_month
    };
    assert_eq!(days("2026-02"), 28);
    assert_eq!(days("2024-02"), 29);
    assert_eq!(days("2000-02"), 29);
    assert_eq!(days("2100-02"), 28);
    assert_eq!(days("2026-04"), 30);
}

proptest! {
    /// Criterion 9: row totals = Σ days; day totals per kind = Σ root rows that day.
    #[test]
    fn calendar_totals_add_up(ops in proptest::collection::vec(op(), 0..40), month in 1u32..=12) {
        let cal = month_calendar(&data_from(&ops), &viewer(View::Combined), &format!("2026-{month:02}")).unwrap();
        for r in &cal.rows {
            prop_assert_eq!(r.total.minor, r.days.iter().map(|d| d.minor).sum::<i64>());
        }
        let n = cal.days_in_month as usize;
        for d in 0..n {
            let root = |kind| cal.rows.iter()
                .filter(|r| r.category.depth == 0 && r.category.kind == kind)
                .map(|r| r.days[d].minor).sum::<i64>();
            prop_assert_eq!(cal.expense_by_day[d].minor, root(CategoryKind::Expense));
            prop_assert_eq!(cal.income_by_day[d].minor, root(CategoryKind::Income));
        }
        prop_assert_eq!(cal.expense_total.minor, cal.expense_by_day.iter().map(|d| d.minor).sum::<i64>());
        prop_assert_eq!(cal.income_total.minor, cal.income_by_day.iter().map(|d| d.minor).sum::<i64>());
    }
}

/// Criterion 10: one line per month touched, dates outside left out, rounded savings rate.
#[test]
fn cash_flow_lines_cover_the_range_months_with_rounded_savings_rate() {
    let mut b = book();
    b.income("2026-01-14", "chk", "salary", clp(7))
        .income("2026-01-15", "chk", "salary", clp(1_000_000))
        .transfer("2026-01-20", "chk", "sav", clp(123_450))
        .income("2026-02-01", "chk", "salary", clp(1_000_000))
        .transfer("2026-02-20", "sav", "chk", clp(123_450))
        .income("2026-03-10", "chk", "salary", clp(1_000_000))
        .transfer("2026-03-10", "chk", "sav", clp(123_456))
        .expense("2026-03-10", "chk", "super", clp(100_000))
        .expense("2026-03-11", "chk", "super", clp(5));
    let cf = cash_flow(
        &b.data,
        &viewer(View::Household),
        "2026-01-15",
        "2026-03-10",
    )
    .unwrap();
    let months: Vec<&str> = cf.months.iter().map(|m| m.month.as_str()).collect();
    assert_eq!(months, ["2026-01", "2026-02", "2026-03"]);
    let f = |i: usize| cf.months[i].flow;
    assert_eq!(
        (f(0).income, f(0).saved, f(0).savings_rate_bp),
        (clp(1_000_000), clp(123_450), Some(1235))
    );
    assert_eq!(
        (f(1).saved, f(1).savings_rate_bp),
        (clp(-123_450), Some(-1235))
    );
    assert_eq!(f(2).expense, clp(100_000));
    assert_eq!(f(2).free_cash_flow, clp(1_000_000 - 100_000 - 123_456));
    assert_eq!(f(2).savings_rate_bp, Some(1235));
    assert_eq!(cf.total.income, clp(3_000_000));
    assert_eq!(cf.total.expense, clp(100_000));
    assert_eq!(cf.total.saved, clp(123_456));
    assert_eq!(cf.total.free_cash_flow, clp(3_000_000 - 100_000 - 123_456));
    // 123 456 × 10 000 ÷ 3 000 000 = 411.52 → 412.
    assert_eq!(cf.total.savings_rate_bp, Some(412));
}

/// Criterion 11: spending rolls up; shares are exact.
#[test]
fn spending_rolls_up_and_shares_sum_to_10000() {
    let mut b = book();
    b.expense("2026-06-01", "chk", "super", clp(6_000))
        .expense("2026-06-02", "chk", "organic", clp(1_000))
        .expense("2026-06-03", "chk", "delivery", clp(2_000))
        .expense("2026-06-04", "chk", "housing", clp(1_000))
        .income("2026-06-05", "chk", "salary", clp(9_999_999));
    let s = spending(
        &b.data,
        &viewer(View::Household),
        "2026-06-01",
        "2026-06-30",
    )
    .unwrap();
    let got: Vec<(&str, i64, Option<u32>)> = s
        .rows
        .iter()
        .map(|r| (r.category.id.as_str(), r.amount.minor, r.share_bp))
        .collect();
    assert_eq!(
        got,
        [
            ("food", 9_000, Some(9_000)),
            ("super", 7_000, Some(7_000)),
            ("organic", 1_000, Some(1_000)),
            ("delivery", 2_000, Some(2_000)),
            ("housing", 1_000, Some(1_000)),
        ]
    );
    assert_eq!(s.total, clp(10_000));
}

fn check_shares(s: &Spending) -> Result<(), TestCaseError> {
    if s.total.minor <= 0 {
        prop_assert!(s.rows.iter().all(|r| r.share_bp.is_none()));
        return Ok(());
    }
    let share = |r: &mrk_domain::SpendingRow| r.share_bp.unwrap();
    let roots: u32 = s
        .rows
        .iter()
        .filter(|r| r.category.depth == 0)
        .map(share)
        .sum();
    prop_assert_eq!(roots, 10_000);
    for g in &s.rows {
        let kids: u32 = s
            .rows
            .iter()
            .filter(|r| r.category.parent_id.as_deref() == Some(g.category.id.as_str()))
            .map(share)
            .sum();
        prop_assert!(kids <= share(g));
        if g.amount.minor <= 0 {
            prop_assert_eq!(share(g), 0);
        }
    }
    Ok(())
}

proptest! {
    /// Criterion 11 (REQ-33): roots sum to exactly 10 000; groups split exactly when they
    /// have no postings of their own.
    #[test]
    fn spending_shares_are_exact(ops in proptest::collection::vec(op(), 0..40)) {
        let data = data_from(&ops);
        let s = spending(&data, &viewer(View::Combined), "2026-01-01", "2026-12-31").unwrap();
        check_shares(&s)?;
        prop_assert!(s.rows.iter().all(|r| r.category.kind == CategoryKind::Expense));
        let roots: i64 = s.rows.iter().filter(|r| r.category.depth == 0).map(|r| r.amount.minor).sum();
        prop_assert_eq!(roots, s.total.minor);
    }

    /// Criterion 11: with postings on leaves only, a group's children split its share exactly.
    #[test]
    fn spending_groups_split_exactly_into_children(
        leaves in proptest::collection::vec((0usize..3, 1u32..=12, 1i64..1_000_000), 1..30)
    ) {
        let mut b = book();
        for (c, m, v) in leaves {
            b.expense(&date(m, 1), "chk", ["organic", "delivery", "housing"][c], clp(v));
        }
        let s = spending(&b.data, &viewer(View::Household), "2026-01-01", "2026-12-31").unwrap();
        check_shares(&s)?;
        for g in s.rows.iter().filter(|r| r.category.is_group) {
            let kids: u32 = s.rows.iter()
                .filter(|r| r.category.parent_id.as_deref() == Some(g.category.id.as_str()))
                .map(|r| r.share_bp.unwrap()).sum();
            prop_assert_eq!(kids, g.share_bp.unwrap());
        }
    }

    /// Criterion 12: the matrix's monthly summary equals cash flow over the same year.
    #[test]
    fn matrix_months_equal_cash_flow_for_the_year(ops in proptest::collection::vec(op(), 0..40)) {
        let data = data_from(&ops);
        for view in [View::Household, View::Personal, View::Combined] {
            let m = budget_matrix(&data, &viewer(view), 2026).unwrap();
            let cf = cash_flow(&data, &viewer(view), "2026-01-01", "2026-12-31").unwrap();
            prop_assert_eq!(cf.months.len(), 12);
            for (mm, c) in m.months.iter().zip(&cf.months) {
                prop_assert_eq!(&mm.month, &c.month);
                prop_assert_eq!(mm.income.actual, c.flow.income);
                prop_assert_eq!(mm.expense.actual, c.flow.expense);
                prop_assert_eq!(mm.saved, c.flow.saved);
                prop_assert_eq!(mm.free_cash_flow, c.flow.free_cash_flow);
            }
        }
    }
}

/// Criterion 13 (edge): categories in a parent cycle or under an unknown parent are left out.
#[test]
fn categories_in_a_cycle_or_under_unknown_parent_are_left_out() {
    let mut b = book();
    b.data.categories.extend([
        cat("a", Some("b"), CategoryKind::Expense, 0, "A"),
        cat("b", Some("a"), CategoryKind::Expense, 0, "B"),
        cat("x", Some("ghost"), CategoryKind::Expense, 0, "X"),
        cat("self", Some("self"), CategoryKind::Expense, 0, "Self"),
    ]);
    b.expense("2026-01-01", "chk", "a", clp(10))
        .expense("2026-01-01", "chk", "x", clp(20))
        .expense("2026-01-01", "chk", "self", clp(40))
        .expense("2026-01-01", "chk", "housing", clp(5));
    let v = viewer(View::Household);
    let m = budget_matrix(&b.data, &v, 2026).unwrap();
    let ids: Vec<&str> = m.rows.iter().map(|r| r.category.id.as_str()).collect();
    assert_eq!(
        ids,
        ["salary", "food", "super", "organic", "delivery", "housing"]
    );
    assert_eq!(m.months[0].expense.actual, clp(5));
    let cal = month_calendar(&b.data, &v, "2026-01").unwrap();
    assert_eq!(cal.expense_total, clp(5));
    let s = spending(&b.data, &v, "2026-01-01", "2026-01-31").unwrap();
    assert_eq!(s.total, clp(5));
}

/// Criterion 14 (edge): no rate without positive income, no shares without positive spending.
#[test]
fn rates_and_shares_are_none_without_a_positive_base() {
    let mut b = book();
    b.transfer("2026-01-02", "chk", "sav", clp(1_000))
        .claw_back("2026-02-02", "chk", "salary", clp(500))
        .transfer("2026-02-03", "chk", "sav", clp(1_000));
    let cf = cash_flow(
        &b.data,
        &viewer(View::Household),
        "2026-01-01",
        "2026-02-28",
    )
    .unwrap();
    assert_eq!(cf.months[0].flow.savings_rate_bp, None);
    assert_eq!(cf.months[1].flow.income, clp(-500));
    assert_eq!(cf.months[1].flow.savings_rate_bp, None);
    assert_eq!(cf.total.savings_rate_bp, None);

    let mut r = book();
    r.refund("2026-01-02", "chk", "super", clp(1_000));
    let s = spending(
        &r.data,
        &viewer(View::Household),
        "2026-01-01",
        "2026-01-31",
    )
    .unwrap();
    assert_eq!(s.total, clp(-1_000));
    assert!(s.rows.iter().all(|row| row.share_bp.is_none()));

    let mut n = book();
    n.refund("2026-01-02", "chk", "super", clp(1_000)).expense(
        "2026-01-03",
        "chk",
        "delivery",
        clp(5_000),
    );
    let s = spending(
        &n.data,
        &viewer(View::Household),
        "2026-01-01",
        "2026-01-31",
    )
    .unwrap();
    let share = |id: &str| {
        s.rows
            .iter()
            .find(|r| r.category.id == id)
            .unwrap()
            .share_bp
    };
    assert_eq!(share("super"), Some(0));
    assert_eq!(share("organic"), Some(0));
    assert_eq!(share("delivery"), Some(10_000));
    assert_eq!(share("food"), Some(10_000));
    assert_eq!(share("housing"), Some(0));
}

/// Criterion 15 (edge): empty data gives zeroed reports in the base currency.
#[test]
fn empty_data_gives_zeroed_reports() {
    let b = book();
    let v = viewer(View::Combined);
    let m = budget_matrix(&b.data, &v, 2026).unwrap();
    assert_eq!(m.rows.len(), 6);
    assert!(
        m.rows
            .iter()
            .all(|r| r.months.iter().all(|c| *c == cell(0, 0)))
    );
    let months: Vec<&str> = m.months.iter().map(|mm| mm.month.as_str()).collect();
    assert_eq!(months.first(), Some(&"2026-01"));
    assert_eq!(months.last(), Some(&"2026-12"));
    assert!(
        m.months
            .iter()
            .all(|mm| mm.saved == clp(0) && mm.free_cash_flow == clp(0))
    );
    assert_eq!(m.excluded, []);
    let cf = cash_flow(&b.data, &v, "2026-01-01", "2026-01-31").unwrap();
    assert_eq!(cf.total.income, clp(0));
    assert_eq!(cf.total.savings_rate_bp, None);
    let s = spending(&b.data, &v, "2026-01-01", "2026-01-31").unwrap();
    assert_eq!(s.total, clp(0));

    let nothing = ReportData {
        base: CLP,
        categories: vec![],
        accounts: vec![],
        postings: vec![],
        budget_lines: vec![],
    };
    let m = budget_matrix(&nothing, &v, 2026).unwrap();
    assert!(m.rows.is_empty());
    assert_eq!(m.months.len(), 12);
}

/// Criterion 16 (abuse): malformed periods are refused; overflow is an error, never a panic.
#[test]
fn malformed_inputs_are_refused_never_panic() {
    let b = book();
    let v = viewer(View::Household);
    for year in [1969, 10_000, 0, -1, i32::MIN, i32::MAX] {
        assert_eq!(
            budget_matrix(&b.data, &v, year),
            Err(ReportError::InvalidYear(year))
        );
    }
    assert!(budget_matrix(&b.data, &v, 1970).is_ok());
    assert!(budget_matrix(&b.data, &v, 9999).is_ok());
    for month in [
        "2026-13",
        "2026-00",
        "2026-3",
        "2026-03-01",
        "abcd-ef",
        "",
        "1969-12",
        "２０２６-03",
    ] {
        assert_eq!(
            month_calendar(&b.data, &v, month),
            Err(ReportError::InvalidMonth(month.into())),
            "{month}"
        );
    }
    assert!(month_calendar(&b.data, &v, "1970-01").is_ok());
    assert!(month_calendar(&b.data, &v, "9999-12").is_ok());
    assert_eq!(
        cash_flow(&b.data, &v, "2026-02-30", "2026-03-01"),
        Err(ReportError::InvalidDate("2026-02-30".into()))
    );
    assert_eq!(
        spending(&b.data, &v, "2026-01-01", "2026-1-31"),
        Err(ReportError::InvalidDate("2026-1-31".into()))
    );
    assert_eq!(
        cash_flow(&b.data, &v, "2026-03-02", "2026-03-01"),
        Err(ReportError::InvalidRange {
            from: "2026-03-02".into(),
            to: "2026-03-01".into()
        })
    );
    assert!(cash_flow(&b.data, &v, "2026-03-01", "2026-03-01").is_ok());

    let mut big = book();
    big.expense("2026-01-01", "chk", "organic", clp(i64::MAX))
        .expense("2026-01-02", "chk", "organic", clp(i64::MAX));
    let overflow = Err(ReportError::Money(MoneyError::Overflow));
    assert_eq!(budget_matrix(&big.data, &v, 2026).map(|_| ()), overflow);
    assert_eq!(
        month_calendar(&big.data, &v, "2026-01").map(|_| ()),
        overflow
    );
    assert_eq!(
        cash_flow(&big.data, &v, "2026-01-01", "2026-01-31").map(|_| ()),
        overflow
    );
    assert_eq!(
        spending(&big.data, &v, "2026-01-01", "2026-01-31").map(|_| ()),
        overflow
    );
}

/// Test-side oracle for `YYYY-MM` with year 1970–9999.
fn valid_month(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 7
        || b[4] != b'-'
        || !b
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || c.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = s[0..4].parse().unwrap();
    let month: u32 = s[5..7].parse().unwrap();
    (1970..=9999).contains(&year) && (1..=12).contains(&month)
}

proptest! {
    /// Criterion 16: any string is accepted or refused, never a panic.
    #[test]
    fn arbitrary_periods_never_panic(s in "\\PC{0,12}|[0-9]{4}-[0-9]{2}", t in "\\PC{0,12}|[0-9]{4}-[0-9]{2}-[0-9]{2}") {
        let b = book();
        let v = viewer(View::Combined);
        let cal = month_calendar(&b.data, &v, &s);
        prop_assert_eq!(cal.is_ok(), valid_month(&s));
        if cal.is_err() {
            prop_assert_eq!(cal, Err(ReportError::InvalidMonth(s.clone())));
        }
        let _ = cash_flow(&b.data, &v, &t, "2026-12-31");
        let _ = spending(&b.data, &v, "1970-01-01", &t);
    }
}
