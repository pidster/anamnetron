//! Canonical path utilities: kebab-case conversion, glob matching, path validation.
//!
//! All functions are WASM-safe -- no platform-specific dependencies.

/// Error returned when a canonical path is malformed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CanonicalPathError {
    /// Path does not start with `/`.
    #[error("must start with '/'")]
    MissingLeadingSlash,

    /// Path ends with `/` (not allowed except for root).
    #[error("must not end with '/'")]
    TrailingSlash,

    /// Path contains an empty segment (double slash).
    #[error("empty segment (double slash)")]
    EmptySegment,

    /// A segment contains characters that are not lowercase kebab-case.
    #[error("segment '{0}' is not lowercase kebab-case")]
    InvalidSegment(String),
}

/// Convert a segment from PascalCase, snake_case, or ALLCAPS to kebab-case.
///
/// Handles acronyms: `HTTPServer` becomes `http-server`.
#[must_use]
pub fn to_kebab_case(segment: &str) -> String {
    let mut result = String::with_capacity(segment.len() + 4);
    let chars: Vec<char> = segment.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        // Replace underscores and preserve existing hyphens as separators
        if c == '_' || c == '-' {
            if !result.is_empty() && !result.ends_with('-') {
                result.push('-');
            }
            continue;
        }

        if c.is_uppercase() {
            let prev_lower = i > 0 && chars[i - 1].is_lowercase();
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_lowercase();

            // Split before: lowercase->uppercase (camelCase) or acronym->word (HTTPServer)
            if (prev_lower || (prev_upper && next_lower))
                && !result.is_empty()
                && !result.ends_with('-')
            {
                result.push('-');
            }
        }

        result.push(c.to_ascii_lowercase());
    }

    result
}

/// Validate that a canonical path is well-formed.
///
/// Requirements: leading `/`, no trailing slash, lowercase kebab-case segments,
/// no empty segments.
pub fn validate_canonical_path(path: &str) -> Result<(), CanonicalPathError> {
    if !path.starts_with('/') {
        return Err(CanonicalPathError::MissingLeadingSlash);
    }
    if path.len() > 1 && path.ends_with('/') {
        return Err(CanonicalPathError::TrailingSlash);
    }
    let segments: Vec<&str> = path[1..].split('/').collect();
    for segment in &segments {
        if segment.is_empty() {
            return Err(CanonicalPathError::EmptySegment);
        }
        if !segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(CanonicalPathError::InvalidSegment(segment.to_string()));
        }
    }
    Ok(())
}

/// Get the parent path. Returns `None` for root-level paths (e.g., `/a`).
#[must_use]
pub fn parent_path(path: &str) -> Option<&str> {
    let last_slash = path.rfind('/')?;
    if last_slash == 0 {
        None
    } else {
        Some(&path[..last_slash])
    }
}

/// Get the last segment of a canonical path.
#[must_use]
pub fn path_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Check whether a canonical path matches a glob pattern.
///
/// `*` matches one segment, `**` matches any depth.
/// A trailing `/**` also matches the base path itself (e.g., `/app/core/**` matches `/app/core`).
#[must_use]
pub fn canonical_path_matches(path: &str, pattern: &str) -> bool {
    if glob_match::glob_match(pattern, path) {
        return true;
    }
    // When pattern ends with /**, also match the base path itself
    if let Some(base) = pattern.strip_suffix("/**") {
        return path == base;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_from_pascal_case() {
        assert_eq!(to_kebab_case("PaymentService"), "payment-service");
    }

    #[test]
    fn kebab_from_snake_case() {
        assert_eq!(to_kebab_case("payment_service"), "payment-service");
    }

    #[test]
    fn kebab_from_allcaps() {
        assert_eq!(to_kebab_case("ALLCAPS"), "allcaps");
    }

    #[test]
    fn kebab_from_acronym_prefix() {
        assert_eq!(to_kebab_case("HTTPServer"), "http-server");
    }

    #[test]
    fn kebab_from_mixed_camel_acronym() {
        assert_eq!(to_kebab_case("getHTTPClient"), "get-http-client");
    }

    #[test]
    fn kebab_noop_for_already_kebab() {
        assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
    }

    #[test]
    fn kebab_single_lowercase_word() {
        assert_eq!(to_kebab_case("core"), "core");
    }

    // --- validate_canonical_path ---

    #[test]
    fn valid_canonical_path() {
        assert!(validate_canonical_path("/svt/core/model").is_ok());
    }

    #[test]
    fn valid_root_level_path() {
        assert!(validate_canonical_path("/svt").is_ok());
    }

    #[test]
    fn invalid_missing_leading_slash() {
        assert!(validate_canonical_path("svt/core").is_err());
    }

    #[test]
    fn invalid_trailing_slash() {
        assert!(validate_canonical_path("/svt/core/").is_err());
    }

    #[test]
    fn invalid_uppercase_segment() {
        assert!(validate_canonical_path("/svt/Core").is_err());
    }

    #[test]
    fn invalid_empty_segment() {
        assert!(validate_canonical_path("/svt//core").is_err());
    }

    #[test]
    fn valid_path_with_digits() {
        assert!(validate_canonical_path("/svt/v2/core").is_ok());
    }

    // --- parent_path ---

    #[test]
    fn parent_of_deep_path() {
        assert_eq!(parent_path("/a/b/c"), Some("/a/b"));
    }

    #[test]
    fn parent_of_two_segment_path() {
        assert_eq!(parent_path("/a/b"), Some("/a"));
    }

    #[test]
    fn parent_of_root_level_path() {
        assert_eq!(parent_path("/a"), None);
    }

    // --- path_name ---

    #[test]
    fn name_of_deep_path() {
        assert_eq!(path_name("/a/b/c"), "c");
    }

    #[test]
    fn name_of_root_level_path() {
        assert_eq!(path_name("/a"), "a");
    }

    // --- canonical_path_matches ---

    #[test]
    fn matches_exact_path() {
        assert!(canonical_path_matches("/svt/core", "/svt/core"));
    }

    #[test]
    fn matches_star_one_segment() {
        assert!(canonical_path_matches("/svt/core/model", "/svt/*/model"));
    }

    #[test]
    fn star_does_not_match_multiple_segments() {
        assert!(!canonical_path_matches(
            "/svt/core/store/cozo",
            "/svt/*/cozo"
        ));
    }

    #[test]
    fn matches_globstar_any_depth() {
        assert!(canonical_path_matches("/svt/core/model", "/svt/**"));
    }

    #[test]
    fn globstar_matches_deeply_nested() {
        assert!(canonical_path_matches(
            "/svt/core/store/cozo",
            "/svt/core/**"
        ));
    }

    #[test]
    fn globstar_matches_immediate_child() {
        assert!(canonical_path_matches("/svt/core", "/svt/**"));
    }

    #[test]
    fn globstar_matches_base_path_itself() {
        assert!(canonical_path_matches("/svt/core", "/svt/core/**"));
    }

    #[test]
    fn no_match_different_path() {
        assert!(!canonical_path_matches("/svt/analyzer", "/svt/core/**"));
    }

    #[test]
    fn root_pattern_matches_root() {
        assert!(canonical_path_matches("/svt", "/svt"));
    }
}
