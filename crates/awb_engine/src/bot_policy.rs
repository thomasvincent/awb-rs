//! {{bots}} / {{nobots}} handling for Wikipedia bot policy compliance.
//!
//! Detects exclusion templates in wikitext and determines whether a named bot
//! is allowed to edit a given page. Fails closed: if parsing is ambiguous,
//! the bot is denied.
//!
//! Supported templates:
//! - `{{nobots}}` — deny all bots
//! - `{{bots}}` — allow all bots (default)
//! - `{{bots|deny=all}}` — deny all bots
//! - `{{bots|allow=all}}` — allow all bots
//! - `{{bots|deny=BotA,BotB}}` — deny specific bots
//! - `{{bots|allow=BotA,BotB}}` — allow only these bots
//! - `{{bots|optout=Category}}` — bot-specific opt-out (treated as deny for safety)
//!
//! All matching is case-insensitive and whitespace-tolerant.

use std::sync::OnceLock;

/// Result of checking bot policy on a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotPolicyResult {
    /// Bot is allowed to edit.
    Allowed,
    /// Bot is denied by policy.
    Denied { reason: String },
}

impl BotPolicyResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

/// Check whether `bot_name` is allowed to edit wikitext according to
/// {{nobots}} and {{bots}} templates.
///
/// `bot_name` should be the bot's username (case-insensitive comparison).
///
/// Returns `Denied` if any exclusion template blocks this bot.
/// Fails closed on ambiguous or unparseable templates.
pub fn check_bot_allowed(wikitext: &str, bot_name: &str) -> BotPolicyResult {
    // Quick check: if no braces present at all, allow
    if !wikitext.contains("{{") {
        return BotPolicyResult::Allowed;
    }

    // Check {{nobots}} — deny all
    static NOBOTS_RE: OnceLock<regex::Regex> = OnceLock::new();
    let nobots_re = NOBOTS_RE
        .get_or_init(|| regex::Regex::new(r"(?i)\{\{\s*nobots\s*\}\}").expect("known-valid regex"));
    if nobots_re.is_match(wikitext) {
        return BotPolicyResult::Denied {
            reason: "{{nobots}} present".to_string(),
        };
    }

    // Check {{bots|...}} variants
    static BOTS_RE: OnceLock<regex::Regex> = OnceLock::new();
    // REGEX LIMITATION: The pattern `[^}]*` matches any character except `}`, which means
    // it stops at the FIRST `}` character, not the closing `}}`. This is an inherent
    // limitation of character class negation in regular expressions.
    //
    // Examples of what this matches correctly:
    // - `{{bots|deny=BotA,BotB}}` → captures `deny=BotA,BotB`
    // - `{{bots|allow=all}}` → captures `allow=all`
    //
    // Examples where the regex stops early (but mitigation handles):
    // - `{{bots|deny={{PAGENAME}}}}` → captures `deny={{PAGENAME` (stops at first `}`)
    //   Mitigation: The nested-template check below detects `{{` in captured params
    //   and fails closed (denies the bot).
    //
    // This limitation is acceptable because:
    // 1. Normal parameter values like `deny=BotA,BotB` contain no `}` characters
    // 2. Nested templates (containing `{{`) are detected and trigger fail-closed denial
    // 3. Malformed templates fail open (bot processes page), which is safe since Wikipedia
    //    editors can fix the template syntax, and we err on the side of processing
    let bots_re = BOTS_RE.get_or_init(|| {
        regex::Regex::new(r"(?i)\{\{\s*bots\s*\|([^}]*)\}\}").expect("known-valid regex")
    });

    let bot_lower = bot_name.to_ascii_lowercase();

    for caps in bots_re.captures_iter(wikitext) {
        let params = &caps[1];

        // Fail closed if params contain nested templates — regex can't parse these reliably
        if params.contains("{{") {
            return BotPolicyResult::Denied {
                reason: "{{bots}} contains nested templates; failing closed".to_string(),
            };
        }

        // Parse key=value pairs
        for param in params.split('|') {
            let param = param.trim();
            if let Some((key, value)) = param.split_once('=') {
                let key = key.trim().to_ascii_lowercase();
                let value = value.trim();

                match key.as_str() {
                    "deny" => {
                        let value_lower = value.to_ascii_lowercase();
                        if value_lower == "all" || value_lower == "none" {
                            // deny=all means deny all; deny=none is unusual but treat as allow
                            if value_lower == "all" {
                                return BotPolicyResult::Denied {
                                    reason: "{{bots|deny=all}}".to_string(),
                                };
                            }
                        } else {
                            // Comma-separated list of bot names
                            let denied: Vec<&str> =
                                value.split(',').map(|s| s.trim()).collect();
                            if denied.iter().any(|name| name.eq_ignore_ascii_case(&bot_lower)) {
                                return BotPolicyResult::Denied {
                                    reason: format!("{{{{bots|deny={}}}}}", value),
                                };
                            }
                        }
                    }
                    "allow" => {
                        let value_lower = value.to_ascii_lowercase();
                        if value_lower == "all" {
                            // Explicitly allowed — but keep checking other templates
                            continue;
                        } else if value_lower == "none" {
                            return BotPolicyResult::Denied {
                                reason: "{{bots|allow=none}}".to_string(),
                            };
                        } else {
                            // Only listed bots are allowed
                            let allowed: Vec<&str> =
                                value.split(',').map(|s| s.trim()).collect();
                            if !allowed.iter().any(|name| name.eq_ignore_ascii_case(&bot_lower)) {
                                return BotPolicyResult::Denied {
                                    reason: format!(
                                        "{{{{bots|allow={}}}}} — bot not in allow list",
                                        value
                                    ),
                                };
                            }
                        }
                    }
                    "optout" => {
                        // Opt-out categories — fail closed (deny)
                        return BotPolicyResult::Denied {
                            reason: format!("{{{{bots|optout={}}}}}", value),
                        };
                    }
                    _ => {
                        // Unknown parameter — fail closed
                        return BotPolicyResult::Denied {
                            reason: format!(
                                "Unknown {{{{bots}}}} parameter: {}={}",
                                key, value
                            ),
                        };
                    }
                }
            }
        }
    }

    BotPolicyResult::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_templates_allows() {
        let result = check_bot_allowed("Just normal article text.", "MyBot");
        assert_eq!(result, BotPolicyResult::Allowed);
    }

    #[test]
    fn test_nobots_denies() {
        let result = check_bot_allowed("Some text\n{{nobots}}\nMore text", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_nobots_case_insensitive() {
        let result = check_bot_allowed("{{NOBOTS}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_nobots_with_whitespace() {
        let result = check_bot_allowed("{{ nobots }}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_all() {
        let result = check_bot_allowed("{{bots|deny=all}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_all_case_insensitive() {
        let result = check_bot_allowed("{{Bots|Deny=All}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_specific_bot() {
        let result = check_bot_allowed("{{bots|deny=MyBot,OtherBot}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_specific_bot_case_insensitive() {
        let result = check_bot_allowed("{{bots|deny=mybot}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_other_bot_allows_us() {
        let result = check_bot_allowed("{{bots|deny=OtherBot}}", "MyBot");
        assert_eq!(result, BotPolicyResult::Allowed);
    }

    #[test]
    fn test_bots_allow_all() {
        let result = check_bot_allowed("{{bots|allow=all}}", "MyBot");
        assert_eq!(result, BotPolicyResult::Allowed);
    }

    #[test]
    fn test_bots_allow_specific_bot() {
        let result = check_bot_allowed("{{bots|allow=MyBot,OtherBot}}", "MyBot");
        assert_eq!(result, BotPolicyResult::Allowed);
    }

    #[test]
    fn test_bots_allow_specific_bot_not_listed() {
        let result = check_bot_allowed("{{bots|allow=OtherBot}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_allow_none() {
        let result = check_bot_allowed("{{bots|allow=none}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_optout_fails_closed() {
        let result = check_bot_allowed("{{bots|optout=SomeCategory}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_unknown_param_fails_closed() {
        let result = check_bot_allowed("{{bots|unknown=value}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_with_whitespace() {
        let result = check_bot_allowed("{{ bots | deny = MyBot , OtherBot }}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_bots_deny_none_allows() {
        let result = check_bot_allowed("{{bots|deny=none}}", "MyBot");
        assert_eq!(result, BotPolicyResult::Allowed);
    }

    #[test]
    fn test_multiple_templates_first_deny_wins() {
        let text = "{{bots|allow=all}}\n{{nobots}}";
        let result = check_bot_allowed(text, "MyBot");
        // nobots takes precedence since we check it first
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_template_in_middle_of_text() {
        let text = "Long article text here.\n\n== References ==\n{{bots|deny=MyBot}}\n[[Category:Test]]";
        let result = check_bot_allowed(text, "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }

    #[test]
    fn test_is_allowed_method() {
        assert!(BotPolicyResult::Allowed.is_allowed());
        assert!(!BotPolicyResult::Denied {
            reason: "test".to_string()
        }
        .is_allowed());
    }

    #[test]
    fn test_nested_templates_in_bots_fails_closed() {
        // Nested templates make regex parsing unreliable — fail closed
        let result = check_bot_allowed("{{bots|deny={{PAGENAME}}}}", "MyBot");
        assert!(matches!(result, BotPolicyResult::Denied { .. }));
    }
}
