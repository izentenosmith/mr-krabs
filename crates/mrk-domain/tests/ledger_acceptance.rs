//! Acceptance tests for M1 slice S2 (ledger domain), written from `.workflow/brief.md`
//! through `mrk_domain`'s public API only. One test per acceptance criterion.

use std::collections::BTreeMap;

use mrk_domain::{
    AccountInfo, BalanceRow, DomainError, EntryKind, JournalEntry, LedgerContext, Leg, LegField,
    LegStatus, NewTx, Posting, StoredLeg, TxType, account_balances, classify, ensure_unlocked,
    plan_update, validate_balanced,
};
use mrk_money::{Currency, CurrencyCode, Money, MoneyError};
use proptest::prelude::*;

const CLP: CurrencyCode = Currency::CLP.code;
const USD: CurrencyCode = Currency::USD.code;

fn clp(v: i64) -> Money {
    Money::new(v, CLP)
}

fn usd(v: i64) -> Money {
    Money::new(v, USD)
}

/// checking, savings (CLP), card-usd (USD), old (CLP, archived).
fn ctx() -> LedgerContext {
    let mut accounts = BTreeMap::new();
    let open = |currency| AccountInfo {
        currency,
        archived: false,
    };
    accounts.insert("checking".to_owned(), open(CLP));
    accounts.insert("savings".to_owned(), open(CLP));
    accounts.insert("card-usd".to_owned(), open(USD));
    accounts.insert(
        "old".to_owned(),
        AccountInfo {
            currency: CLP,
            archived: true,
        },
    );
    LedgerContext { accounts }
}

fn expense(account: &str, category: &str, amount: Money) -> NewTx {
    NewTx::Expense {
        account: account.into(),
        category: category.into(),
        amount,
    }
}

fn income(account: &str, category: &str, amount: Money) -> NewTx {
    NewTx::Income {
        account: account.into(),
        category: category.into(),
        amount,
    }
}

fn transfer(from: &str, to: &str, amount: Money) -> NewTx {
    NewTx::Transfer {
        from: from.into(),
        to: to.into(),
        amount,
    }
}

fn opening(account: &str, amount: Money) -> NewTx {
    NewTx::OpeningBalance {
        account: account.into(),
        amount,
        date: "2026-11-01".into(),
    }
}

/// Stores an entry the way the store would read it back: account leg first, given ids.
fn stored(entry: &JournalEntry, ids: [&str; 2], status: [Option<LegStatus>; 2]) -> Vec<StoredLeg> {
    entry
        .postings()
        .into_iter()
        .zip(ids)
        .zip(status)
        .map(|((posting, id), status)| StoredLeg {
            id: id.into(),
            posting,
            status,
        })
        .collect()
}

fn status_of(p: &Posting) -> Option<LegStatus> {
    matches!(p.leg, Leg::Account(_)).then_some(LegStatus::Cleared)
}

fn stored_cleared(entry: &JournalEntry, ids: [&str; 2]) -> Vec<StoredLeg> {
    let s = [status_of(&entry.account_leg), status_of(&entry.counter_leg)];
    stored(entry, ids, s)
}

fn row(account: &str, amount: Money, date: &str, live: bool) -> BalanceRow {
    BalanceRow {
        posting: Posting::account(account, amount),
        entry_date: date.into(),
        live,
    }
}

// ---- Happy path ---------------------------------------------------------------------------

#[test]
fn tracer_expense_expands_validates_and_classifies_as_expense() {
    let entry = expense("checking", "supermercado", clp(45_990))
        .expand()
        .unwrap();
    assert_eq!(entry.tx_type, TxType::Expense);
    assert_eq!(
        entry.account_leg,
        Posting::account("checking", clp(-45_990))
    );
    assert_eq!(
        entry.counter_leg,
        Posting::category("supermercado", clp(45_990))
    );
    assert_eq!(entry.validate(&ctx()), Ok(()));
    assert_eq!(classify(&entry.postings()), EntryKind::Expense);
}

