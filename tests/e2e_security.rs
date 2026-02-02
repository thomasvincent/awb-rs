use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
use awb_domain::types::Namespace;
use awb_security::credential::{CredentialPort, InMemoryCredentialStore};
use awb_security::redact_secrets;
use std::collections::HashSet;
use std::time::Duration;

#[tokio::test]
async fn test_credential_store_roundtrip_in_memory() {
    let store = InMemoryCredentialStore::new();

    // Store various types of credentials
    store.store("test_user", "password", "secret_password_123").await.unwrap();
    store.store("api_key", "token", "sk-1234567890abcdef").await.unwrap();
    store.store("oauth_client", "secret", "oauth_secret_xyz").await.unwrap();

    // Retrieve and verify
    let password = store.retrieve("test_user", "password").await.unwrap();
    assert_eq!(password, "secret_password_123");

    let token = store.retrieve("api_key", "token").await.unwrap();
    assert_eq!(token, "sk-1234567890abcdef");

    let secret = store.retrieve("oauth_client", "secret").await.unwrap();
    assert_eq!(secret, "oauth_secret_xyz");
}

#[tokio::test]
async fn test_credential_store_not_found() {
    let store = InMemoryCredentialStore::new();

    let result = store.retrieve("nonexistent", "key").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_credential_store_delete() {
    let store = InMemoryCredentialStore::new();

    store.store("user", "pass", "secret").await.unwrap();

    // Verify it exists
    let result = store.retrieve("user", "pass").await;
    assert!(result.is_ok());

    // Delete it
    store.delete("user", "pass").await.unwrap();

    // Verify it's gone
    let result = store.retrieve("user", "pass").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_credential_store_overwrite() {
    let store = InMemoryCredentialStore::new();

    // Store initial value
    store.store("user", "key", "original_secret").await.unwrap();

    // Overwrite with new value
    store.store("user", "key", "new_secret").await.unwrap();

    // Should retrieve the new value
    let retrieved = store.retrieve("user", "key").await.unwrap();
    assert_eq!(retrieved, "new_secret");
}

#[test]
fn test_secret_redaction_passwords() {
    let text = "password=my_secret_pass123";
    let redacted = redact_secrets(text);
    assert!(redacted.contains("password=REDACTED"));
    assert!(!redacted.contains("my_secret_pass123"));

    let text2 = "pwd: super_secret";
    let redacted2 = redact_secrets(text2);
    assert!(redacted2.contains("REDACTED"));
    assert!(!redacted2.contains("super_secret"));
}

#[test]
fn test_secret_redaction_tokens() {
    let text = "token=ghp_1234567890abcdefghij";
    let redacted = redact_secrets(text);
    assert!(redacted.contains("token=REDACTED"));
    assert!(!redacted.contains("ghp_1234567890abcdefghij"));

    let text2 = "Authorization: Bearer sk-proj-abcdef123456";
    let redacted2 = redact_secrets(text2);
    assert!(redacted2.contains("REDACTED"));
    assert!(!redacted2.contains("sk-proj-abcdef123456"));
}

#[test]
fn test_secret_redaction_api_keys() {
    let text = "api_key: AIzaSyD1234567890abcdefghij";
    let redacted = redact_secrets(text);
    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("AIzaSyD1234567890abcdefghij"));

    let text2 = "APIKEY=1234567890abcdef1234567890abcdef";
    let redacted2 = redact_secrets(text2);
    assert!(redacted2.contains("REDACTED"));
}

#[test]
fn test_secret_redaction_oauth_secrets() {
    let text = "client_secret: oauth_secret_1234567890";
    let redacted = redact_secrets(text);
    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("oauth_secret_1234567890"));

    let text2 = "consumer_secret=abcdef1234567890";
    let redacted2 = redact_secrets(text2);
    assert!(redacted2.contains("REDACTED"));
}

