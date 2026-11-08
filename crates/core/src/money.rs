//! Money amounts in BRL, stored as integer cents to avoid float drift.

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
}
