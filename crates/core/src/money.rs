//! Money amounts in BRL, stored as integer cents to avoid float drift.

use crate::entities::ValidationError;
use serde::{Deserialize, Serialize};

/// A monetary amount in Brazilian Reais, stored as integer cents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub struct Money {
    cents: i64,
}

impl Money {
    /// Creates a money value from an exact cents amount.
    pub const fn from_cents(cents: i64) -> Self {
        Self { cents }
    }

    /// Creates a money value from a Reais float, rounding to the nearest cent.
    pub fn from_brl(value: f64) -> Self {
        Self {
            cents: (value * 100.0).round() as i64,
        }
    }

    /// The amount in cents.
    pub const fn cents(&self) -> i64 {
        self.cents
    }

    /// Parses user input in the pt-BR money format into centavos (D11).
    ///
    /// Accepted (after trimming, an optional `R$` prefix, and an optional
    /// leading `-`):
    /// - `1234` / `1.234` / `1.234.567` — the dot is the thousands
    ///   separator, pt-BR style (groups of three; `12.34` is *not* accepted,
    ///   it is malformed pt-BR and ambiguous);
    /// - `1234,56` / `1234,5` / `,56` — the comma is the decimal separator
    ///   (1–2 digits).
    ///
    /// The dot is never a decimal separator: there is no pt-BR input that
    /// means "twelve reais and thirty-four centavos" other than `12,34`.
    /// Ambiguous or malformed input is rejected with a field error the UI
    /// can map to the form control (D12: pt-BR message).
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        let err = || ValidationError::new("value", "valor monetário inválido");
        let s = input.trim();
        let s = s.strip_prefix("R$").map(str::trim).unwrap_or(s);
        let (neg, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, s),
        };
        // sign before the currency prefix: "-R$ 10,00"
        let s = s.strip_prefix("R$").map(str::trim).unwrap_or(s);
        if s.is_empty() {
            return Err(err());
        }
        let (int_part, dec_part) = match s.split_once(',') {
            Some((_, d)) if !(1..=2).contains(&d.len()) => return Err(err()),
            Some((i, d)) => (i, d),
            None => (s, ""),
        };
        if !dec_part.chars().all(|c| c.is_ascii_digit()) {
            return Err(err());
        }
        let units: i64 = if int_part.is_empty() {
            0 // ",56" → 56 centavos
        } else {
            let groups: Vec<&str> = int_part.split('.').collect();
            for g in &groups {
                if g.is_empty() || !g.chars().all(|c| c.is_ascii_digit()) {
                    return Err(err());
                }
            }
            // pt-BR grouping (only when dots are present): first group
            // 1–3 digits, the rest exactly 3. Dotless integers are fine.
            if groups.len() > 1 && groups[0].len() > 3 {
                return Err(err());
            }
            if groups.iter().skip(1).any(|g| g.len() != 3) {
                return Err(err());
            }
            let digits: String = groups.iter().flat_map(|g| g.chars()).collect();
            if digits.len() > 16 {
                return Err(err()); // beyond i64 centavos range
            }
            digits.parse().map_err(|_| err())?
        };
        let dec = if dec_part.is_empty() {
            0
        } else if dec_part.len() == 1 {
            dec_part.parse::<i64>().unwrap_or(0) * 10
        } else {
            dec_part.parse::<i64>().unwrap_or(0)
        };
        let cents = units * 100 + dec;
        Ok(Self {
            cents: if neg { -cents } else { cents },
        })
    }

    /// Formats as a Brazilian Reais string, e.g. `R$ 1.234,56`.
    pub fn format_brl(&self) -> String {
        let sign = if self.cents < 0 { "-" } else { "" };
        let abs = self.cents.unsigned_abs();
        let units = abs / 100;
        let cents = abs % 100;

        let mut groups: Vec<String> = Vec::new();
        let mut rest = units;
        loop {
            groups.push(format!("{:03}", rest % 1000));
            rest /= 1000;
            if rest == 0 {
                break;
            }
        }

        let mut int_part = groups.pop().expect("at least one group");
        while let Some(group) = groups.pop() {
            int_part = format!("{int_part}.{group}");
        }
        int_part = int_part.trim_start_matches('0').to_string();
        if int_part.is_empty() {
            int_part = "0".to_string();
        }

        format!("{sign}R$ {int_part},{cents:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cents_round_trip() {
        assert_eq!(Money::from_cents(123456).cents(), 123456);
    }

    #[test]
    fn from_brl_rounds_to_nearest_cent() {
        assert_eq!(Money::from_brl(1234.56).cents(), 123456);
        assert_eq!(Money::from_brl(0.005).cents(), 1);
    }

    #[test]
    fn formats_brl_with_grouping() {
        assert_eq!(Money::from_cents(123456).format_brl(), "R$ 1.234,56");
        assert_eq!(Money::from_cents(0).format_brl(), "R$ 0,00");
        assert_eq!(Money::from_cents(-99).format_brl(), "-R$ 0,99");
        assert_eq!(Money::from_cents(100000000).format_brl(), "R$ 1.000.000,00");
    }

    #[test]
    fn parses_plain_integers() {
        assert_eq!(Money::parse("1234").unwrap().cents(), 123_400);
        assert_eq!(Money::parse("0").unwrap().cents(), 0);
        // dotless integers may be any length (within the overflow cap)
        assert_eq!(Money::parse("1234567").unwrap().cents(), 123_456_700);
    }

    #[test]
    fn parses_pt_br_decimal_comma() {
        assert_eq!(Money::parse("1234,56").unwrap().cents(), 123_456);
        assert_eq!(Money::parse("1234,5").unwrap().cents(), 123_450);
        assert_eq!(Money::parse(",56").unwrap().cents(), 56);
        assert_eq!(Money::parse("0,5").unwrap().cents(), 50);
    }

    #[test]
    fn dot_is_thousands_separator_never_decimal() {
        assert_eq!(Money::parse("1.234").unwrap().cents(), 123_400);
        assert_eq!(Money::parse("1.234.567").unwrap().cents(), 123_456_700);
    }

    #[test]
    fn parses_prefix_sign_and_spaces() {
        assert_eq!(Money::parse("R$ 1.234,56").unwrap().cents(), 123_456);
        assert_eq!(Money::parse("  100  ").unwrap().cents(), 10_000);
        assert_eq!(Money::parse("-R$ 10,00").unwrap().cents(), -1_000);
        assert_eq!(Money::parse("R$ -10").unwrap().cents(), -1_000);
        assert_eq!(Money::parse("-10").unwrap().cents(), -1_000);
    }

    #[test]
    fn parse_format_round_trips() {
        let original = Money::from_cents(123_456);
        assert_eq!(Money::parse(&original.format_brl()).unwrap(), original);
    }

    #[test]
    fn parses_reject_garbage_and_ambiguous_input() {
        for bad in [
            "",
            "   ",
            "abc",
            "12.34,56,78",
            "1,234",
            "1.234.56",
            "1.23,45",
            "12.34", // ambiguous: not valid pt-BR grouping
            ".",
            ",",
            "1.",
            ".5",
            "R$",
            "-",
            "1e5",
            "1.2.3",
        ] {
            assert!(
                Money::parse(bad).is_err(),
                "expected {bad:?} to be rejected"
            );
        }
    }

    #[test]
    fn parses_rejects_overflow() {
        assert!(Money::parse("12345678901234567").is_err()); // 17 digits
                                                             // 16 digits is the limit — fits in i64 centavos
        assert_eq!(
            Money::parse("9999999999999999").unwrap().cents(),
            999_999_999_999_999_900
        );
    }
}
