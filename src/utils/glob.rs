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
}
