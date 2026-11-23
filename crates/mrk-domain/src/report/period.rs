//! Report periods: years, `YYYY-MM` months and civil date ranges, validated at the boundary.

use super::ReportError;
use crate::date::{days_in_month, validate_civil_date};

const YEARS: std::ops::RangeInclusive<u32> = 1970..=9999;

/// A calendar month of a year in 1970–9999.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Month {
    pub year: u32,
    pub month: u32,
}

impl Month {
    /// Parses `YYYY-MM`; anything else is `InvalidMonth`.
    pub fn parse(s: &str) -> Result<Self, ReportError> {
        let invalid = || ReportError::InvalidMonth(s.to_owned());
        let b = s.as_bytes();
        if b.len() != 7 || b[4] != b'-' {
            return Err(invalid());
        }
        let number = |r: std::ops::Range<usize>| {
            b[r].iter().try_fold(0u32, |n, c| {
                c.is_ascii_digit().then(|| n * 10 + u32::from(c - b'0'))
            })
        };
        match (number(0..4), number(5..7)) {
            (Some(year), Some(month)) if YEARS.contains(&year) && (1..=12).contains(&month) => {
                Ok(Self { year, month })
            }
            _ => Err(invalid()),
        }
    }

    /// The month a civil `YYYY-MM-DD` date (already validated) falls in.
    pub fn of_date(date: &str) -> Self {
        let digits = |r: std::ops::Range<usize>| {
            date.as_bytes()[r]
                .iter()
                .fold(0u32, |n, c| n * 10 + u32::from(c - b'0'))
        };
        Self {
            year: digits(0..4),
            month: digits(5..7),
        }
    }

    pub fn days(self) -> u32 {
        days_in_month(self.year, self.month).unwrap_or(0)
    }

    pub fn first_day(self) -> String {
        format!("{self}-01")
    }

    pub fn last_day(self) -> String {
        format!("{self}-{:02}", self.days())
    }

    fn next(self) -> Self {
        if self.month == 12 {
            Self {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Self {
                month: self.month + 1,
                ..self
            }
        }
    }
}

/// The day of month of a civil `YYYY-MM-DD` date (already validated).
pub(crate) fn day_of(date: &str) -> u32 {
    date.as_bytes()[8..10]
        .iter()
        .fold(0u32, |n, c| n * 10 + u32::from(c - b'0'))
}

impl std::fmt::Display for Month {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:04}-{:02}", self.year, self.month)
    }
}

/// Checks a report year (1970–9999) and returns its months, January first.
pub(crate) fn year_months(year: i32) -> Result<Vec<Month>, ReportError> {
    match u32::try_from(year) {
        Ok(y) if YEARS.contains(&y) => Ok((1..=12).map(|month| Month { year: y, month }).collect()),
        _ => Err(ReportError::InvalidYear(year)),
    }
}

/// An inclusive range of civil dates, `from ≤ to`. Civil dates compare as strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DateRange {
    pub from: String,
    pub to: String,
}

impl DateRange {
    pub fn parse(from: &str, to: &str) -> Result<Self, ReportError> {
        for d in [from, to] {
            validate_civil_date(d).map_err(|_| ReportError::InvalidDate(d.to_owned()))?;
        }
        if from > to {
            return Err(ReportError::InvalidRange {
                from: from.to_owned(),
                to: to.to_owned(),
            });
        }
        Ok(Self {
            from: from.to_owned(),
            to: to.to_owned(),
        })
    }

    /// The whole of the given months, first to last (built from validated months).
    pub fn of_months(first: Month, last: Month) -> Self {
        Self {
            from: first.first_day(),
            to: last.last_day(),
        }
    }

    pub fn contains(&self, date: &str) -> bool {
        self.from.as_str() <= date && date <= self.to.as_str()
    }

    /// Every calendar month the range touches, in order.
    pub fn months(&self) -> Vec<Month> {
        let last = Month::of_date(&self.to);
        let mut m = Month::of_date(&self.from);
        let mut out = vec![m];
        while m < last {
            m = m.next();
            out.push(m);
        }
        out
    }
}
