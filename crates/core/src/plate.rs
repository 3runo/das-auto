//! License-plate normalization and validation (T1-2).
//!
//! Brazilian plates: 3 letters + 4 digits (old format) or ABC1D23 (Mercosul),
//! always 7 chars. The plan allows up to 7 chars of normalized uppercase
//! alphanumerics without spaces (plan.md §4).

use crate::entities::ValidationError;

/// Normalizes a plate: strips all whitespace (including internal) and
/// uppercases ASCII. `ab 12c34` → `AB12C34`.
pub fn normalize_plate(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

/// Validates a plate after normalization: 1..=7 chars, ASCII alphanumerics.
pub fn validate_plate(plate: &str) -> Result<(), ValidationError> {
    let normalized = normalize_plate(plate);
    if normalized.is_empty() {
        return Err(ValidationError::new("plate", "placa é obrigatória"));
    }
    if normalized.len() > 7 {
        return Err(ValidationError::new(
            "plate",
            "placa deve ter no máximo 7 caracteres",
        ));
    }
    if !normalized
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c.is_ascii_digit())
    {
        return Err(ValidationError::new(
            "plate",
            "placa deve conter apenas letras e números",
        ));
    }
    Ok(())
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
    fn strips_internal_spaces() {
        assert_eq!(normalize_plate("ab 12c34"), "AB12C34");
        assert_eq!(normalize_plate("a b 1 2 c 3 4"), "AB12C34");
    }

    #[test]
    fn mixed_case() {
        assert_eq!(normalize_plate("aB12cD3"), "AB12CD3");
    }

    #[test]
    fn all_whitespace_becomes_empty() {
        assert_eq!(normalize_plate("  \t\n"), "");
    }

    #[test]
    fn validates_mercosul_plate() {
        validate_plate("ABC1D23").unwrap();
    }

    #[test]
    fn validates_old_format_plate() {
        validate_plate("AB12C34").unwrap();
    }

    #[test]
    fn validates_after_normalization() {
        validate_plate("  ab 12c34 ").unwrap();
    }

    #[test]
    fn rejects_empty_plate() {
        assert_eq!(validate_plate("").unwrap_err().field(), "plate");
        assert_eq!(validate_plate("   ").unwrap_err().field(), "plate");
    }

    #[test]
    fn rejects_plate_longer_than_seven_chars() {
        assert_eq!(validate_plate("AB12C345").unwrap_err().field(), "plate");
    }

    #[test]
    fn rejects_non_alphanumeric_plate() {
        assert_eq!(validate_plate("AB12C!4").unwrap_err().field(), "plate");
        // non-ASCII letter (UTF-8 multi-byte) must not slip through
        assert_eq!(validate_plate("AB12Ç34").unwrap_err().field(), "plate");
    }
}
