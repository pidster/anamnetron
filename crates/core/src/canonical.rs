//! Canonical path utilities: kebab-case conversion, glob matching, path validation.
//!
//! All functions are WASM-safe -- no platform-specific dependencies.

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
            if prev_lower || (prev_upper && next_lower) {
                if !result.is_empty() && !result.ends_with('-') {
                    result.push('-');
                }
            }
        }

        result.push(c.to_ascii_lowercase());
    }

    result
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
}
