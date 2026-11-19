//! Statement import (see `docs/06-statement-ingestion.md`).
//!
//! M0 ships the description normalizer used for dedup fingerprints and payee rules.

/// Normalizes a bank description for matching: uppercase, accents folded, punctuation
/// and digit runs (dates, references, cuotas) collapsed, single spaces.
pub fn normalize_description(input: &str) -> String {
    // 1. Fold accents, uppercase, and split into letter runs / digit runs.
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_is_digit = false;
    for ch in input.chars().map(fold_accent) {
        let kind = if ch.is_ascii_alphabetic() {
            Some(false)
        } else if ch.is_ascii_digit() {
            Some(true)
        } else {
            None
        };
        match kind {
            Some(is_digit) if !current.is_empty() && is_digit == current_is_digit => {
                current.push(ch.to_ascii_uppercase());
            }
            Some(is_digit) => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                current.push(ch.to_ascii_uppercase());
                current_is_digit = is_digit;
            }
            None => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    // 2. Digit runs become `#`; consecutive `#` collapse (dates, cuotas "03/12", refs).
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    for t in tokens {
        let t = if t.bytes().all(|b| b.is_ascii_digit()) {
            "#".to_owned()
        } else {
            t
        };
        if !(t == "#" && out.last().is_some_and(|l| l == "#")) {
            out.push(t);
        }
    }
    out.join(" ")
}

fn fold_accent(c: char) -> char {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'Á' | 'À' | 'Ä' | 'Â' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'Ó' | 'Ò' | 'Ö' | 'Ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        'ñ' | 'Ñ' => 'n',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bank_descriptions() {
        assert_eq!(
            normalize_description("Compra  Jumbo Ñuñoa 12/11"),
            "COMPRA JUMBO NUNOA #"
        );
        assert_eq!(
            normalize_description("PAGO-AUTOMÁTICO luz.  cuota 03/12"),
            "PAGO AUTOMATICO LUZ CUOTA #"
        );
        assert_eq!(normalize_description("   "), "");
        assert_eq!(normalize_description("ABC123DEF"), "ABC # DEF");
    }

    #[test]
    fn same_merchant_same_key() {
        assert_eq!(
            normalize_description("UBER *EATS 8821"),
            normalize_description("Uber Eats 1190")
        );
    }
}