#[test]
fn income_expands_to_account_credit_and_classifies_as_income() {
    let entry = income("checking", "sueldo", clp(1_000_000))
        .expand()
        .unwrap();
    assert_eq!(entry.tx_type, TxType::Income);
    assert_eq!(
        entry.account_leg,
        Posting::account("checking", clp(1_000_000))
    );
    assert_eq!(
        entry.counter_leg,
        Posting::category("sueldo", clp(-1_000_000))
    );
    assert_eq!(entry.validate(&ctx()), Ok(()));
    assert_eq!(classify(&entry.postings()), EntryKind::Income);
}

#[test]
fn transfer_expands_to_two_account_legs_and_is_never_an_expense() {
    let entry = transfer("checking", "savings", clp(100_000))
        .expand()
        .unwrap();
    assert_eq!(entry.tx_type, TxType::Transfer);
    assert_eq!(
        entry.account_leg,
        Posting::account("checking", clp(-100_000))
    );
    assert_eq!(entry.counter_leg, Posting::account("savings", clp(100_000)));
    assert_eq!(entry.validate(&ctx()), Ok(()));
    assert_eq!(classify(&entry.postings()), EntryKind::Transfer);
}

#[test]
fn opening_balance_is_balanced_two_legs_classified_as_transfer() {
    for amount in [clp(250_000), clp(-80_000)] {
        let entry = opening("checking", amount).expand().unwrap();
        assert_eq!(entry.tx_type, TxType::OpeningBalance);
        assert_eq!(entry.account_leg, Posting::account("checking", amount));
        assert_eq!(
            entry.counter_leg,
            Posting::fx(amount.checked_neg().unwrap())
        );
        assert_eq!(entry.validate(&ctx()), Ok(()));
        assert_eq!(classify(&entry.postings()), EntryKind::Transfer);
    }
}

#[test]
fn balances_sum_live_account_legs_up_to_as_of() {
    let rows = vec![
        row("checking", clp(500_000), "2026-11-01", true),
        row("checking", clp(-45_990), "2026-11-10", true),
        // Boundary: dated exactly as_of counts; the day after does not.
        row("checking", clp(-10_000), "2026-11-15", true),
        row("checking", clp(-1), "2026-11-16", true),
        // Deleted rows don't count.
        row("checking", clp(-999), "2026-11-02", false),
        row("savings", clp(100_000), "2026-11-12", true),
        // Category and FX legs are not account balances.
        BalanceRow {
            posting: Posting::category("supermercado", clp(45_990)),
            entry_date: "2026-11-10".into(),
            live: true,
        },
        BalanceRow {
            posting: Posting::fx(clp(-500_000)),
            entry_date: "2026-11-01".into(),
            live: true,
        },
    ];
    let balances = account_balances(&rows, "2026-11-15").unwrap();
    let expected: BTreeMap<String, Money> = [
        ("checking".to_owned(), clp(444_010)),
        ("savings".to_owned(), clp(100_000)),
    ]
    .into();
    assert_eq!(balances, expected);
}

#[test]
fn update_of_expense_amount_keeps_ids_and_changes_only_amounts() {
    let old = expense("checking", "supermercado", clp(45_990))
        .expand()
        .unwrap();
    let legs = stored_cleared(&old, ["p-acc", "p-cat"]);
    let new = expense("checking", "supermercado", clp(50_000));
    let plan = plan_update(&legs, &new, &ctx()).unwrap();
    assert_eq!(plan.account_leg.id, "p-acc");
    assert_eq!(plan.counter_leg.id, "p-cat");
    assert_eq!(plan.account_leg.changes, [LegField::Amount]);
    assert_eq!(plan.counter_leg.changes, [LegField::Amount]);
    assert_eq!(
        plan.account_leg.posting,
        Posting::account("checking", clp(-50_000))
    );
    assert_eq!(
        plan.counter_leg.posting,
        Posting::category("supermercado", clp(50_000))
    );
    assert_eq!(plan.entry, new.expand().unwrap());
}

