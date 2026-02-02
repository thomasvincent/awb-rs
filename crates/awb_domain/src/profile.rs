use crate::types::Namespace;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub api_url: url::Url,
    pub auth_method: AuthMethod,
    pub default_namespaces: HashSet<Namespace>,
    pub throttle_policy: ThrottlePolicy,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    BotPassword {
        username: String,
    },
    OAuth1 {
        consumer_key: String,
        consumer_secret: String,
        access_token: String,
        access_secret: String,
    },
    OAuth2 {
        client_id: String,
        client_secret: String,
    },
}

impl std::fmt::Debug for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthMethod::BotPassword { username } => f
                .debug_struct("BotPassword")
                .field("username", username)
                .finish(),
            AuthMethod::OAuth1 {
                consumer_key,
                consumer_secret: _,
                access_token,
                access_secret: _,
            } => f
                .debug_struct("OAuth1")
                .field("consumer_key", consumer_key)
                .field("consumer_secret", &"***REDACTED***")
                .field("access_token", access_token)
                .field("access_secret", &"***REDACTED***")
                .finish(),
            AuthMethod::OAuth2 {
                client_id,
                client_secret: _,
            } => f
                .debug_struct("OAuth2")
                .field("client_id", client_id)
                .field("client_secret", &"***REDACTED***")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottlePolicy {
    #[serde(with = "duration_secs")]
    pub min_edit_interval: Duration,
    pub maxlag: u32,
    pub max_retries: u32,
    #[serde(with = "duration_secs")]
    pub backoff_base: Duration,
}

impl Default for ThrottlePolicy {
    fn default() -> Self {
        Self {
            min_edit_interval: Duration::from_secs(12),
            maxlag: 5,
            max_retries: 3,
            backoff_base: Duration::from_secs(2),
        }
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_f64(d.as_secs_f64())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let secs = f64::deserialize(d)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throttle_policy_default() {
        let policy = ThrottlePolicy::default();
        assert_eq!(policy.min_edit_interval, Duration::from_secs(12));
        assert_eq!(policy.maxlag, 5);
        assert_eq!(policy.max_retries, 3);
        assert_eq!(policy.backoff_base, Duration::from_secs(2));
    }

    #[test]
    fn test_throttle_policy_serialization() {
        let policy = ThrottlePolicy {
            min_edit_interval: Duration::from_secs(10),
            maxlag: 3,
            max_retries: 5,
            backoff_base: Duration::from_millis(1500),
        };

        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: ThrottlePolicy = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.min_edit_interval, Duration::from_secs(10));
        assert_eq!(deserialized.maxlag, 3);
        assert_eq!(deserialized.max_retries, 5);
        assert_eq!(deserialized.backoff_base, Duration::from_millis(1500));
    }

    #[test]
    fn test_auth_method_bot_password() {
        let auth = AuthMethod::BotPassword {
            username: "TestBot".to_string(),
        };

        match auth {
            AuthMethod::BotPassword { username } => assert_eq!(username, "TestBot"),
            _ => panic!("Expected BotPassword auth method"),
        }
    }

    #[test]
    fn test_auth_method_oauth2() {
        let auth = AuthMethod::OAuth2 {
            client_id: "client123".to_string(),
            client_secret: "secret456".to_string(),
        };

        match auth {
            AuthMethod::OAuth2 {
                client_id,
                client_secret,
            } => {
                assert_eq!(client_id, "client123");
                assert_eq!(client_secret, "secret456");
            }
            _ => panic!("Expected OAuth2 auth method"),
        }
    }

    #[test]
    fn test_profile_serialization() {
        let mut namespaces = HashSet::new();
        namespaces.insert(Namespace::MAIN);
        namespaces.insert(Namespace::USER);

        let profile = Profile {
            id: "enwiki".to_string(),
            name: "English Wikipedia".to_string(),
            api_url: url::Url::parse("https://en.wikipedia.org/w/api.php").unwrap(),
            auth_method: AuthMethod::BotPassword {
                username: "Bot".to_string(),
            },
            default_namespaces: namespaces.clone(),
            throttle_policy: ThrottlePolicy::default(),
        };

        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "enwiki");
        assert_eq!(deserialized.name, "English Wikipedia");
        assert_eq!(deserialized.default_namespaces.len(), 2);
        assert!(deserialized.default_namespaces.contains(&Namespace::MAIN));
    }

    #[test]
    fn test_duration_roundtrip() {
        let original = Duration::from_secs_f64(12.5);
        let _serialized = serde_json::to_value(&original).unwrap();

        // Manual serialization test
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Wrapper {
            #[serde(with = "duration_secs")]
            duration: Duration,
        }

        let wrapper = Wrapper { duration: original };
        let json = serde_json::to_string(&wrapper).unwrap();
        let deserialized: Wrapper = serde_json::from_str(&json).unwrap();

        assert!((deserialized.duration.as_secs_f64() - 12.5).abs() < 0.001);
    }
}
