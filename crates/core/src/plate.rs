//! License-plate normalization. Full validation rules land in T1-2.

/// Normalizes a plate: trims surrounding whitespace and uppercases ASCII.
/// `ab12c34` → `AB12C34`.
pub fn normalize_plate(raw: &str) -> String {
    raw.trim().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lowercase() {
        assert_eq!(normalize_plate("ab12c34"), "AB12C34");
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(normalize_plate("  a1b2c3 \n"), "A1B2C3");
    }

    #[test]
    fn mixed_case() {
        assert_eq!(normalize_plate("aB12cD3"), "AB12CD3");
    }
}