#[test]
fn update_expense_to_transfer_converts_counter_leg_in_place() {
    let old = expense("checking", "supermercado", clp(100_000))
        .expand()
        .unwrap();
    let legs = stored_cleared(&old, ["p-acc", "p-cat"]);
    let plan = plan_update(
        &legs,
        &transfer("checking", "savings", clp(100_000)),
        &ctx(),
    )
    .unwrap();
    assert_eq!(plan.account_leg.id, "p-acc");
    assert!(plan.account_leg.changes.is_empty());
    assert_eq!(plan.counter_leg.id, "p-cat");
    assert_eq!(plan.counter_leg.changes, [LegField::Target]);
    assert_eq!(
        plan.counter_leg.posting,
        Posting::account("savings", clp(100_000))
    );
}

#[test]
fn update_matches_roles_of_stored_transfer_and_opening_balance() {
    // A transfer stored counter leg first: the account leg is the negative account leg.
    let t = transfer("checking", "savings", clp(100_000))
        .expand()
        .unwrap();
    let mut legs = stored_cleared(&t, ["p-from", "p-to"]);
    legs.reverse();
    let plan = plan_update(
        &legs,
        &expense("checking", "delivery", clp(100_000)),
        &ctx(),
    )
    .unwrap();
    assert_eq!(plan.account_leg.id, "p-from");
    assert!(plan.account_leg.changes.is_empty());
    assert_eq!(plan.counter_leg.id, "p-to");
    assert_eq!(plan.counter_leg.changes, [LegField::Target]);

    // An opening balance stored FX leg first: the account leg is the only account leg.
    let o = opening("checking", clp(-80_000)).expand().unwrap();
    let mut legs = stored_cleared(&o, ["p-acc", "p-fx"]);
    legs.reverse();
    let plan = plan_update(&legs, &opening("checking", clp(-90_000)), &ctx()).unwrap();
    assert_eq!(plan.account_leg.id, "p-acc");
    assert_eq!(plan.counter_leg.id, "p-fx");
    assert_eq!(plan.account_leg.changes, [LegField::Amount]);
    assert_eq!(plan.counter_leg.changes, [LegField::Amount]);
}

#[test]
fn unchanged_update_reports_no_changes() {
    let tx = income("checking", "sueldo", clp(1_000_000));
    let legs = stored_cleared(&tx.expand().unwrap(), ["a", "b"]);
    let plan = plan_update(&legs, &tx, &ctx()).unwrap();
    assert!(plan.account_leg.changes.is_empty());
    assert!(plan.counter_leg.changes.is_empty());
}

// ---- Properties ---------------------------------------------------------------------------

fn account_id() -> impl Strategy<Value = String> {
    prop_oneof![Just("checking".to_owned()), Just("savings".to_owned())]
}

fn category_id() -> impl Strategy<Value = String> {
    prop_oneof![Just("supermercado".to_owned()), Just("sueldo".to_owned())]
}

/// Any valid `NewTx` over the CLP accounts of `ctx()`.
fn valid_tx() -> impl Strategy<Value = NewTx> {
    let amount = 1i64..=1_000_000_000_000;
    prop_oneof![
        (account_id(), category_id(), amount.clone()).prop_map(|(a, c, v)| expense(&a, &c, clp(v))),
        (account_id(), category_id(), amount.clone()).prop_map(|(a, c, v)| income(&a, &c, clp(v))),
        (any::<bool>(), amount.clone()).prop_map(|(dir, v)| if dir {
            transfer("checking", "savings", clp(v))
        } else {
            transfer("savings", "checking", clp(v))
        }),
        (account_id(), amount, any::<bool>())
            .prop_map(|(a, v, neg)| opening(&a, clp(if neg { -v } else { v }))),
    ]
}

