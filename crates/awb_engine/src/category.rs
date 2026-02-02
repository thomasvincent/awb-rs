use regex::Regex;

/// Actions that can be performed on categories
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CategoryAction {
    /// Add a category to the page
    Add(String),
    /// Remove a category from the page
    Remove(String),
    /// Rename a category (old name, new name)
    Rename(String, String),
    /// Sort all categories alphabetically
    Sort,
}

/// Manages category operations on wikitext
#[derive(Debug)]
pub struct CategoryManager {
    /// Regex to match category links (case-insensitive)
    category_re: Regex,
}

impl CategoryManager {
    /// Create a new CategoryManager
    pub fn new() -> Self {
        Self {
            // Match [[Category:Name]] or [[category:Name]] with optional sort key
            category_re: Regex::new(r"(?i)\[\[Category:([^\]|]+)(?:\|([^\]]*))?\]\]").unwrap(),
        }
    }

    /// Add a category to wikitext
    /// Adds the category before existing categories, or at the end if none exist
    pub fn add_category(&self, wikitext: &str, category: &str) -> String {
        let category_name = self.normalize_category_name(category);

        // Check if category already exists
        if self.has_category(wikitext, &category_name) {
            return wikitext.to_string();
        }

        let new_category = format!("[[Category:{}]]", category_name);

        // Find the position of the first existing category
        if let Some(first_match) = self.category_re.find(wikitext) {
            let pos = first_match.start();
            let mut result = String::with_capacity(wikitext.len() + new_category.len() + 1);
            result.push_str(&wikitext[..pos]);
            result.push_str(&new_category);
            result.push('\n');
            result.push_str(&wikitext[pos..]);
            result
        } else {
            // No categories exist, add at the end
            let mut result = wikitext.to_string();
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&new_category);
            result.push('\n');
            result
        }
    }

    /// Remove a category from wikitext
    pub fn remove_category(&self, wikitext: &str, category: &str) -> String {
        let category_name = self.normalize_category_name(category);
        let category_lower = category_name.to_lowercase();

        let mut result = String::new();
        let mut last_end = 0;

        for cap in self.category_re.captures_iter(wikitext) {
            let full_match = cap.get(0).unwrap();
            let cat_name = cap.get(1).unwrap().as_str();

            // Case-insensitive comparison
            if cat_name.to_lowercase() == category_lower {
                // Add text before this category
                result.push_str(&wikitext[last_end..full_match.start()]);

                // Skip the category line (including newline if present)
                last_end = full_match.end();
                if wikitext[last_end..].starts_with('\n') {
                    last_end += 1;
                }
            }
        }

        // Add remaining text
        result.push_str(&wikitext[last_end..]);
        result
    }

    /// Rename a category in wikitext
    pub fn rename_category(&self, wikitext: &str, old_name: &str, new_name: &str) -> String {
        let old_name_norm = self.normalize_category_name(old_name);
        let new_name_norm = self.normalize_category_name(new_name);
        let old_lower = old_name_norm.to_lowercase();

        let mut result = String::new();
        let mut last_end = 0;

        for cap in self.category_re.captures_iter(wikitext) {
            let full_match = cap.get(0).unwrap();
            let cat_name = cap.get(1).unwrap().as_str();
            let sort_key = cap.get(2).map(|m| m.as_str());

            result.push_str(&wikitext[last_end..full_match.start()]);

            // Case-insensitive comparison
            if cat_name.to_lowercase() == old_lower {
                // Replace with new name, preserving sort key
                if let Some(key) = sort_key {
                    result.push_str(&format!("[[Category:{}|{}]]", new_name_norm, key));
                } else {
                    result.push_str(&format!("[[Category:{}]]", new_name_norm));
                }
            } else {
                // Keep original category
                result.push_str(full_match.as_str());
            }

            last_end = full_match.end();
        }

        result.push_str(&wikitext[last_end..]);
        result
    }

    /// List all categories in wikitext
    pub fn list_categories(&self, wikitext: &str) -> Vec<String> {
        self.category_re
            .captures_iter(wikitext)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    }

    /// Sort all categories alphabetically
    pub fn sort_categories(&self, wikitext: &str) -> String {
        // Extract all category lines with their positions
        let mut categories: Vec<(String, Option<String>)> = Vec::new();
        let mut category_positions: Vec<(usize, usize)> = Vec::new();

        for cap in self.category_re.captures_iter(wikitext) {
            let full_match = cap.get(0).unwrap();
            let cat_name = cap.get(1).unwrap().as_str().to_string();
            let sort_key = cap.get(2).map(|m| m.as_str().to_string());

            categories.push((cat_name, sort_key));
            category_positions.push((full_match.start(), full_match.end()));
        }

        if categories.is_empty() {
            return wikitext.to_string();
        }

        // Sort categories by name (case-insensitive)
        let mut sorted_cats = categories.clone();
        sorted_cats.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        // If already sorted, return unchanged
        if sorted_cats == categories {
            return wikitext.to_string();
        }

        // Find the range containing all categories
        let first_pos = category_positions[0].0;
        let last_pos = category_positions[category_positions.len() - 1].1;

        // Build the sorted category block
        let mut sorted_block = String::new();
        for (i, (cat_name, sort_key)) in sorted_cats.iter().enumerate() {
            if let Some(key) = sort_key {
                sorted_block.push_str(&format!("[[Category:{}|{}]]", cat_name, key));
            } else {
                sorted_block.push_str(&format!("[[Category:{}]]", cat_name));
            }
            if i < sorted_cats.len() - 1 {
                sorted_block.push('\n');
            }
        }

        // Reconstruct wikitext
        let mut result = String::new();
        result.push_str(&wikitext[..first_pos]);
        result.push_str(&sorted_block);
        result.push_str(&wikitext[last_pos..]);
        result
    }

    /// Apply a batch of category actions
    pub fn apply_actions(&self, wikitext: &str, actions: &[CategoryAction]) -> String {
        let mut result = wikitext.to_string();

        for action in actions {
            result = match action {
                CategoryAction::Add(cat) => self.add_category(&result, cat),
                CategoryAction::Remove(cat) => self.remove_category(&result, cat),
                CategoryAction::Rename(old, new) => self.rename_category(&result, old, new),
                CategoryAction::Sort => self.sort_categories(&result),
            };
        }

        result
    }

    /// Check if a category exists in wikitext (case-insensitive)
    fn has_category(&self, wikitext: &str, category: &str) -> bool {
        let category_lower = category.to_lowercase();
        self.list_categories(wikitext)
            .iter()
            .any(|cat| cat.to_lowercase() == category_lower)
    }

    /// Normalize category name (remove "Category:" prefix if present)
    fn normalize_category_name(&self, category: &str) -> String {
        if category.starts_with("Category:") {
            category[9..].to_string()
        } else if category.starts_with("category:") {
            category[9..].to_string()
        } else {
            category.to_string()
        }
    }
}