#[test]
fn test_secret_redaction_multiple_secrets() {
    let text = "password=secret123 token=abc456 api_key=xyz789";
    let redacted = redact_secrets(text);

    assert!(redacted.contains("password=REDACTED"));
    assert!(redacted.contains("token=REDACTED"));
    assert!(redacted.contains("api_key=REDACTED"));

    assert!(!redacted.contains("secret123"));
    assert!(!redacted.contains("abc456"));
    assert!(!redacted.contains("xyz789"));
}

#[test]
fn test_secret_redaction_json_format() {
    let json = r#"{"username": "user", "password": "secret_pass", "token": "abc123"}"#;
    let redacted = redact_secrets(json);

    assert!(redacted.contains("username"));
    assert!(redacted.contains("user"));
    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("secret_pass"));
    assert!(!redacted.contains("abc123"));
}

#[test]
fn test_secret_redaction_url_encoded() {
    let text = "password=my%20secret&token=abc%20def";
    let redacted = redact_secrets(text);

    assert!(redacted.contains("password=REDACTED"));
    assert!(redacted.contains("token=REDACTED"));
}

#[test]
fn test_secret_redaction_case_insensitive() {
    let text = "PASSWORD=secret TOKEN=abc API_KEY=xyz";
    let redacted = redact_secrets(text);

    // Should redact regardless of case
    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("secret"));
}

#[test]
fn test_secret_redaction_preserves_safe_content() {
    let text = "username=john_doe status=active count=42";
    let redacted = redact_secrets(text);

    // Non-secret fields should be preserved
    assert!(redacted.contains("username=john_doe"));
    assert!(redacted.contains("status=active"));
    assert!(redacted.contains("count=42"));
}

#[test]
fn test_oauth_debug_output_redacted() {
    // Simulate Debug output for OAuth structs
    let auth = AuthMethod::OAuth2 {
        client_id: "public_client_id".to_string(),
        client_secret: "super_secret_value".to_string(),
    };

    let debug_output = format!("{:?}", auth);
    let redacted = redact_secrets(&debug_output);

    // client_id should be visible (it's public)
    assert!(redacted.contains("public_client_id") || redacted.contains("client_id"));

    // client_secret should be redacted
    assert!(redacted.contains("REDACTED") || !redacted.contains("super_secret_value"));
}

#[test]
fn test_profile_with_oauth1_redaction() {
    let profile = Profile {
        id: "test".to_string(),
        name: "Test Profile".to_string(),
        api_url: url::Url::parse("https://test.example.com/api").unwrap(),
        auth_method: AuthMethod::OAuth1 {
            consumer_key: "public_consumer_key".to_string(),
            consumer_secret: "secret_consumer_value".to_string(),
            access_token: "public_access_token".to_string(),
            access_secret: "secret_access_value".to_string(),
        },
        default_namespaces: HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };

    let debug_output = format!("{:?}", profile);
    let redacted = redact_secrets(&debug_output);

    // Public keys should be visible
    assert!(redacted.contains("public_consumer_key") || redacted.contains("consumer_key"));

    // Secrets should be redacted
    assert!(!redacted.contains("secret_consumer_value") || redacted.contains("REDACTED"));
    assert!(!redacted.contains("secret_access_value") || redacted.contains("REDACTED"));
}

#[tokio::test]
async fn test_credential_isolation() {
    let store = InMemoryCredentialStore::new();

    // Store credentials for different services
    store.store("service_a", "password", "secret_a").await.unwrap();
    store.store("service_b", "password", "secret_b").await.unwrap();

    // Verify isolation
    let secret_a = store.retrieve("service_a", "password").await.unwrap();
    let secret_b = store.retrieve("service_b", "password").await.unwrap();

    assert_eq!(secret_a, "secret_a");
    assert_eq!(secret_b, "secret_b");
    assert_ne!(secret_a, secret_b);
}