proptest! {
    #[test]
    fn prop_every_expansion_has_two_legs_and_balances(tx in valid_tx()) {
        let entry = tx.expand().unwrap();
        prop_assert_eq!(entry.postings().len(), 2);
        prop_assert_eq!(validate_balanced(&entry.postings()), Ok(()));
        prop_assert_eq!(entry.validate(&ctx()), Ok(()));
    }

    #[test]
    fn prop_transfer_never_classifies_as_expense_or_income(
        dir in any::<bool>(), v in 1i64..=1_000_000_000_000,
    ) {
        let (from, to) = if dir { ("checking", "savings") } else { ("savings", "checking") };
        let entry = transfer(from, to, clp(v)).expand().unwrap();
        prop_assert_eq!(classify(&entry.postings()), EntryKind::Transfer);
    }

    #[test]
    fn prop_balance_equals_sum_of_live_legs_up_to_as_of(
        legs in proptest::collection::vec(
            (account_id(), -1_000_000i64..1_000_000, 1u8..=28, any::<bool>()), 0..40),
        as_of_day in 1u8..=28,
    ) {
        let rows: Vec<BalanceRow> = legs
            .iter()
            .map(|(a, v, d, live)| row(a, clp(*v), &format!("2026-11-{d:02}"), *live))
            .collect();
        let as_of = format!("2026-11-{as_of_day:02}");
        let balances = account_balances(&rows, &as_of).unwrap();
        for account in ["checking", "savings"] {
            let counted: Vec<i64> = legs
                .iter()
                .filter(|(a, _, d, live)| a == account && *live && *d <= as_of_day)
                .map(|(_, v, _, _)| *v)
                .collect();
            let got = balances.get(account);
            if counted.is_empty() {
                prop_assert_eq!(got, None);
            } else {
                prop_assert_eq!(got, Some(&clp(counted.iter().sum())));
            }
        }
    }

    #[test]
    fn prop_update_keeps_exactly_the_existing_ids(old in valid_tx(), new in valid_tx()) {
        let legs = stored_cleared(&old.expand().unwrap(), ["id-1", "id-2"]);
        let plan = plan_update(&legs, &new, &ctx()).unwrap();
        let mut ids = [plan.account_leg.id.clone(), plan.counter_leg.id.clone()];
        ids.sort();
        prop_assert_eq!(ids, ["id-1".to_owned(), "id-2".to_owned()]);
        prop_assert_eq!(plan.entry, new.expand().unwrap());
    }
}

// ---- Edge cases ---------------------------------------------------------------------------

#[test]
fn edge_posting_to_archived_account_is_rejected() {
    // Contrast: the same expense on an open account is accepted.
    let ok = expense("checking", "supermercado", clp(1_000))
        .expand()
        .unwrap();
    assert_eq!(ok.validate(&ctx()), Ok(()));
    let entry = expense("old", "supermercado", clp(1_000)).expand().unwrap();
    assert_eq!(
        entry.validate(&ctx()),
        Err(DomainError::ArchivedAccount("old".into()))
    );
    let to_old = transfer("checking", "old", clp(1_000)).expand().unwrap();
    assert_eq!(
        to_old.validate(&ctx()),
        Err(DomainError::ArchivedAccount("old".into()))
    );
}

#[test]
fn edge_account_leg_currency_must_match_account() {
    let entry = expense("checking", "software", usd(2_500))
        .expand()
        .unwrap();
    assert_eq!(
        entry.validate(&ctx()),
        Err(DomainError::CurrencyMismatch {
            account: "checking".into(),
            expected: CLP,
            found: USD,
        })
    );
    // A CLP→USD transfer with one amount: the USD account's leg is in CLP.
    let cross = transfer("checking", "card-usd", clp(23_750))
        .expand()
        .unwrap();
    assert_eq!(
        cross.validate(&ctx()),
        Err(DomainError::CurrencyMismatch {
            account: "card-usd".into(),
            expected: USD,
            found: CLP,
        })
    );
    // Contrast: a USD expense on the USD account is accepted.
    let ok = expense("card-usd", "software", usd(2_500))
        .expand()
        .unwrap();
    assert_eq!(ok.validate(&ctx()), Ok(()));
}

