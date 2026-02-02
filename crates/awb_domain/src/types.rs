use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RevisionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Namespace(pub i32);

impl Namespace {
    pub const MAIN: Self = Self(0);
    pub const TALK: Self = Self(1);
    pub const USER: Self = Self(2);
    pub const USER_TALK: Self = Self(3);
    pub const PROJECT: Self = Self(4);
    pub const PROJECT_TALK: Self = Self(5);
    pub const FILE: Self = Self(6);
    pub const FILE_TALK: Self = Self(7);
    pub const MEDIAWIKI: Self = Self(8);
    pub const TEMPLATE: Self = Self(10);
    pub const TEMPLATE_TALK: Self = Self(11);
    pub const HELP: Self = Self(12);
    pub const CATEGORY: Self = Self(14);
    pub const CATEGORY_TALK: Self = Self(15);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Title {
    pub namespace: Namespace,
    pub name: String,
    pub display: String,
}

impl Title {
    pub fn new(namespace: Namespace, name: impl Into<String>) -> Self {
        let name = name.into();
        let display = if namespace == Namespace::MAIN {
            name.clone()
        } else {
            format!("{}:{}", namespace.0, &name) // simplified
        };
        Self {
            namespace,
            name,
            display,
        }
    }
}

impl std::fmt::Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    pub page_id: PageId,
    pub title: Title,
    pub revision: RevisionId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub wikitext: String,
    pub size_bytes: u64,
    pub is_redirect: bool,
    pub protection: ProtectionInfo,
    pub properties: PageProperties,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProtectionInfo {
    pub edit: Option<ProtectionLevel>,
    pub move_page: Option<ProtectionLevel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtectionLevel {
    Autoconfirmed,
    ExtendedConfirmed,
    Sysop,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageProperties {
    pub is_disambig: bool,
    pub wikibase_item: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_constants() {
        assert_eq!(Namespace::MAIN.0, 0);
        assert_eq!(Namespace::TALK.0, 1);
        assert_eq!(Namespace::USER.0, 2);
        assert_eq!(Namespace::CATEGORY.0, 14);
    }

    #[test]
    fn test_title_new_main_namespace() {
        let title = Title::new(Namespace::MAIN, "Example");
        assert_eq!(title.namespace, Namespace::MAIN);
        assert_eq!(title.name, "Example");
        assert_eq!(title.display, "Example");
    }

    #[test]
    fn test_title_new_with_namespace() {
        let title = Title::new(Namespace::USER, "TestUser");
        assert_eq!(title.namespace, Namespace::USER);
        assert_eq!(title.name, "TestUser");
        assert_eq!(title.display, "2:TestUser");
    }

    #[test]
    fn test_title_display() {
        let title = Title::new(Namespace::CATEGORY, "Rust");
        assert_eq!(title.to_string(), "14:Rust");
    }

    #[test]
    fn test_page_id_serialization() {
        let page_id = PageId(12345);
        let json = serde_json::to_string(&page_id).unwrap();
        assert_eq!(json, "12345");
        let deserialized: PageId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.0, 12345);
    }

    #[test]
    fn test_protection_level_equality() {
        assert_eq!(
            ProtectionLevel::Autoconfirmed,
            ProtectionLevel::Autoconfirmed
        );
        assert_ne!(ProtectionLevel::Autoconfirmed, ProtectionLevel::Sysop);
    }

    #[test]
    fn test_protection_info_default() {
        let protection = ProtectionInfo::default();
        assert!(protection.edit.is_none());
        assert!(protection.move_page.is_none());
    }

    #[test]
    fn test_page_properties_default() {
        let props = PageProperties::default();
        assert!(!props.is_disambig);
        assert!(props.wikibase_item.is_none());
    }
}
