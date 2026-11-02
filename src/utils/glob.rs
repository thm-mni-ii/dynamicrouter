use std::collections::HashSet;

/// Returns true if the given string contains glob wildcard characters ('*' or '?').
#[must_use]
pub fn is_glob_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?')
}

/// Checks whether `candidate` matches a single pattern (`*` catch-all, exact equality, or a glob expression).
#[must_use]
pub fn matches_glob(pattern: &str, candidate: &str) -> bool {
    if pattern == "*" || pattern == candidate {
        return true;
    }
    if let Ok(pat) = glob::Pattern::new(pattern) {
        pat.matches(candidate)
    } else {
        false
    }
}

/// Checks whether `model_id` is permitted by a set of model patterns.
/// If `allowed_patterns` is empty or None, handling is caller-dependent, but if passed a set:
/// Returns true if the set contains "*", contains `model_id`, or matches any glob pattern.
#[must_use]
pub fn is_model_allowed(allowed_patterns: &HashSet<String>, model_id: &str) -> bool {
    if allowed_patterns.contains("*") || allowed_patterns.contains(model_id) {
        return true;
    }
    for pat in allowed_patterns {
        if matches_glob(pat, model_id) {
            return true;
        }
    }
    false
}

/// Checks whether `username` is permitted by a set of user patterns (or "*").
#[must_use]
pub fn is_user_allowed(allowed_users: &HashSet<String>, username: &str) -> bool {
    if allowed_users.contains("*") || allowed_users.contains(username) {
        return true;
    }
    for pat in allowed_users {
        if matches_glob(pat, username) {
            return true;
        }
    }
    false
}

/// Checks whether any of the user's groups is permitted by a set of user/group patterns.
/// Supports "@group_name", "group:group_name", or raw "group_name" in allowed targets.
#[must_use]
pub fn is_any_group_allowed(allowed_targets: &HashSet<String>, groups: &[String]) -> bool {
    if allowed_targets.contains("*") {
        return true;
    }
    for group in groups {
        let at_group = format!("@{group}");
        let colon_group = format!("group:{group}");
        if allowed_targets.contains(group)
            || allowed_targets.contains(&at_group)
            || allowed_targets.contains(&colon_group)
        {
            return true;
        }
        for pat in allowed_targets {
            let pat_stripped = pat
                .strip_prefix('@')
                .or_else(|| pat.strip_prefix("group:"))
                .unwrap_or(pat);
            if matches_glob(pat_stripped, group) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_glob_pattern() {
        assert!(is_glob_pattern("*"));
        assert!(is_glob_pattern("meta-llama/*"));
        assert!(is_glob_pattern("model-?"));
        assert!(!is_glob_pattern("meta-llama/Llama-3-8B"));
    }

    #[test]
    fn test_matches_glob() {
        assert!(matches_glob("*", "any-model"));
        assert!(matches_glob("meta-llama/*", "meta-llama/Llama-3-8B"));
        assert!(!matches_glob("meta-llama/*", "google/gemma-7b"));
        assert!(matches_glob("gpt-*", "gpt-4o"));
        assert!(!matches_glob("gpt-*", "claude-3"));
    }

    #[test]
    fn test_is_model_allowed() {
        let mut patterns = HashSet::new();
        patterns.insert("meta-llama/*".to_string());
        patterns.insert("mistralai/Mistral-7B-v0.1".to_string());

        assert!(is_model_allowed(&patterns, "meta-llama/Llama-3-8B"));
        assert!(is_model_allowed(&patterns, "mistralai/Mistral-7B-v0.1"));
        assert!(!is_model_allowed(&patterns, "google/gemma-7b"));

        let mut wildcard_patterns = HashSet::new();
        wildcard_patterns.insert("*".to_string());
        assert!(is_model_allowed(&wildcard_patterns, "anything"));
    }

    #[test]
    fn test_is_user_allowed() {
        let mut allowed = HashSet::new();
        allowed.insert("team-*".to_string());
        allowed.insert("alice".to_string());

        assert!(is_user_allowed(&allowed, "team-ml"));
        assert!(is_user_allowed(&allowed, "alice"));
        assert!(!is_user_allowed(&allowed, "bob"));
    }

    #[test]
    fn test_is_any_group_allowed() {
        let mut allowed = HashSet::new();
        allowed.insert("@engineering".to_string());
        allowed.insert("group:research".to_string());
        allowed.insert("students-*".to_string());

        assert!(is_any_group_allowed(&allowed, &["engineering".to_string()]));
        assert!(is_any_group_allowed(&allowed, &["research".to_string()]));
        assert!(is_any_group_allowed(&allowed, &["students-2026".to_string()]));
        assert!(!is_any_group_allowed(&allowed, &["marketing".to_string()]));
    }
}