#[test]
fn edge_unknown_account_is_rejected() {
    let entry = expense("nope", "supermercado", clp(1_000))
        .expand()
        .unwrap();
    assert_eq!(
        entry.validate(&ctx()),
        Err(DomainError::UnknownAccount("nope".into()))
    );
}

#[test]
fn edge_unbalanced_entry_is_rejected() {
    let entry = JournalEntry {
        tx_type: TxType::Expense,
        account_leg: Posting::account("checking", clp(-10_000)),
        counter_leg: Posting::category("delivery", clp(9_999)),
    };
    assert_eq!(
        entry.validate(&ctx()),
        Err(DomainError::Unbalanced {
            currency: CLP,
            difference: -1
        })
    );
}

#[test]
fn edge_transfer_with_category_leg_is_rejected() {
    let entry = JournalEntry {
        tx_type: TxType::Transfer,
        account_leg: Posting::account("checking", clp(-10_000)),
        counter_leg: Posting::category("supermercado", clp(10_000)),
    };
    assert_eq!(
        entry.validate(&ctx()),
        Err(DomainError::TypeMismatch {
            declared: TxType::Transfer,
            derived: EntryKind::Expense,
        })
    );
}

#[test]
fn edge_amount_rules_on_expansion() {
    for amount in [clp(0), clp(-1)] {
        for tx in [
            expense("checking", "supermercado", amount),
            income("checking", "sueldo", amount),
            transfer("checking", "savings", amount),
        ] {
            assert_eq!(tx.expand(), Err(DomainError::NonPositiveAmount), "{tx:?}");
        }
    }
    assert_eq!(
        opening("checking", clp(0)).expand(),
        Err(DomainError::ZeroAmount)
    );
    assert_eq!(
        transfer("checking", "checking", clp(1)).expand(),
        Err(DomainError::SameAccountTransfer)
    );
    // Contrast: the smallest positive amount is accepted.
    assert!(expense("checking", "supermercado", clp(1)).expand().is_ok());
}

#[test]
fn edge_reconciled_entry_is_locked() {
    let old = expense("checking", "supermercado", clp(45_990))
        .expand()
        .unwrap();
    let legs = stored(
        &old,
        ["p-acc", "p-cat"],
        [Some(LegStatus::Reconciled), None],
    );
    // Changing only the (unreconciled) category leg is still refused: the entry is locked.
    assert_eq!(
        plan_update(&legs, &expense("checking", "delivery", clp(45_990)), &ctx()),
        Err(DomainError::ReconciledEntryLocked {
            posting_id: "p-acc".into()
        })
    );
    // An unchanged update of it succeeds.
    let same = expense("checking", "supermercado", clp(45_990));
    assert!(plan_update(&legs, &same, &ctx()).is_ok());
    assert_eq!(
        ensure_unlocked(&legs),
        Err(DomainError::ReconciledEntryLocked {
            posting_id: "p-acc".into()
        })
    );
    // Contrast: pending and cleared entries are not locked.
    for status in [LegStatus::Pending, LegStatus::Cleared] {
        let open = stored(&old, ["p-acc", "p-cat"], [Some(status), None]);
        assert_eq!(ensure_unlocked(&open), Ok(()));
        assert!(plan_update(&open, &expense("checking", "delivery", clp(1)), &ctx()).is_ok());
    }
}

#[test]
fn edge_unsupported_stored_shape_is_rejected() {
    let leg = |id: &str, p: Posting| StoredLeg {
        id: id.into(),
        status: status_of(&p),
        posting: p,
    };
    let new = expense("checking", "supermercado", clp(1));
    let cases = [
        vec![leg("a", Posting::account("checking", clp(-1)))],
        vec![
            leg("a", Posting::account("checking", clp(-2))),
            leg("b", Posting::category("x", clp(1))),
            leg("c", Posting::category("y", clp(1))),
        ],
        // No account leg at all.
        vec![
            leg("a", Posting::category("x", clp(-1))),
            leg("b", Posting::category("y", clp(1))),
        ],
    ];
    for legs in cases {
        assert_eq!(
            plan_update(&legs, &new, &ctx()),
            Err(DomainError::UnsupportedEntryShape),
            "{legs:?}"
        );
    }
}

