//! Exact money for Mr Krabs.
//!
//! * Amounts are `i64` **minor units** (CLP has 0 decimals, USD 2, UF/CLF 4).
//! * No floating point anywhere (enforced by clippy, see `clippy.toml`).
//! * Arithmetic is checked: currency mismatches and overflows are errors, never silent.

use std::fmt;

/// Errors produced by money operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    #[error("invalid currency code: {0:?}")]
    InvalidCurrencyCode(String),
    #[error("currency mismatch: {left} vs {right}")]
    CurrencyMismatch {
        left: CurrencyCode,
        right: CurrencyCode,
    },
    #[error("amount overflow")]
    Overflow,
    #[error("allocation needs at least one positive weight")]
    EmptyAllocation,
    #[error("invalid amount: {0:?}")]
    InvalidAmount(String),
    #[error("too many decimals (currency allows {allowed})")]
    TooManyDecimals { allowed: u8 },
}

/// ISO-4217-like currency code (3–8 uppercase ASCII letters), stored inline (`Copy`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CurrencyCode {
    bytes: [u8; 8],
    len: u8,
}

impl CurrencyCode {
    /// Parses and validates a code such as `"CLP"`.
    pub fn new(code: &str) -> Result<Self, MoneyError> {
        let b = code.as_bytes();
        if !(3..=8).contains(&b.len()) || !b.iter().all(u8::is_ascii_uppercase) {
            return Err(MoneyError::InvalidCurrencyCode(code.to_owned()));
        }
        Ok(Self::from_ascii(b))
    }

    /// Const constructor for built-in codes. Callers must pass 3–8 uppercase ASCII letters.
    const fn from_ascii(b: &[u8]) -> Self {
        let mut bytes = [0u8; 8];
        let mut i = 0;
        while i < b.len() {
            bytes[i] = b[i];
            i += 1;
        }
        Self {
            bytes,
            len: b.len() as u8,
        }
    }

    pub fn as_str(&self) -> &str {
        // Only ever constructed from validated ASCII.
        std::str::from_utf8(&self.bytes[..self.len as usize]).unwrap_or("???")
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CurrencyCode({})", self.as_str())
    }
}

/// Currency metadata needed for exact arithmetic and display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Currency {
    pub code: CurrencyCode,
    pub minor_units: u8,
    pub symbol: &'static str,
}

impl Currency {
    /// Chilean peso — the default base currency.
    pub const CLP: Currency = Currency {
        code: CurrencyCode::from_ascii(b"CLP"),
        minor_units: 0,
        symbol: "$",
    };
    pub const USD: Currency = Currency {
        code: CurrencyCode::from_ascii(b"USD"),
        minor_units: 2,
        symbol: "US$",
    };
    pub const EUR: Currency = Currency {
        code: CurrencyCode::from_ascii(b"EUR"),
        minor_units: 2,
        symbol: "€",
    };
    /// Unidad de Fomento (ISO code CLF), a unit of account with 4 decimals.
    pub const CLF: Currency = Currency {
        code: CurrencyCode::from_ascii(b"CLF"),
        minor_units: 4,
        symbol: "UF ",
    };

    /// Currencies available out of the box.
    pub const BUILT_IN: [Currency; 4] = [Self::CLP, Self::USD, Self::EUR, Self::CLF];

    pub fn built_in(code: &str) -> Option<Currency> {
        Self::BUILT_IN.into_iter().find(|c| c.code.as_str() == code)
    }
}

/// An exact amount of money in minor units.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Money {
    pub minor: i64,
    pub currency: CurrencyCode,
}

impl Money {
    pub const fn new(minor: i64, currency: CurrencyCode) -> Self {
        Self { minor, currency }
    }

    pub const fn zero(currency: CurrencyCode) -> Self {
        Self { minor: 0, currency }
    }

    pub const fn is_zero(&self) -> bool {
        self.minor == 0
    }

    fn same_currency(&self, other: &Money) -> Result<(), MoneyError> {
        if self.currency == other.currency {
            Ok(())
        } else {
            Err(MoneyError::CurrencyMismatch {
                left: self.currency,
                right: other.currency,
            })
        }
    }

