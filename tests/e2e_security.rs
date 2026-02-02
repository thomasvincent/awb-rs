use awb_security::credential::{CredentialPort, InMemoryCredentialStore};
use awb_security::redact_secrets;

#[test]
fn test_credential_store_roundtrip_in_memory() {
    let store = InMemoryCredentialStore::new();

    store.set_password("test_user", "secret_password_123").unwrap();
    store.set_password("api_key", "sk-1234567890abcdef").unwrap();
    store.set_password("oauth_client", "oauth_secret_xyz").unwrap();

    let password = store.get_password("test_user").unwrap();
    assert_eq!(password, "secret_password_123");

    let token = store.get_password("api_key").unwrap();
    assert_eq!(token, "sk-1234567890abcdef");

    let secret = store.get_password("oauth_client").unwrap();
    assert_eq!(secret, "oauth_secret_xyz");
}

#[test]
fn test_credential_store_not_found() {
    let store = InMemoryCredentialStore::new();
    let result = store.get_password("nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_credential_store_delete() {
    let store = InMemoryCredentialStore::new();

    store.set_password("user", "secret").unwrap();
    assert!(store.get_password("user").is_ok());

    store.delete_password("user").unwrap();
    assert!(store.get_password("user").is_err());
}

#[test]
fn test_credential_store_overwrite() {
    let store = InMemoryCredentialStore::new();

    store.set_password("user", "original_secret").unwrap();
    store.set_password("user", "new_secret").unwrap();

    let retrieved = store.get_password("user").unwrap();
    assert_eq!(retrieved, "new_secret");
}

#[test]
fn test_secret_redaction_basic() {
    let text = "password=my_secret_pass123 token=abc456";
    let redacted = redact_secrets(text, &["my_secret_pass123", "abc456"]);
    assert!(!redacted.contains("my_secret_pass123"));
    assert!(!redacted.contains("abc456"));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn test_secret_redaction_multiple_secrets() {
    let text = "key1=secret1 key2=secret2 key3=secret3";
    let redacted = redact_secrets(text, &["secret1", "secret2", "secret3"]);
    assert!(!redacted.contains("secret1"));
    assert!(!redacted.contains("secret2"));
    assert!(!redacted.contains("secret3"));
    assert_eq!(redacted.matches("[REDACTED]").count(), 3);
}

#[test]
fn test_secret_redaction_preserves_safe_content() {
    let text = "username=john_doe status=active count=42";
    let redacted = redact_secrets(text, &["some_other_secret"]);
    assert_eq!(redacted, text);
}

#[test]
fn test_secret_redaction_empty_and_whitespace() {
    assert_eq!(redact_secrets("", &["secret"]), "");
    assert_eq!(redact_secrets("   ", &["secret"]), "   ");
    assert_eq!(redact_secrets("\n\n", &["secret"]), "\n\n");
}

#[test]
fn test_secret_redaction_no_secrets() {
    let text = "This is a normal message with no secrets.";
    let redacted = redact_secrets(text, &[]);
    assert_eq!(redacted, text);
}

#[test]
fn test_secret_redaction_empty_secret_ignored() {
    let text = "safe text";
    let redacted = redact_secrets(text, &[""]);
    assert_eq!(redacted, "safe text");
}

#[test]
fn test_credential_isolation() {
    let store = InMemoryCredentialStore::new();

    store.set_password("service_a", "secret_a").unwrap();
    store.set_password("service_b", "secret_b").unwrap();

    let secret_a = store.get_password("service_a").unwrap();
    let secret_b = store.get_password("service_b").unwrap();

    assert_eq!(secret_a, "secret_a");
    assert_eq!(secret_b, "secret_b");
    assert_ne!(secret_a, secret_b);
}

#[test]
fn test_oauth_token_roundtrip() {
    let store = InMemoryCredentialStore::new();

    let token_json = r#"{"access_token":"abc123","refresh_token":"xyz789"}"#;
    store.set_oauth_token("profile1", token_json).unwrap();

    let retrieved = store.get_oauth_token("profile1").unwrap();
    assert_eq!(retrieved, token_json);

    store.delete_oauth_token("profile1").unwrap();
    assert!(store.get_oauth_token("profile1").is_err());
}

#[test]
fn test_redact_secrets_in_debug_output() {
    let store = InMemoryCredentialStore::new();
    store.set_password("wiki_bot", "super_secret_pw").unwrap();

    let password = store.get_password("wiki_bot").unwrap();
    let debug_msg = format!("Logging in with password: {}", password);
    let redacted = redact_secrets(&debug_msg, &[&password]);

    assert!(!redacted.contains("super_secret_pw"));
    assert!(redacted.contains("[REDACTED]"));
}

#[test]
fn test_credential_store_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(InMemoryCredentialStore::new());
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = thread::spawn(move || {
            let key = format!("service_{}", i);
            let secret = format!("secret_{}", i);
            store_clone.set_password(&key, &secret).unwrap();
            let retrieved = store_clone.get_password(&key).unwrap();
            assert_eq!(retrieved, secret);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    for i in 0..10 {
        let key = format!("service_{}", i);
        let secret = format!("secret_{}", i);
        let retrieved = store.get_password(&key).unwrap();
        assert_eq!(retrieved, secret);
    }
}

#[test]
fn test_canary_redaction_in_telemetry_event() {
    // Simulate a telemetry event that accidentally contains a known secret
    let canary_secret = "sk-super-secret-api-key-12345";
    let telemetry_event = format!(
        r#"{{"event":"api_call","url":"https://en.wikipedia.org/w/api.php","headers":{{"Authorization":"Bearer {}"}}, "status":200,"duration_ms":150}}"#,
        canary_secret
    );

    // Pass through redact_secrets and verify the secret is gone
    let redacted = redact_secrets(&telemetry_event, &[canary_secret]);

    assert!(
        !redacted.contains(canary_secret),
        "Canary secret was NOT redacted from telemetry event: {}",
        redacted
    );
    assert!(
        redacted.contains("[REDACTED]"),
        "Redacted marker missing from output: {}",
        redacted
    );

    // Verify the rest of the event structure is preserved
    assert!(redacted.contains("api_call"));
    assert!(redacted.contains("en.wikipedia.org"));
    assert!(redacted.contains("200"));
}

#[test]
fn test_canary_redaction_multiple_occurrences() {
    let secret = "oauth-token-xyz789";
    let text = format!(
        "request token={} response contained token={} in body",
        secret, secret
    );

    let redacted = redact_secrets(&text, &[secret]);

    assert!(
        !redacted.contains(secret),
        "Secret still present after redaction"
    );
    assert_eq!(
        redacted.matches("[REDACTED]").count(),
        2,
        "Expected exactly 2 redactions"
    );
}