#[test]
fn abuse_overflow_is_an_error_not_a_panic() {
    // Negating i64::MIN overflows.
    assert_eq!(
        opening("checking", clp(i64::MIN)).expand(),
        Err(DomainError::Money(MoneyError::Overflow))
    );
    let rows = [
        row("checking", clp(i64::MAX), "2026-11-01", true),
        row("checking", clp(1), "2026-11-01", true),
    ];
    assert_eq!(
        account_balances(&rows, "2026-11-30"),
        Err(DomainError::Money(MoneyError::Overflow))
    );
    // Contrast: i64::MAX alone is representable.
    assert!(account_balances(&rows[..1], "2026-11-30").is_ok());
}

// ---- Remediation (tyr-verdict iteration 1) ------------------------------------------------

#[test]
fn f2_validate_rejects_wrong_leg_kinds_for_the_type() {
    let wrong = [
        // A transfer's counter leg must be an account.
        JournalEntry {
            tx_type: TxType::Transfer,
            account_leg: Posting::account("checking", clp(-1_000)),
            counter_leg: Posting::fx(clp(1_000)),
        },
        // An expense's account leg must be an account.
        JournalEntry {
            tx_type: TxType::Expense,
            account_leg: Posting::fx(clp(-1_000)),
            counter_leg: Posting::category("supermercado", clp(1_000)),
        },
        // An opening balance's counter leg must be FX (ADR-0011).
        JournalEntry {
            tx_type: TxType::OpeningBalance,
            account_leg: Posting::account("checking", clp(5)),
            counter_leg: Posting::account("savings", clp(-5)),
        },
    ];
    for entry in wrong {
        assert_eq!(
            entry.validate(&ctx()),
            Err(DomainError::LegKindMismatch {
                declared: entry.tx_type
            }),
            "{entry:?}"
        );
    }
    // Contrast: every expansion has the right kinds and validates.
    for tx in [
        expense("checking", "supermercado", clp(1)),
        income("checking", "sueldo", clp(1)),
        transfer("checking", "savings", clp(1)),
        opening("checking", clp(1)),
    ] {
        assert_eq!(tx.expand().unwrap().validate(&ctx()), Ok(()));
    }
}

#[test]
fn f3_unchanged_update_on_archived_account_succeeds_and_lock_comes_first() {
    let tx = expense("old", "supermercado", clp(10));
    let legs = stored_cleared(&tx.expand().unwrap(), ["a", "b"]);
    let plan = plan_update(&legs, &tx, &ctx()).unwrap();
    assert!(plan.account_leg.changes.is_empty() && plan.counter_leg.changes.is_empty());
    // Contrast: a real change on the archived account is still refused.
    assert_eq!(
        plan_update(&legs, &expense("old", "supermercado", clp(11)), &ctx()),
        Err(DomainError::ArchivedAccount("old".into()))
    );
    // A locked entry answers "locked" even when the edit is also invalid.
    let locked = stored(
        &tx.expand().unwrap(),
        ["a", "b"],
        [Some(LegStatus::Reconciled), None],
    );
    assert_eq!(
        plan_update(&locked, &expense("old", "supermercado", clp(11)), &ctx()),
        Err(DomainError::ReconciledEntryLocked {
            posting_id: "a".into()
        })
    );
}

#[test]
fn f7_balances_reject_an_as_of_that_is_not_a_civil_date() {
    let rows = [row("checking", clp(7), "2026-11-10", true)];
    for bad in [
        "2026-11-5",
        "",
        "2026/11/15",
        "26-11-15",
        "2026-11-15x",
        "2026-1a-15",
    ] {
        assert_eq!(
            account_balances(&rows, bad),
            Err(DomainError::InvalidDate(bad.into())),
            "{bad:?}"
        );
    }
    // Contrast: a well-formed date is accepted.
    assert!(account_balances(&rows, "2026-11-15").is_ok());
}