impl Default for CategoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_category_to_empty() {
        let mgr = CategoryManager::new();
        let text = "Some article text.";
        let result = mgr.add_category(text, "Foo");
        assert!(result.contains("[[Category:Foo]]"));
    }

    #[test]
    fn test_add_category_before_existing() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Existing]]";
        let result = mgr.add_category(text, "New");
        assert!(result.contains("[[Category:New]]"));
        assert!(result.find("[[Category:New]]").unwrap() < result.find("[[Category:Existing]]").unwrap());
    }

    #[test]
    fn test_add_category_duplicate() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Foo]]";
        let result = mgr.add_category(text, "Foo");
        assert_eq!(result, text);
    }

    #[test]
    fn test_add_category_case_insensitive_duplicate() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Foo]]";
        let result = mgr.add_category(text, "foo");
        assert_eq!(result, text);
    }

    #[test]
    fn test_remove_category() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Foo]]\n[[Category:Bar]]";
        let result = mgr.remove_category(text, "Foo");
        assert!(!result.contains("[[Category:Foo]]"));
        assert!(result.contains("[[Category:Bar]]"));
    }

    #[test]
    fn test_remove_category_case_insensitive() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Foo]]";
        let result = mgr.remove_category(text, "foo");
        assert!(!result.contains("[[Category:Foo]]"));
    }

    #[test]
    fn test_remove_category_with_sort_key() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:Foo|Sort]]";
        let result = mgr.remove_category(text, "Foo");
        assert!(!result.contains("[[Category:Foo"));
    }

    #[test]
    fn test_rename_category() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:OldName]]";
        let result = mgr.rename_category(text, "OldName", "NewName");
        assert!(!result.contains("[[Category:OldName]]"));
        assert!(result.contains("[[Category:NewName]]"));
    }

    #[test]
    fn test_rename_category_preserves_sort_key() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:OldName|SortKey]]";
        let result = mgr.rename_category(text, "OldName", "NewName");
        assert!(result.contains("[[Category:NewName|SortKey]]"));
    }

    #[test]
    fn test_rename_category_case_insensitive() {
        let mgr = CategoryManager::new();
        let text = "Article text.\n[[Category:OldName]]";
        let result = mgr.rename_category(text, "oldname", "NewName");
        assert!(result.contains("[[Category:NewName]]"));
    }

    #[test]
    fn test_list_categories() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Foo]]\n[[Category:Bar]]\n[[Category:Baz|Sort]]";
        let cats = mgr.list_categories(text);
        assert_eq!(cats, vec!["Foo", "Bar", "Baz"]);
    }

    #[test]
    fn test_list_categories_empty() {
        let mgr = CategoryManager::new();
        let text = "Article with no categories.";
        let cats = mgr.list_categories(text);
        assert!(cats.is_empty());
    }

    #[test]
    fn test_sort_categories() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Zebra]]\n[[Category:Apple]]\n[[Category:Middle]]";
        let result = mgr.sort_categories(text);
        let cats = mgr.list_categories(&result);
        assert_eq!(cats, vec!["Apple", "Middle", "Zebra"]);
    }

    #[test]
    fn test_sort_categories_preserves_sort_keys() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Zebra|Z]]\n[[Category:Apple|A]]";
        let result = mgr.sort_categories(text);
        assert!(result.contains("[[Category:Apple|A]]"));
        assert!(result.contains("[[Category:Zebra|Z]]"));
    }

    #[test]
    fn test_sort_categories_already_sorted() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Apple]]\n[[Category:Banana]]";
        let result = mgr.sort_categories(text);
        assert_eq!(result, text);
    }

    #[test]
    fn test_sort_categories_case_insensitive() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:zebra]]\n[[Category:Apple]]";
        let result = mgr.sort_categories(text);
        let cats = mgr.list_categories(&result);
        assert_eq!(cats, vec!["Apple", "zebra"]);
    }

    #[test]
    fn test_apply_actions_multiple() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Old]]\n[[Category:Keep]]";
        let actions = vec![
            CategoryAction::Remove("Old".to_string()),
            CategoryAction::Add("New".to_string()),
            CategoryAction::Sort,
        ];
        let result = mgr.apply_actions(text, &actions);
        assert!(!result.contains("[[Category:Old]]"));
        assert!(result.contains("[[Category:Keep]]"));
        assert!(result.contains("[[Category:New]]"));
    }

    #[test]
    fn test_normalize_category_name() {
        let mgr = CategoryManager::new();
        assert_eq!(mgr.normalize_category_name("Foo"), "Foo");
        assert_eq!(mgr.normalize_category_name("Category:Foo"), "Foo");
        assert_eq!(mgr.normalize_category_name("category:Foo"), "Foo");
    }

    #[test]
    fn test_category_with_lowercase_prefix() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[category:LowerCase]]";
        let cats = mgr.list_categories(text);
        assert_eq!(cats, vec!["LowerCase"]);
    }

    #[test]
    fn test_mixed_case_categories() {
        let mgr = CategoryManager::new();
        let text = "Article.\n[[Category:Upper]]\n[[category:lower]]";
        let cats = mgr.list_categories(text);
        assert_eq!(cats, vec!["Upper", "lower"]);
    }
}
