use awb_domain::profile::{AuthMethod, Profile, ThrottlePolicy};
use awb_domain::rules::{Rule, RuleSet};
use awb_domain::session::{SessionState, SkipCondition};
use awb_domain::types::Namespace;
use awb_storage::config_store::{Preferences, TomlConfigStore};
use awb_storage::session_store::{JsonSessionStore, SessionStore};
use std::collections::HashSet;
use std::time::Duration;
use tempfile::TempDir;

#[tokio::test]
async fn test_json_session_store_save_and_load() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    // Create a session with some data
    let mut session = SessionState::new("test_profile");
    session.page_list = vec![
        awb_domain::types::Title::new(Namespace::MAIN, "Page1"),
        awb_domain::types::Title::new(Namespace::MAIN, "Page2"),
    ];
    session.current_index = 1;

    let mut ruleset = RuleSet::new();
    ruleset.add(Rule::new_plain("old", "new", true));
    session.rule_set = ruleset;

    // Save the session
    store.save(&session).await.unwrap();

    // Load it back
    let loaded = store.load(&session.session_id).await.unwrap();

    // Verify all fields
    assert_eq!(loaded.session_id, session.session_id);
    assert_eq!(loaded.profile_id, session.profile_id);
    assert_eq!(loaded.page_list.len(), 2);
    assert_eq!(loaded.current_index, 1);
    assert_eq!(loaded.rule_set.rules.len(), 1);
}

#[tokio::test]
async fn test_json_session_store_crash_safe_write() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    let session = SessionState::new("test_profile");

    // Save session
    store.save(&session).await.unwrap();

    // Check that temp file is cleaned up after successful write
    let temp_path = temp_dir
        .path()
        .join(format!("{}.json.tmp", session.session_id));
    assert!(
        !temp_path.exists(),
        "Temp file should be cleaned up after successful write"
    );

    // Check that final file exists
    let final_path = temp_dir.path().join(format!("{}.json", session.session_id));
    assert!(final_path.exists(), "Final file should exist");
}

#[tokio::test]
async fn test_json_session_store_list_sessions() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    // Save multiple sessions
    let session1 = SessionState::new("profile1");
    let session2 = SessionState::new("profile2");
    let session3 = SessionState::new("profile3");

    store.save(&session1).await.unwrap();
    store.save(&session2).await.unwrap();
    store.save(&session3).await.unwrap();

    // List all sessions
    let sessions = store.list_sessions().await.unwrap();

    assert_eq!(sessions.len(), 3);
    assert!(sessions.contains(&session1.session_id));
    assert!(sessions.contains(&session2.session_id));
    assert!(sessions.contains(&session3.session_id));
}

#[tokio::test]
async fn test_json_session_store_delete() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    let session = SessionState::new("test_profile");
    store.save(&session).await.unwrap();

    // Verify it exists
    let loaded = store.load(&session.session_id).await;
    assert!(loaded.is_ok());

    // Delete it
    store.delete(&session.session_id).await.unwrap();

    // Verify it's gone
    let loaded = store.load(&session.session_id).await;
    assert!(loaded.is_err());
}

#[tokio::test]
async fn test_json_session_store_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    let result = store.load("nonexistent").await;
    assert!(result.is_err());
}

#[test]
fn test_toml_config_store_preferences() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let store = TomlConfigStore::new(&config_path);

    // Create custom preferences
    let prefs = Preferences {
        default_profile: "enwiki".to_string(),
        theme: "dark".to_string(),
        diff_mode: "unified".to_string(),
        diff_context_lines: 5,
        auto_save_interval_secs: 60,
        confirm_large_change_threshold: 1000,
        log_level: "debug".to_string(),
    };

    // Save preferences
    store.save_preferences(&prefs).unwrap();

    // Verify file exists
    assert!(config_path.exists());

    // Load preferences back
    let loaded = store.load_preferences().unwrap();

    assert_eq!(loaded.default_profile, "enwiki");
    assert_eq!(loaded.theme, "dark");
    assert_eq!(loaded.diff_mode, "unified");
    assert_eq!(loaded.diff_context_lines, 5);
    assert_eq!(loaded.auto_save_interval_secs, 60);
    assert_eq!(loaded.confirm_large_change_threshold, 1000);
    assert_eq!(loaded.log_level, "debug");
}