    pub fn checked_add(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(&other)?;
        let minor = self
            .minor
            .checked_add(other.minor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    pub fn checked_sub(self, other: Money) -> Result<Money, MoneyError> {
        self.same_currency(&other)?;
        let minor = self
            .minor
            .checked_sub(other.minor)
            .ok_or(MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    pub fn checked_neg(self) -> Result<Money, MoneyError> {
        let minor = self.minor.checked_neg().ok_or(MoneyError::Overflow)?;
        Ok(Money::new(minor, self.currency))
    }

    /// Splits the amount proportionally to `weights` using the largest-remainder method.
    /// The parts always sum **exactly** to the original amount (used for shared expenses
    /// and installment schedules).
    pub fn allocate(self, weights: &[u64]) -> Result<Vec<Money>, MoneyError> {
        let total_weight: u128 = weights.iter().map(|&w| u128::from(w)).sum();
        if total_weight == 0 {
            return Err(MoneyError::EmptyAllocation);
        }
        let negative = self.minor < 0;
        let amount = u128::from(self.minor.unsigned_abs());

        let mut parts: Vec<u128> = Vec::with_capacity(weights.len());
        let mut remainders: Vec<(u128, usize)> = Vec::with_capacity(weights.len());
        for (i, &w) in weights.iter().enumerate() {
            let scaled = amount * u128::from(w);
            parts.push(scaled / total_weight);
            remainders.push((scaled % total_weight, i));
        }
        let mut leftover = amount - parts.iter().sum::<u128>();
        // Largest remainder first; ties go to the earliest index (deterministic).
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, i) in remainders {
            if leftover == 0 {
                break;
            }
            parts[i] += 1;
            leftover -= 1;
        }

        parts
            .into_iter()
            .map(|p| {
                let v = i64::try_from(p).map_err(|_| MoneyError::Overflow)?;
                Ok(Money::new(if negative { -v } else { v }, self.currency))
            })
            .collect()
    }

    /// Splits into `n` near-equal parts that sum exactly to the original.
    pub fn split_evenly(self, n: usize) -> Result<Vec<Money>, MoneyError> {
        self.allocate(&vec![1; n])
    }
}

/// Parses an amount written in Chilean format into minor units.
///
/// * Thousands separator `.`, decimal separator `,` (`"1.234.567"`, `"12,50"`).
/// * Accepts spaces, wrapping parentheses, then a leading `-`, then at most one marker of
///   `currency` itself: its symbol (`$`, `US$`, `€`, `UF`) or ISO code (`CLP`), in any ASCII
///   case. The ISO code may instead follow the number (`27.990 CLP`). Another currency's
///   marker is an error (`€10` is not a CLP amount); no marker is fine.
/// * Never rounds: more decimals than the currency allows is an error.
pub fn parse_es_cl(input: &str, currency: Currency) -> Result<i64, MoneyError> {
    let minor_units = currency.minor_units;
    let invalid = || MoneyError::InvalidAmount(input.to_owned());
    let mut s: String = input.chars().filter(|c| !c.is_whitespace()).collect();

    let mut negative = false;
    if s.starts_with('(') && s.ends_with(')') && s.len() >= 2 {
        negative = true;
        s = s[1..s.len() - 1].to_owned();
    }
    if let Some(rest) = s.strip_prefix('-') {
        negative = !negative;
        s = rest.to_owned();
    }
    let s = strip_currency_marker(&s, currency);
    if s.is_empty() {
        return Err(invalid());
    }

    let (int_part, frac_part) = match s.rsplit_once(',') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int_part.is_empty() || frac_part.contains('.') {
        return Err(invalid());
    }
    if s.matches(',').count() > 1 {
        return Err(invalid());
    }

    // Validate thousands grouping: first group 1-3 digits, following groups exactly 3.
    let groups: Vec<&str> = int_part.split('.').collect();
    for (i, g) in groups.iter().enumerate() {
        let ok_len = if i == 0 {
            (1..=3).contains(&g.len()) || groups.len() == 1
        } else {
            g.len() == 3
        };
        if !ok_len || g.is_empty() || !g.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
    }
    if !frac_part.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    if frac_part.len() > usize::from(minor_units) {
        return Err(MoneyError::TooManyDecimals {
            allowed: minor_units,
        });
    }

    let mut digits: String = groups.concat();
    digits.push_str(frac_part);
    for _ in frac_part.len()..usize::from(minor_units) {
        digits.push('0');
    }
    // i128 so that i64::MIN's magnitude (one past i64::MAX) still reads back.
    let value: i128 = digits.parse().map_err(|_| MoneyError::Overflow)?;
    i64::try_from(if negative { -value } else { value }).map_err(|_| MoneyError::Overflow)
}

/// Removes one marker of `currency`: a leading symbol or ISO code, or else a trailing ISO
/// code. `s` has no whitespace, so the symbol is compared without its own (`"UF "` → `UF`).
/// The longer marker is tried first, so neither can shadow the other. Anything else, another
/// currency's marker included, is left in place for the digit checks to reject.
fn strip_currency_marker(s: &str, currency: Currency) -> &str {
    fn strip_prefix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
        let head = s.get(..p.len())?;
        head.eq_ignore_ascii_case(p).then(|| &s[p.len()..])
    }
    fn strip_suffix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
        let split = s.len().checked_sub(p.len())?;
        let tail = s.get(split..)?;
        tail.eq_ignore_ascii_case(p).then(|| &s[..split])
    }

    let code = currency.code.as_str();
    let mut markers = [currency.symbol.trim(), code];
    markers.sort_by_key(|m| std::cmp::Reverse(m.len()));
    markers
        .into_iter()
        .filter(|m| !m.is_empty())
        .find_map(|m| strip_prefix_ci(s, m))
        .or_else(|| strip_suffix_ci(s, code))
        .unwrap_or(s)
}

/// Formats minor units in Chilean style: `$1.234.567`, `-US$12,50`, `UF 1.234,5678`.
pub fn format_es_cl(minor: i64, minor_units: u8, symbol: &str) -> String {
    let abs = minor.unsigned_abs();
    let divisor = 10u64.pow(u32::from(minor_units));
    let int_part = abs / divisor;
    let frac_part = abs % divisor;

    let raw = int_part.to_string();
    let mut grouped = String::with_capacity(raw.len() + raw.len() / 3);
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i) % 3 == 0 {
            grouped.push('.');
        }
        grouped.push(ch);
    }

    let sign = if minor < 0 { "-" } else { "" };
    if minor_units == 0 {
        format!("{sign}{symbol}{grouped}")
    } else {
        format!(
            "{sign}{symbol}{grouped},{frac_part:0width$}",
            width = usize::from(minor_units)
        )
    }
}

