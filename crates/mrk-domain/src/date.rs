//! Civil dates as the ledger stores them: `YYYY-MM-DD` text (CLAUDE.md conventions).

use crate::ledger::DomainError;

/// Checks that `date` is a real calendar date written `YYYY-MM-DD`: four ASCII digits for the
/// year (0000–9999, proleptic Gregorian), month 01–12, and a day that exists in that month
/// (29 February only in leap years: divisible by 4, except centuries not divisible by 400).
/// In this shape string order is date order, which balances and the store's `entry_date`
/// rely on. Anything else is `Err(DomainError::InvalidDate(date))`, which core maps to
/// `VALIDATION` with `fields: { date: "errors.field.invalidDate" }` (X-8).
///
/// This is the one date rule of the ledger: opening-balance dates and `as_of` already go
/// through it, and the transactions slice must call it for every `entry_date` it writes,
/// new or edited, before the entry reaches the store (X-6).
pub fn validate_civil_date(date: &str) -> Result<(), DomainError> {
    if is_civil_date(date) {
        Ok(())
    } else {
        Err(DomainError::InvalidDate(date.to_owned()))
    }
}

fn is_civil_date(date: &str) -> bool {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let number = |range: std::ops::Range<usize>| -> Option<u32> {
        b[range].iter().try_fold(0u32, |n, c| {
            c.is_ascii_digit().then(|| n * 10 + u32::from(c - b'0'))
        })
    };
    let (Some(year), Some(month), Some(day)) = (number(0..4), number(5..7), number(8..10)) else {
        return false;
    };
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days_in_month).contains(&day)
}
