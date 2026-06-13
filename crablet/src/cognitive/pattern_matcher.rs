//! Pattern Matcher - Multi-pattern matching engine
//!
//! Supports exact, prefix, contains, regex, and fuzzy matching
//! with lazy regex compilation and confidence scoring.

use regex::Regex;
use std::collections::HashMap;

/// Match confidence level
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MatchConfidence {
    Exact = 100, // Exact match
    High = 80,   // Very close match
    Medium = 60, // Good match
    Low = 40,    // Possible match
    None = 0,    // No match
    Fuzzy,
}

/// Result of a pattern match
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub command_id: String,
    pub confidence: MatchConfidence,
    pub matched_pattern: String,
    pub extracted_params: HashMap<String, String>,
    pub match_type: MatchType,
}

/// Type of pattern match
#[derive(Clone, Debug, PartialEq)]
pub enum MatchType {
    Exact,      // Character-for-character match
    Prefix,     // Prefix match
    Regex,      // Regular expression match
    Fuzzy,      // Fuzzy string match
    Semantic,   // Semantic similarity match
    Contextual, // Context-based match
}

/// Pattern types for matching
#[derive(Clone, Debug)]
pub enum Pattern {
    Exact(String),
    Prefix(String),
    Contains(String),
    Regex(String),
    Fuzzy(String, usize),
}

/// Pattern matcher with lazy regex compilation
pub struct PatternMatcher {
    compiled_regexes: HashMap<String, Regex>,
}

impl Default for PatternMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternMatcher {
    pub fn new() -> Self {
        Self {
            compiled_regexes: HashMap::new(),
        }
    }

    /// Match input against a pattern
    pub fn match_pattern(
        &mut self,
        input: &str,
        pattern: &Pattern,
    ) -> Option<(MatchConfidence, HashMap<String, String>)> {
        let input_lower = input.trim().to_lowercase();

        match pattern {
            Pattern::Exact(s) => {
                if input_lower == s.to_lowercase() {
                    Some((MatchConfidence::Exact, HashMap::new()))
                } else {
                    None
                }
            }

            Pattern::Prefix(s) => {
                if input_lower.starts_with(&s.to_lowercase()) {
                    Some((MatchConfidence::High, HashMap::new()))
                } else {
                    None
                }
            }

            Pattern::Contains(s) => {
                if input_lower.contains(&s.to_lowercase()) {
                    Some((MatchConfidence::Medium, HashMap::new()))
                } else {
                    None
                }
            }

            Pattern::Regex(pattern_str) => {
                // Compile lazily, but never panic on an invalid user-supplied
                // pattern: skip the rule and warn instead of crashing the
                // request-handling hot path.
                if !self.compiled_regexes.contains_key(pattern_str) {
                    match Regex::new(pattern_str) {
                        Ok(re) => {
                            self.compiled_regexes
                                .insert(pattern_str.to_string(), re);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Skipping invalid regex pattern '{}': {}",
                                pattern_str,
                                e
                            );
                            return None;
                        }
                    }
                }

                if let Some(re) = self.compiled_regexes.get(pattern_str) {
                    if re.is_match(&input_lower) {
                        let mut params = HashMap::new();
                        if let Some(captures) = re.captures(&input_lower) {
                            for name in re.capture_names().flatten() {
                                if let Some(m) = captures.name(name) {
                                    params.insert(name.to_string(), m.as_str().to_string());
                                }
                            }
                        }
                        Some((MatchConfidence::High, params))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }

            Pattern::Fuzzy(target, max_distance) => {
                let distance = levenshtein_distance(&input_lower, &target.to_lowercase());
                if distance <= *max_distance {
                    Some((MatchConfidence::Fuzzy, HashMap::new()))
                } else {
                    None
                }
            }
        }
    }
}

/// Compute Levenshtein edit distance between two strings.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }

    for (j, cell) in matrix[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("hello", &Pattern::Exact("hello".to_string()));
        assert!(result.is_some());
        assert_eq!(result.map(|r| r.0), Some(MatchConfidence::Exact));
    }

    #[test]
    fn test_prefix_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("hello there", &Pattern::Prefix("hello".to_string()));
        assert!(result.is_some());
        assert_eq!(result.map(|r| r.0), Some(MatchConfidence::High));
    }

    #[test]
    fn test_contains_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("say hello world", &Pattern::Contains("hello".to_string()));
        assert!(result.is_some());
        assert_eq!(result.map(|r| r.0), Some(MatchConfidence::Medium));
    }

    #[test]
    fn test_regex_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("hello!", &Pattern::Regex(r"^(?i)hello[!?.]*$".to_string()));
        assert!(result.is_some());
        assert_eq!(result.map(|r| r.0), Some(MatchConfidence::High));
    }

    #[test]
    fn test_fuzzy_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("helo", &Pattern::Fuzzy("hello".to_string(), 1));
        assert!(result.is_some());
        assert_eq!(result.map(|r| r.0), Some(MatchConfidence::Fuzzy));
    }

    #[test]
    fn test_no_match() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("goodbye", &Pattern::Exact("hello".to_string()));
        assert!(result.is_none());
    }

    #[test]
    fn test_invalid_regex_skipped() {
        let mut matcher = PatternMatcher::new();
        let result = matcher.match_pattern("test", &Pattern::Regex("[invalid".to_string()));
        assert!(result.is_none());
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        assert_eq!(levenshtein_distance("hello", "helo"), 1);
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        assert_eq!(levenshtein_distance("", "hello"), 5);
        assert_eq!(levenshtein_distance("hello", ""), 5);
    }
}