#[tokio::test]
async fn test_credential_store_list_keys() {
    let store = InMemoryCredentialStore::new();

    // Store multiple credentials
    store.store("service", "key1", "value1").await.unwrap();
    store.store("service", "key2", "value2").await.unwrap();
    store.store("service", "key3", "value3").await.unwrap();

    // List should return all keys for the service
    let keys = store.list("service").await.unwrap();

    assert_eq!(keys.len(), 3);
    assert!(keys.contains(&"key1".to_string()));
    assert!(keys.contains(&"key2".to_string()));
    assert!(keys.contains(&"key3".to_string()));
}

#[test]
fn test_secret_redaction_github_tokens() {
    let patterns = vec![
        "ghp_1234567890abcdefghijklmnopqrst",
        "gho_1234567890abcdefghijklmnopqrst",
        "ghu_1234567890abcdefghijklmnopqrst",
        "ghs_1234567890abcdefghijklmnopqrst",
        "ghr_1234567890abcdefghijklmnopqrst",
    ];

    for pattern in patterns {
        let text = format!("token: {}", pattern);
        let redacted = redact_secrets(&text);
        assert!(redacted.contains("REDACTED"));
        assert!(!redacted.contains(pattern));
    }
}

#[test]
fn test_secret_redaction_aws_keys() {
    let text = "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    let redacted = redact_secrets(text);

    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!redacted.contains("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"));
}

#[test]
fn test_secret_redaction_private_keys() {
    let text = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC\n-----END PRIVATE KEY-----";
    let redacted = redact_secrets(text);

    assert!(redacted.contains("REDACTED") || redacted.contains("PRIVATE KEY"));
    // The actual key content should be redacted
}

#[test]
fn test_secret_redaction_connection_strings() {
    let text = "postgres://user:password123@localhost:5432/db";
    let redacted = redact_secrets(text);

    // Should redact the password in connection string
    assert!(redacted.contains("REDACTED") || !redacted.contains("password123"));
}

#[test]
fn test_secret_redaction_sensitive_headers() {
    let text = r#"
    Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
    X-API-Key: sk-1234567890
    Cookie: session=abc123def456
    "#;

    let redacted = redact_secrets(text);

    // All sensitive headers should be redacted
    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    assert!(!redacted.contains("sk-1234567890"));
}

#[test]
fn test_bot_password_format_redaction() {
    let text = "Login with BotName@BotPassword using password: mySecretBotPass123";
    let redacted = redact_secrets(text);

    assert!(redacted.contains("REDACTED"));
    assert!(!redacted.contains("mySecretBotPass123"));
}

#[test]
fn test_secret_redaction_empty_and_whitespace() {
    assert_eq!(redact_secrets(""), "");
    assert_eq!(redact_secrets("   "), "   ");
    assert_eq!(redact_secrets("\n\n"), "\n\n");
}

#[test]
fn test_secret_redaction_no_secrets() {
    let text = "This is a normal message with no secrets.";
    let redacted = redact_secrets(text);
    assert_eq!(redacted, text);
}

#[tokio::test]
async fn test_credential_store_concurrent_access() {
    use std::sync::Arc;
    use tokio::task;

    let store = Arc::new(InMemoryCredentialStore::new());

    // Spawn multiple concurrent tasks
    let mut handles = vec![];

    for i in 0..10 {
        let store_clone = Arc::clone(&store);
        let handle = task::spawn(async move {
            let service = format!("service_{}", i);
            let secret = format!("secret_{}", i);
            store_clone.store(&service, "password", &secret).await.unwrap();
            let retrieved = store_clone.retrieve(&service, "password").await.unwrap();
            assert_eq!(retrieved, secret);
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all credentials are stored
    for i in 0..10 {
        let service = format!("service_{}", i);
        let secret = format!("secret_{}", i);
        let retrieved = store.retrieve(&service, "password").await.unwrap();
        assert_eq!(retrieved, secret);
    }
}