impl Money {
    /// Display string for a known currency (falls back to the code as a prefix).
    pub fn display_es_cl(&self) -> String {
        match Currency::built_in(self.currency.as_str()) {
            Some(c) => format_es_cl(self.minor, c.minor_units, c.symbol),
            None => format_es_cl(self.minor, 0, &format!("{} ", self.currency)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clp(minor: i64) -> Money {
        Money::new(minor, Currency::CLP.code)
    }

    #[test]
    fn currency_code_validation() {
        assert_eq!(CurrencyCode::new("CLP").unwrap().as_str(), "CLP");
        assert!(CurrencyCode::new("clp").is_err());
        assert!(CurrencyCode::new("CL").is_err());
        assert!(CurrencyCode::new("TOOLONGCODE").is_err());
        assert_eq!(Currency::CLF.code.as_str(), "CLF");
    }

    #[test]
    fn add_and_sub_same_currency() {
        assert_eq!(clp(1_000).checked_add(clp(500)).unwrap(), clp(1_500));
        assert_eq!(clp(1_000).checked_sub(clp(1_500)).unwrap(), clp(-500));
    }

    #[test]
    fn mixing_currencies_is_an_error() {
        let usd = Money::new(100, Currency::USD.code);
        assert!(matches!(
            clp(1).checked_add(usd),
            Err(MoneyError::CurrencyMismatch { .. })
        ));
    }

    #[test]
    fn overflow_is_an_error() {
        assert_eq!(clp(i64::MAX).checked_add(clp(1)), Err(MoneyError::Overflow));
        assert_eq!(clp(i64::MIN).checked_neg(), Err(MoneyError::Overflow));
    }

    #[test]
    fn allocate_sums_exactly() {
        let parts = clp(100).split_evenly(3).unwrap();
        assert_eq!(parts, vec![clp(34), clp(33), clp(33)]);

        let parts = clp(-100).split_evenly(3).unwrap();
        assert_eq!(parts, vec![clp(-34), clp(-33), clp(-33)]);

        // Subscription of 49.900 split between 3 people.
        let parts = clp(49_900).split_evenly(3).unwrap();
        assert_eq!(parts.iter().map(|m| m.minor).sum::<i64>(), 49_900);
        assert_eq!(parts, vec![clp(16_634), clp(16_633), clp(16_633)]);
    }

    #[test]
    fn allocate_by_weights() {
        let parts = clp(1_000).allocate(&[70, 30]).unwrap();
        assert_eq!(parts, vec![clp(700), clp(300)]);
        let parts = clp(10).allocate(&[1, 1, 1, 0]).unwrap();
        assert_eq!(parts.iter().map(|m| m.minor).sum::<i64>(), 10);
        assert_eq!(parts[3], clp(0));
        assert_eq!(clp(10).allocate(&[0, 0]), Err(MoneyError::EmptyAllocation));
    }

    #[test]
    fn parse_clp() {
        assert_eq!(parse_es_cl("1.234.567", Currency::CLP), Ok(1_234_567));
        assert_eq!(parse_es_cl("$1.234.567", Currency::CLP), Ok(1_234_567));
        assert_eq!(parse_es_cl("-$45.990", Currency::CLP), Ok(-45_990));
        assert_eq!(parse_es_cl("(45.990)", Currency::CLP), Ok(-45_990));
        assert_eq!(parse_es_cl(" 500 ", Currency::CLP), Ok(500));
        assert_eq!(parse_es_cl("1234567", Currency::CLP), Ok(1_234_567));
        assert_eq!(parse_es_cl("CLP 27.990", Currency::CLP), Ok(27_990));
    }

    #[test]
    fn parse_with_decimals() {
        assert_eq!(parse_es_cl("12,5", Currency::USD), Ok(1_250));
        assert_eq!(parse_es_cl("US$1.234,56", Currency::USD), Ok(123_456));
        assert_eq!(parse_es_cl("UF 1,2345", Currency::CLF), Ok(12_345));
    }

    /// X-9: Rust's own EUR display string must parse back.
    #[test]
    fn parse_eur_display() {
        assert_eq!(parse_es_cl("€1.234,56", Currency::EUR), Ok(123_456));
        assert_eq!(parse_es_cl("-€1.234,56", Currency::EUR), Ok(-123_456));
        assert_eq!(parse_es_cl("(€1.234,56)", Currency::EUR), Ok(-123_456));
        assert_eq!(parse_es_cl("EUR 1.234,56", Currency::EUR), Ok(123_456));
        assert_eq!(parse_es_cl("1.234,56 EUR", Currency::EUR), Ok(123_456));
    }

    #[test]
    fn parse_accepts_markers_case_insensitively() {
        assert_eq!(parse_es_cl("clp 27.990", Currency::CLP), Ok(27_990));
        assert_eq!(parse_es_cl("uf 1,5", Currency::CLF), Ok(15_000));
        assert_eq!(parse_es_cl("27.990 CLP", Currency::CLP), Ok(27_990));
    }

    #[test]
    fn parse_rejects_misplaced_markers() {
        for bad in [
            "€", "$", "US$", "UF", "CLP", "-€", "()", // marker alone
            "1.€234", "12€", "1$0", // marker in the middle or at the end
            "$$5", "€€5", "CLP$5", "$5CLP", "US$€5", // two markers
            "$-5", "€(5)", // marker before the sign
            "abc5", "5abc", "X$5", // unknown letters
            "£5", "é5", "5é", "5€",
            "€5€", // other non-ASCII: rejected, never a char-boundary panic
        ] {
            for c in Currency::BUILT_IN {
                assert!(
                    parse_es_cl(bad, c).is_err(),
                    "{bad:?} should be rejected for {}",
                    c.code
                );
            }
        }
    }

    /// X-12: a marker must name the currency being parsed; no marker is always fine.
    #[test]
    fn parse_requires_the_target_currencys_marker() {
        assert!(matches!(
            parse_es_cl("€10", Currency::CLP),
            Err(MoneyError::InvalidAmount(_))
        ));
        assert!(parse_es_cl("US$10", Currency::CLP).is_err());
        assert!(parse_es_cl("10 EUR", Currency::CLP).is_err());
        assert!(parse_es_cl("$10", Currency::USD).is_err());
        assert!(parse_es_cl("UF 1", Currency::CLP).is_err());
        assert_eq!(parse_es_cl("$10", Currency::CLP), Ok(10));
        assert_eq!(parse_es_cl("US$10", Currency::USD), Ok(1_000));
        assert_eq!(parse_es_cl("USD 10", Currency::USD), Ok(1_000));
        assert_eq!(parse_es_cl("10", Currency::CLP), Ok(10));
        assert_eq!(parse_es_cl("10", Currency::EUR), Ok(1_000));
    }

    #[test]
    fn parse_reads_the_i64_edges() {
        assert_eq!(
            parse_es_cl(&format_es_cl(i64::MIN, 0, "$"), Currency::CLP),
            Ok(i64::MIN)
        );
        assert_eq!(
            parse_es_cl(&format_es_cl(i64::MAX, 0, "$"), Currency::CLP),
            Ok(i64::MAX)
        );
        assert_eq!(
            parse_es_cl("9.223.372.036.854.775.808", Currency::CLP),
            Err(MoneyError::Overflow)
        );
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert_eq!(
            parse_es_cl("1.234,5", Currency::CLP),
            Err(MoneyError::TooManyDecimals { allowed: 0 })
        );
        assert!(parse_es_cl("1.23", Currency::CLP).is_err()); // bad grouping
        assert!(parse_es_cl("1,2,3", Currency::USD).is_err());
        assert!(parse_es_cl("abc", Currency::CLP).is_err());
        assert!(parse_es_cl("", Currency::CLP).is_err());
        assert!(parse_es_cl("12a3", Currency::CLP).is_err());
    }

    #[test]
    fn format_round_trips() {
        assert_eq!(format_es_cl(1_234_567, 0, "$"), "$1.234.567");
        assert_eq!(format_es_cl(-45_990, 0, "$"), "-$45.990");
        assert_eq!(format_es_cl(0, 0, "$"), "$0");
        assert_eq!(format_es_cl(123_456, 2, "US$"), "US$1.234,56");
        assert_eq!(format_es_cl(5, 2, "US$"), "US$0,05");
        assert_eq!(format_es_cl(12_345, 4, "UF "), "UF 1,2345");
        for v in [0, 7, 999, 1_000, 1_234_567, -98_765] {
            let s = format_es_cl(v, 0, "$");
            assert_eq!(parse_es_cl(&s, Currency::CLP), Ok(v), "{s}");
        }
    }

    #[test]
    fn display_uses_currency_metadata() {
        assert_eq!(clp(27_990).display_es_cl(), "$27.990");
        assert_eq!(
            Money::new(1_999, Currency::USD.code).display_es_cl(),
            "US$19,99"
        );
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Shared expenses and installments must never create or lose a peso.
        #[test]
        fn allocate_preserves_total(
            minor in -1_000_000_000_000i64..1_000_000_000_000,
            weights in prop::collection::vec(0u64..1_000, 1..8),
        ) {
            prop_assume!(weights.iter().any(|&w| w > 0));
            let parts = Money::new(minor, Currency::CLP.code).allocate(&weights).unwrap();
            prop_assert_eq!(parts.len(), weights.len());
            prop_assert_eq!(parts.iter().map(|m| m.minor).sum::<i64>(), minor);
            prop_assert!(parts.iter().all(|m| m.minor == 0 || (m.minor < 0) == (minor < 0)));
        }

        /// Even splits differ by at most one minor unit.
        #[test]
        fn split_evenly_is_fair(minor in -1_000_000_000i64..1_000_000_000, n in 1usize..12) {
            let parts = Money::new(minor, Currency::CLP.code).split_evenly(n).unwrap();
            let max = parts.iter().map(|m| m.minor).max().unwrap();
            let min = parts.iter().map(|m| m.minor).min().unwrap();
            prop_assert!(max - min <= 1);
        }

        /// X-8/X-9: the UI re-sends `MoneyDto.display`, so every built-in currency's own
        /// display string must parse back to the same amount, across the whole `i64` range.
        #[test]
        fn display_round_trips_for_every_built_in_currency(
            minor in any::<i64>(),
            currency in prop::sample::select(Currency::BUILT_IN.to_vec()),
        ) {
            let s = format_es_cl(minor, currency.minor_units, currency.symbol);
            prop_assert_eq!(parse_es_cl(&s, currency), Ok(minor), "{}", s);
        }

        /// X-12: another currency's display string never parses for the target currency,
        /// so no marker of one built-in currency is consumed as another's.
        #[test]
        fn another_currencys_display_is_rejected(
            minor in any::<i64>(),
            shown in prop::sample::select(Currency::BUILT_IN.to_vec()),
            target in prop::sample::select(Currency::BUILT_IN.to_vec()),
        ) {
            prop_assume!(shown != target);
            let s = format_es_cl(minor, shown.minor_units, shown.symbol);
            prop_assert!(parse_es_cl(&s, target).is_err(), "{} parsed as {}", s, target.code);
        }
    }
}
