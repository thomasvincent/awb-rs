/// Redacts known secrets from a string, replacing them with [REDACTED].
pub fn redact_secrets(input: &str, secrets: &[&str]) -> String {
    let mut result = input.to_string();
    for secret in secrets {
        if !secret.is_empty() {
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
        let input = "token=abc123&password=secret456";
        let result = redact_secrets(input, &["abc123", "secret456"]);
        assert_eq!(result, "token=[REDACTED]&password=[REDACTED]");
    }

    #[test]
    fn test_empty_secret() {
        let input = "safe text";
        let result = redact_secrets(input, &[""]);
        assert_eq!(result, "safe text");
    }
}