#[test]
fn test_toml_config_store_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let store = TomlConfigStore::new(&config_path);

    // Create a profile
    let mut namespaces = HashSet::new();
    namespaces.insert(Namespace::MAIN);

    let profile = Profile {
        id: "enwiki".to_string(),
        name: "English Wikipedia".to_string(),
        api_url: url::Url::parse("https://en.wikipedia.org/w/api.php").unwrap(),
        auth_method: AuthMethod::BotPassword {
            username: "TestBot".to_string(),
        },
        default_namespaces: namespaces,
        throttle_policy: ThrottlePolicy {
            min_edit_interval: Duration::from_secs(12),
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        },
    };

    // Save profile
    store.save_profile(&profile).unwrap();

    // Load it back
    let loaded = store.load_profile("enwiki").unwrap();

    assert_eq!(loaded.id, "enwiki");
    assert_eq!(loaded.name, "English Wikipedia");
    assert_eq!(
        loaded.api_url.as_str(),
        "https://en.wikipedia.org/w/api.php"
    );
    assert!(loaded.default_namespaces.contains(&Namespace::MAIN));
}

#[test]
fn test_toml_config_store_list_profiles() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let store = TomlConfigStore::new(&config_path);

    // Save multiple profiles
    let profile1 = Profile {
        id: "enwiki".to_string(),
        name: "English Wikipedia".to_string(),
        api_url: url::Url::parse("https://en.wikipedia.org/w/api.php").unwrap(),
        auth_method: AuthMethod::BotPassword {
            username: "Bot1".to_string(),
        },
        default_namespaces: HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };

    let profile2 = Profile {
        id: "dewiki".to_string(),
        name: "German Wikipedia".to_string(),
        api_url: url::Url::parse("https://de.wikipedia.org/w/api.php").unwrap(),
        auth_method: AuthMethod::BotPassword {
            username: "Bot2".to_string(),
        },
        default_namespaces: HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };

    store.save_profile(&profile1).unwrap();
    store.save_profile(&profile2).unwrap();

    // List profiles
    let profiles = store.list_profiles().unwrap();

    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().any(|p| p.id == "enwiki"));
    assert!(profiles.iter().any(|p| p.id == "dewiki"));
}

#[tokio::test]
async fn test_combined_session_and_config_storage() {
    let temp_dir = TempDir::new().unwrap();
    let session_store = JsonSessionStore::new(temp_dir.path().join("sessions"));
    let config_store = TomlConfigStore::new(temp_dir.path().join("config.toml"));

    // Create and save a profile
    let profile = Profile {
        id: "test_profile".to_string(),
        name: "Test Profile".to_string(),
        api_url: url::Url::parse("https://test.wikipedia.org/w/api.php").unwrap(),
        auth_method: AuthMethod::BotPassword {
            username: "TestBot".to_string(),
        },
        default_namespaces: HashSet::new(),
        throttle_policy: ThrottlePolicy::default(),
    };
    config_store.save_profile(&profile).unwrap();

    // Create and save a session using this profile
    let session = SessionState::new("test_profile");
    session_store.save(&session).await.unwrap();

    // Load both back and verify they're linked
    let loaded_profile = config_store.load_profile("test_profile").unwrap();
    let loaded_session = session_store.load(&session.session_id).await.unwrap();

    assert_eq!(loaded_session.profile_id, loaded_profile.id);
}

#[test]
fn test_toml_config_store_file_permissions() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    let store = TomlConfigStore::new(&config_path);

    let prefs = Preferences::default();
    store.save_preferences(&prefs).unwrap();

    // On Unix, verify file has restrictive permissions (0600)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(&config_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "Config file should have 0600 permissions"
        );
    }
}

#[tokio::test]
async fn test_session_with_skip_conditions() {
    let temp_dir = TempDir::new().unwrap();
    let store = JsonSessionStore::new(temp_dir.path());

    let mut session = SessionState::new("test_profile");

    // Add skip conditions
    let mut allowed_namespaces = HashSet::new();
    allowed_namespaces.insert(Namespace::MAIN);
    allowed_namespaces.insert(Namespace::USER);

    session.skip_conditions = vec![
        SkipCondition::Namespace {
            allowed: allowed_namespaces,
        },
        SkipCondition::PageSize {
            min_bytes: Some(100),
            max_bytes: Some(10000),
        },
        SkipCondition::IsRedirect(true),
    ];

    // Save and reload
    store.save(&session).await.unwrap();
    let loaded = store.load(&session.session_id).await.unwrap();

    assert_eq!(loaded.skip_conditions.len(), 3);
}

#[test]
fn test_preferences_default_values() {
    let prefs = Preferences::default();

    assert_eq!(prefs.default_profile, "enwiki");
    assert_eq!(prefs.theme, "system");
    assert_eq!(prefs.diff_mode, "side-by-side");
    assert_eq!(prefs.diff_context_lines, 3);
    assert_eq!(prefs.auto_save_interval_secs, 30);
    assert_eq!(prefs.confirm_large_change_threshold, 500);
    assert_eq!(prefs.log_level, "info");
}
