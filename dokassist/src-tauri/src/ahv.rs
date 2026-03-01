use crate::error::AppError;

/// Validates and normalizes Swiss AHV/AVS number
/// Accepts both dotted (756.XXXX.XXXX.XX) and plain (13-digit) formats
/// Returns normalized dotted format
pub fn validate_ahv(ahv: &str) -> Result<String, AppError> {
    // Remove dots and spaces
    let digits: String = ahv.chars().filter(|c| c.is_ascii_digit()).collect();

    // Must be exactly 13 digits
    if digits.len() != 13 {
        return Err(AppError::Validation(format!(
            "AHV number must be 13 digits, got {}",
            digits.len()
        )));
    }

    // Must start with 756 (Swiss country code)
    if !digits.starts_with("756") {
        return Err(AppError::Validation(
            "AHV number must start with 756 (Swiss country code)".to_string(),
        ));
    }

    // Validate checksum (EAN-13 algorithm)
    let check_digit = digits.chars().nth(12).unwrap().to_digit(10).unwrap();
    let calculated = calculate_ahv_checksum(&digits[..12])?;

    if check_digit != calculated {
        return Err(AppError::Validation(format!(
            "Invalid AHV checksum: expected {}, got {}",
            calculated, check_digit
        )));
    }

    // Format as dotted: 756.XXXX.XXXX.XX
    Ok(format!(
        "{}.{}.{}.{}",
        &digits[0..3],
        &digits[3..7],
        &digits[7..11],
        &digits[11..13]
    ))
}

/// Calculate EAN-13 checksum for AHV number
fn calculate_ahv_checksum(digits: &str) -> Result<u32, AppError> {
    if digits.len() != 12 {
        return Err(AppError::Validation(
            "Checksum calculation requires 12 digits".to_string(),
        ));
    }

    let sum: u32 = digits
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let digit = c.to_digit(10).unwrap();
            if i % 2 == 0 {
                digit
            } else {
                digit * 3
            }
        })
        .sum();

    let checksum = (10 - (sum % 10)) % 10;
    Ok(checksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_ahv_valid_dotted() {
        let result = validate_ahv("756.1234.5678.97").unwrap();
        assert_eq!(result, "756.1234.5678.97");
    }

    #[test]
    fn test_validate_ahv_valid_plain() {
        let result = validate_ahv("7561234567897").unwrap();
        assert_eq!(result, "756.1234.5678.97");
    }

    #[test]
    fn test_validate_ahv_invalid_length() {
        let result = validate_ahv("756123456789");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_ahv_invalid_country_code() {
        let result = validate_ahv("7551234567897");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_ahv_invalid_checksum() {
        let result = validate_ahv("7561234567898");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_ahv_with_spaces() {
        let result = validate_ahv("756 1234 5678 97").unwrap();
        assert_eq!(result, "756.1234.5678.97");
    }
}
