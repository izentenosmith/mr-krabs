//! Acceptance tests for the public civil-date rule (ledger follow-ups F18, F19), through
//! `mrk_domain`'s public API only.

use mrk_domain::{DomainError, validate_civil_date};
use proptest::prelude::*;

fn rejected(date: &str) {
    assert_eq!(
        validate_civil_date(date),
        Err(DomainError::InvalidDate(date.to_owned())),
        "{date} should be rejected"
    );
}

fn accepted(date: &str) {
    assert_eq!(validate_civil_date(date), Ok(()), "{date} should be accepted");
}

/// Test-side oracle: Gregorian month lengths, written independently of the crate.
fn days_in(year: u32, month: u32) -> u32 {
    let leap = (year % 400 == 0) || (year % 4 == 0 && year % 100 != 0);
    [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month as usize - 1]
}

// AC1
#[test]
fn ac1_real_date_is_ok_and_anything_else_is_invalid_date_with_the_input() {
    accepted("2026-11-22");
    for bad in [
        "", "2026-11-2", "2026/11/22", "2026-11-22 ", " 2026-11-22", "26-11-22", "2026-1-022",
        "2026-11-2x", "+026-11-22", "2026-02-30", "２０２６-11-22",
    ] {
        rejected(bad);
    }
}

// AC2
#[test]
fn ac2_century_leap_rule() {
    rejected("1900-02-29");
    rejected("2100-02-29");
    rejected("2023-02-29");
    accepted("2000-02-29");
    accepted("2024-02-29");
    accepted("1900-02-28");
    accepted("2100-02-28");
}

// AC3
#[test]
fn ac3_day_00_and_month_00_or_13_are_rejected() {
    rejected("2026-01-00");
    rejected("2026-00-01");
    rejected("2026-13-01");
    rejected("2026-00-00");
}

// AC4
#[test]
fn ac4_day_31_only_in_31_day_months_and_day_30_never_in_february() {
    for month in 1..=12u32 {
        let d31 = format!("2026-{month:02}-31");
        let d30 = format!("2026-{month:02}-30");
        if [1, 3, 5, 7, 8, 10, 12].contains(&month) {
            accepted(&d31);
        } else {
            rejected(&d31);
        }
        if month == 2 {
            rejected(&d30);
        } else {
            accepted(&d30);
        }
        rejected(&format!("2026-{month:02}-32"));
    }
}

proptest! {
    // AC5: every accepted string is the canonical spelling of its own year, month and day.
    #[test]
    fn ac5_accepted_dates_round_trip(s in "[0-9]{4}-[0-9]{2}-[0-9]{2}|\\PC{0,12}") {
        if validate_civil_date(&s).is_ok() {
            let year: u32 = s[0..4].parse().unwrap();
            let month: u32 = s[5..7].parse().unwrap();
            let day: u32 = s[8..10].parse().unwrap();
            prop_assert_eq!(format!("{year:04}-{month:02}-{day:02}"), s);
        }
    }

    // AC5: the rule agrees with an independent calendar on every well-shaped date.
    #[test]
    fn ac5_accepts_exactly_the_days_that_exist(year in 0u32..=9999, month in 0u32..=13, day in 0u32..=32) {
        let s = format!("{year:04}-{month:02}-{day:02}");
        let exists = (1..=12).contains(&month) && (1..=days_in(year, month)).contains(&day);
        prop_assert_eq!(validate_civil_date(&s).is_ok(), exists, "{}", s);
    }
}
