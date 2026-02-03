/// Minimum secret length to avoid false-positive redaction of short substrings.
const MIN_SECRET_LEN: usize = 8;

/// Redacts known secrets from a string, replacing them with [REDACTED].
///
/// Secrets shorter than 8 characters are skipped to avoid false positives
/// (e.g., a 2-char secret matching random substrings throughout the text).
pub fn redact_secrets(input: &str, secrets: &[&str]) -> String {
    let mut result = input.to_string();
    for secret in secrets {
        if secret.len() >= MIN_SECRET_LEN {
            result = result.replace(secret, "[REDACTED]");
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction() {
        let input = "token=abc12345&password=secret456789";
        let result = redact_secrets(input, &["abc12345", "secret456789"]);
        assert_eq!(result, "token=[REDACTED]&password=[REDACTED]");
    }

    #[test]
    fn test_empty_secret() {
        let input = "safe text";
        let result = redact_secrets(input, &[""]);
        assert_eq!(result, "safe text");
    }

    #[test]
    fn test_short_secret_skipped() {
        // Secrets shorter than 8 chars are skipped to avoid false positives
        let input = "token=ab&key=cd";
        let result = redact_secrets(input, &["ab", "cd"]);
        assert_eq!(result, input, "Short secrets should not be redacted");
    }

    #[test]
    fn test_exactly_min_length_secret() {
        let input = "key=12345678";
        let result = redact_secrets(input, &["12345678"]);
        assert_eq!(result, "key=[REDACTED]");
    }
}
