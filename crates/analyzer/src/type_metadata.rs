//! Language-agnostic type metadata helpers for data flow analysis.
//!
//! This module provides shared abstractions for extracting, filtering, and
//! building data flow metadata from function signatures. Language-specific
//! parsers handle AST extraction (tree-sitter), then delegate to these helpers
//! for normalization, filtering, and metadata construction.

use serde::{Deserialize, Serialize};

/// Typed representation of data flow metadata for function signatures.
///
/// Captures the parameter types and return type of a function, filtered to
/// include only project-local (non-primitive, non-standard-library) types
/// that are meaningful for data flow analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataFlowMetadata {
    /// Parameter types that carry project-local data.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub param_types: Vec<ParamType>,
    /// Return type, if it is a project-local data type.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub return_type: Option<String>,
}

/// A single parameter's name and resolved type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamType {
    /// Parameter name (e.g., `msg`, `config`).
    pub name: String,
    /// Resolved qualified type name (e.g., `my_crate::model::Node`).
    #[serde(rename = "type")]
    pub type_name: String,
}

/// Language-specific configuration for type metadata extraction.
///
/// Each supported language provides a static instance describing which types
/// to skip (primitives, standard library types) and which container types to
/// unwrap (e.g., `Result<T, E>` → `T`).
pub struct LanguageTypeConfig {
    /// Types to skip (primitives, standard library types).
    ///
    /// A type is skipped when its short name (the last segment after `::`)
    /// matches any entry in this list.
    pub skip_types: &'static [&'static str],
    /// Container types that wrap an inner data type.
    ///
    /// When a type name matches one of these, the inner type argument is
    /// extracted. For example, `Result<MyType, Error>` unwraps to `MyType`
    /// when `"Result"` is in this list.
    pub wrapper_types: &'static [&'static str],
}

/// Rust language type configuration.
///
/// Skips Rust primitives (`bool`, `u8`–`u128`, `i8`–`i128`, `f32`, `f64`,
/// `usize`, `isize`), string types (`str`, `String`), and common standard
/// library containers (`Vec`, `Box`, `Arc`, `Rc`, `Cow`).
/// Unwraps `Result` and `Option`.
pub const RUST_TYPE_CONFIG: LanguageTypeConfig = LanguageTypeConfig {
    skip_types: &[
        "bool", "char", "str", "String", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16",
        "i32", "i64", "i128", "isize", "f32", "f64", "Vec", "Box", "Arc", "Rc", "Cow",
    ],
    wrapper_types: &["Result", "Option"],
};

/// Go language type configuration.
///
/// Skips Go built-in types and common primitives. Go does not have generic
/// wrapper types (pre-generics), but `error` returns are filtered.
pub const GO_TYPE_CONFIG: LanguageTypeConfig = LanguageTypeConfig {
    skip_types: &[
        "bool", "int", "int8", "int16", "int32", "int64", "uint", "uint8", "uint16", "uint32",
        "uint64", "float32", "float64", "string", "byte", "rune", "error",
    ],
    wrapper_types: &[],
};

/// TypeScript language type configuration.
///
/// Skips TypeScript/JavaScript primitive types. Unwraps `Promise`,
/// `Observable`, and `Array` to their inner type argument.
pub const TYPESCRIPT_TYPE_CONFIG: LanguageTypeConfig = LanguageTypeConfig {
    skip_types: &[
        "string",
        "number",
        "boolean",
        "void",
        "undefined",
        "null",
        "any",
        "unknown",
        "never",
        "object",
        "symbol",
        "bigint",
    ],
    wrapper_types: &["Promise", "Observable", "Array"],
};

/// Java language type configuration.
///
/// Skips Java primitives and common boxed types. Unwraps `Optional`,
/// `CompletableFuture`, `List`, and `Set`.
pub const JAVA_TYPE_CONFIG: LanguageTypeConfig = LanguageTypeConfig {
    skip_types: &[
        "int", "long", "short", "byte", "float", "double", "boolean", "char", "void", "String",
        "Object",
    ],
    wrapper_types: &["Optional", "CompletableFuture", "List", "Set"],
};

/// Python language type configuration.
///
/// Skips Python built-in types. Unwraps `Optional`, `List`, `Dict`, `Set`,
/// and `Tuple`.
pub const PYTHON_TYPE_CONFIG: LanguageTypeConfig = LanguageTypeConfig {
    skip_types: &[
        "int", "float", "str", "bool", "bytes", "None", "Any", "object",
    ],
    wrapper_types: &["Optional", "List", "Dict", "Set", "Tuple"],
};

/// Check if a type name should be skipped for data flow purposes.
///
/// A type is skipped when its short name (the segment after the last `::`)
/// matches any entry in `config.skip_types`.
///
/// # Examples
///
/// ```
/// use svt_analyzer::type_metadata::{is_skip_type, RUST_TYPE_CONFIG};
///
/// assert!(is_skip_type("u32", &RUST_TYPE_CONFIG));
/// assert!(is_skip_type("my_crate::String", &RUST_TYPE_CONFIG));
/// assert!(!is_skip_type("my_crate::MyType", &RUST_TYPE_CONFIG));
/// ```
#[must_use]
pub fn is_skip_type(type_name: &str, config: &LanguageTypeConfig) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);
    config.skip_types.contains(&short_name)
}

/// Unwrap container/wrapper types to get the inner data type.
///
/// Strips wrapper types defined in `config.wrapper_types` by extracting the
/// first type argument from angle brackets. Handles nested wrappers
/// (e.g., `Option<Result<T, E>>` → `T`). Returns the type unchanged if it
/// is not a wrapper.
///
/// # Examples
///
/// ```
/// use svt_analyzer::type_metadata::{unwrap_wrapper_type, RUST_TYPE_CONFIG};
///
/// assert_eq!(unwrap_wrapper_type("Result<MyType, Error>", &RUST_TYPE_CONFIG), "MyType");
/// assert_eq!(unwrap_wrapper_type("Option<Foo>", &RUST_TYPE_CONFIG), "Foo");
/// assert_eq!(unwrap_wrapper_type("PlainType", &RUST_TYPE_CONFIG), "PlainType");
/// ```
#[must_use]
pub fn unwrap_wrapper_type(type_name: &str, config: &LanguageTypeConfig) -> String {
    let trimmed = type_name.trim();

    // Check if the type matches any wrapper pattern: WrapperName<...>
    if let Some(angle_pos) = trimmed.find('<') {
        let base = trimmed[..angle_pos].trim();
        // Get the short name for matching (handle qualified names like std::option::Option)
        let short_base = base.rsplit("::").next().unwrap_or(base);

        if config.wrapper_types.contains(&short_base) {
            // Extract the first type argument.
            // For Result<T, E>, we want T. For Option<T>, we want T.
            if let Some(inner) = extract_first_type_arg(trimmed) {
                // Recursively unwrap in case of nested wrappers.
                return unwrap_wrapper_type(&inner, config);
            }
        }
    }

    trimmed.to_string()
}

/// Build [`DataFlowMetadata`] from parameter types and a return type.
///
/// Filters out skip types and unwraps wrapper types using the provided
/// language configuration. Returns `None` if no project-local types remain
/// after filtering.
///
/// # Arguments
///
/// * `param_types` — Pairs of `(parameter_name, type_name)` extracted from
///   the function signature by the language-specific parser.
/// * `return_type` — The return type string, already extracted from the AST.
/// * `config` — Language-specific type configuration.
#[must_use]
pub fn build_data_flow_metadata(
    param_types: &[(String, String)],
    return_type: Option<&str>,
    config: &LanguageTypeConfig,
) -> Option<DataFlowMetadata> {
    let filtered_params: Vec<ParamType> = param_types
        .iter()
        .filter_map(|(name, type_name)| {
            let unwrapped = unwrap_wrapper_type(type_name, config);
            if is_skip_type(&unwrapped, config) {
                None
            } else {
                Some(ParamType {
                    name: name.clone(),
                    type_name: unwrapped,
                })
            }
        })
        .collect();

    let filtered_return = return_type.and_then(|rt| {
        let unwrapped = unwrap_wrapper_type(rt, config);
        if is_skip_type(&unwrapped, config) {
            None
        } else {
            Some(unwrapped)
        }
    });

    if filtered_params.is_empty() && filtered_return.is_none() {
        return None;
    }

    Some(DataFlowMetadata {
        param_types: filtered_params,
        return_type: filtered_return,
    })
}

/// Merge [`DataFlowMetadata`] into an existing `serde_json::Value` metadata object.
///
/// Inserts `param_types` (if non-empty) and `return_type` (if present) into
/// the given metadata object. If `metadata` is not a JSON object, this is a
/// no-op.
pub fn merge_into_metadata(metadata: &mut serde_json::Value, data_flow: &DataFlowMetadata) {
    let Some(obj) = metadata.as_object_mut() else {
        return;
    };

    if !data_flow.param_types.is_empty() {
        let params: Vec<serde_json::Value> = data_flow
            .param_types
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "type": p.type_name,
                })
            })
            .collect();
        obj.insert("param_types".to_string(), serde_json::Value::Array(params));
    }

    if let Some(ref rt) = data_flow.return_type {
        obj.insert(
            "return_type".to_string(),
            serde_json::Value::String(rt.clone()),
        );
    }
}

/// Extract the first type argument from a generic type string.
///
/// Given `"Result<MyType, Error>"`, returns `Some("MyType")`.
/// Given `"Option<Foo>"`, returns `Some("Foo")`.
/// Handles nested angle brackets correctly.
fn extract_first_type_arg(type_str: &str) -> Option<String> {
    let open = type_str.find('<')?;
    let after_open = open + 1;
    let inner = &type_str[after_open..];

    // Find the end of the first type argument, respecting nested angle brackets.
    let mut depth = 0;
    let mut end = None;
    for (i, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                if depth == 0 {
                    end = Some(i);
                    break;
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                end = Some(i);
                break;
            }
            _ => {}
        }
    }

    let end = end?;
    let first_arg = inner[..end].trim();
    if first_arg.is_empty() {
        return None;
    }
    Some(first_arg.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_skip_type tests ----

    #[test]
    fn is_skip_type_rust_primitives() {
        assert!(is_skip_type("bool", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("u32", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("i64", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("f32", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("usize", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("str", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("String", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("char", &RUST_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_rust_containers() {
        assert!(is_skip_type("Vec", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("Box", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("Arc", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("Rc", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("Cow", &RUST_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_rust_qualified_names() {
        assert!(is_skip_type("std::string::String", &RUST_TYPE_CONFIG));
        assert!(is_skip_type("my_crate::u32", &RUST_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_rust_project_types_not_skipped() {
        assert!(!is_skip_type("MyStruct", &RUST_TYPE_CONFIG));
        assert!(!is_skip_type("my_crate::model::Node", &RUST_TYPE_CONFIG));
        assert!(!is_skip_type("Config", &RUST_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_go_primitives() {
        assert!(is_skip_type("bool", &GO_TYPE_CONFIG));
        assert!(is_skip_type("int", &GO_TYPE_CONFIG));
        assert!(is_skip_type("int64", &GO_TYPE_CONFIG));
        assert!(is_skip_type("float32", &GO_TYPE_CONFIG));
        assert!(is_skip_type("string", &GO_TYPE_CONFIG));
        assert!(is_skip_type("byte", &GO_TYPE_CONFIG));
        assert!(is_skip_type("rune", &GO_TYPE_CONFIG));
        assert!(is_skip_type("error", &GO_TYPE_CONFIG));
        assert!(!is_skip_type("MyService", &GO_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_typescript_primitives() {
        assert!(is_skip_type("string", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("number", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("boolean", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("void", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("any", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("unknown", &TYPESCRIPT_TYPE_CONFIG));
        assert!(is_skip_type("never", &TYPESCRIPT_TYPE_CONFIG));
        assert!(!is_skip_type("UserService", &TYPESCRIPT_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_java_primitives() {
        assert!(is_skip_type("int", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("long", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("double", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("boolean", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("String", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("Object", &JAVA_TYPE_CONFIG));
        assert!(is_skip_type("void", &JAVA_TYPE_CONFIG));
        assert!(!is_skip_type("UserRepository", &JAVA_TYPE_CONFIG));
    }

    #[test]
    fn is_skip_type_python_primitives() {
        assert!(is_skip_type("int", &PYTHON_TYPE_CONFIG));
        assert!(is_skip_type("float", &PYTHON_TYPE_CONFIG));
        assert!(is_skip_type("str", &PYTHON_TYPE_CONFIG));
        assert!(is_skip_type("bool", &PYTHON_TYPE_CONFIG));
        assert!(is_skip_type("None", &PYTHON_TYPE_CONFIG));
        assert!(is_skip_type("Any", &PYTHON_TYPE_CONFIG));
        assert!(!is_skip_type("DataFrame", &PYTHON_TYPE_CONFIG));
    }

    // ---- unwrap_wrapper_type tests ----

    #[test]
    fn unwrap_result_type() {
        assert_eq!(
            unwrap_wrapper_type("Result<MyType, Error>", &RUST_TYPE_CONFIG),
            "MyType"
        );
    }

    #[test]
    fn unwrap_option_type() {
        assert_eq!(unwrap_wrapper_type("Option<Foo>", &RUST_TYPE_CONFIG), "Foo");
    }

    #[test]
    fn unwrap_nested_wrappers() {
        assert_eq!(
            unwrap_wrapper_type("Option<Result<Bar, Error>>", &RUST_TYPE_CONFIG),
            "Bar"
        );
        assert_eq!(
            unwrap_wrapper_type("Result<Option<Baz>, Error>", &RUST_TYPE_CONFIG),
            "Baz"
        );
    }

    #[test]
    fn unwrap_plain_type_passes_through() {
        assert_eq!(
            unwrap_wrapper_type("PlainType", &RUST_TYPE_CONFIG),
            "PlainType"
        );
        assert_eq!(
            unwrap_wrapper_type("my::qualified::Type", &RUST_TYPE_CONFIG),
            "my::qualified::Type"
        );
    }

    #[test]
    fn unwrap_promise_type() {
        assert_eq!(
            unwrap_wrapper_type("Promise<UserData>", &TYPESCRIPT_TYPE_CONFIG),
            "UserData"
        );
    }

    #[test]
    fn unwrap_observable_type() {
        assert_eq!(
            unwrap_wrapper_type("Observable<Event>", &TYPESCRIPT_TYPE_CONFIG),
            "Event"
        );
    }

    #[test]
    fn unwrap_array_type() {
        assert_eq!(
            unwrap_wrapper_type("Array<Item>", &TYPESCRIPT_TYPE_CONFIG),
            "Item"
        );
    }

    #[test]
    fn unwrap_java_optional() {
        assert_eq!(
            unwrap_wrapper_type("Optional<User>", &JAVA_TYPE_CONFIG),
            "User"
        );
    }

    #[test]
    fn unwrap_java_completable_future() {
        assert_eq!(
            unwrap_wrapper_type("CompletableFuture<Response>", &JAVA_TYPE_CONFIG),
            "Response"
        );
    }

    #[test]
    fn unwrap_java_list() {
        assert_eq!(
            unwrap_wrapper_type("List<Order>", &JAVA_TYPE_CONFIG),
            "Order"
        );
    }

    #[test]
    fn unwrap_python_optional() {
        assert_eq!(
            unwrap_wrapper_type("Optional<Config>", &PYTHON_TYPE_CONFIG),
            "Config"
        );
    }

    #[test]
    fn unwrap_python_list() {
        assert_eq!(
            unwrap_wrapper_type("List<Record>", &PYTHON_TYPE_CONFIG),
            "Record"
        );
    }

    #[test]
    fn unwrap_non_wrapper_generic_passes_through() {
        // HashMap is not in RUST_TYPE_CONFIG.wrapper_types
        assert_eq!(
            unwrap_wrapper_type("HashMap<String, Value>", &RUST_TYPE_CONFIG),
            "HashMap<String, Value>"
        );
    }

    #[test]
    fn unwrap_go_has_no_wrappers() {
        // Go config has no wrapper types, so everything passes through.
        assert_eq!(unwrap_wrapper_type("MyStruct", &GO_TYPE_CONFIG), "MyStruct");
    }

    // ---- extract_first_type_arg tests ----

    #[test]
    fn extract_first_arg_simple() {
        assert_eq!(
            extract_first_type_arg("Option<Foo>"),
            Some("Foo".to_string())
        );
    }

    #[test]
    fn extract_first_arg_with_second() {
        assert_eq!(
            extract_first_type_arg("Result<MyType, Error>"),
            Some("MyType".to_string())
        );
    }

    #[test]
    fn extract_first_arg_nested() {
        assert_eq!(
            extract_first_type_arg("Option<Result<T, E>>"),
            Some("Result<T, E>".to_string())
        );
    }

    #[test]
    fn extract_first_arg_no_generics() {
        assert_eq!(extract_first_type_arg("PlainType"), None);
    }

    #[test]
    fn extract_first_arg_empty_angle_brackets() {
        assert_eq!(extract_first_type_arg("Foo<>"), None);
    }

    // ---- build_data_flow_metadata tests ----

    #[test]
    fn build_metadata_filters_primitives() {
        let params = vec![
            ("count".to_string(), "u32".to_string()),
            ("config".to_string(), "my_crate::Config".to_string()),
        ];
        let result = build_data_flow_metadata(&params, Some("bool"), &RUST_TYPE_CONFIG);
        let meta = result.expect("should produce metadata with one project param");
        assert_eq!(meta.param_types.len(), 1);
        assert_eq!(meta.param_types[0].name, "config");
        assert_eq!(meta.param_types[0].type_name, "my_crate::Config");
        assert!(
            meta.return_type.is_none(),
            "bool return type should be filtered"
        );
    }

    #[test]
    fn build_metadata_unwraps_wrappers() {
        let params = vec![("req".to_string(), "Option<Request>".to_string())];
        let result =
            build_data_flow_metadata(&params, Some("Result<Response, Error>"), &RUST_TYPE_CONFIG);
        let meta = result.expect("should produce metadata");
        assert_eq!(meta.param_types.len(), 1);
        assert_eq!(meta.param_types[0].type_name, "Request");
        assert_eq!(meta.return_type.as_deref(), Some("Response"));
    }

    #[test]
    fn build_metadata_returns_none_when_all_filtered() {
        let params = vec![
            ("x".to_string(), "i32".to_string()),
            ("y".to_string(), "f64".to_string()),
        ];
        let result = build_data_flow_metadata(&params, Some("bool"), &RUST_TYPE_CONFIG);
        assert!(
            result.is_none(),
            "all types are primitives, should return None"
        );
    }

    #[test]
    fn build_metadata_returns_none_when_empty() {
        let result = build_data_flow_metadata(&[], None, &RUST_TYPE_CONFIG);
        assert!(result.is_none());
    }

    #[test]
    fn build_metadata_with_only_return_type() {
        let result = build_data_flow_metadata(&[], Some("my_crate::MyResult"), &RUST_TYPE_CONFIG);
        let meta = result.expect("should produce metadata with return type only");
        assert!(meta.param_types.is_empty());
        assert_eq!(meta.return_type.as_deref(), Some("my_crate::MyResult"));
    }

    #[test]
    fn build_metadata_with_only_params() {
        let params = vec![("node".to_string(), "GraphNode".to_string())];
        let result = build_data_flow_metadata(&params, None, &RUST_TYPE_CONFIG);
        let meta = result.expect("should produce metadata with params only");
        assert_eq!(meta.param_types.len(), 1);
        assert!(meta.return_type.is_none());
    }

    #[test]
    fn build_metadata_typescript_promise_unwrap() {
        let params = vec![];
        let result = build_data_flow_metadata(
            &params,
            Some("Promise<UserProfile>"),
            &TYPESCRIPT_TYPE_CONFIG,
        );
        let meta = result.expect("should produce metadata");
        assert_eq!(meta.return_type.as_deref(), Some("UserProfile"));
    }

    #[test]
    fn build_metadata_java_optional_unwrap() {
        let params = vec![("user".to_string(), "Optional<User>".to_string())];
        let result = build_data_flow_metadata(&params, None, &JAVA_TYPE_CONFIG);
        let meta = result.expect("should produce metadata");
        assert_eq!(meta.param_types[0].type_name, "User");
    }

    // ---- merge_into_metadata tests ----

    #[test]
    fn merge_into_metadata_adds_fields() {
        let mut metadata = serde_json::json!({"loc": 42});
        let data_flow = DataFlowMetadata {
            param_types: vec![ParamType {
                name: "req".to_string(),
                type_name: "HttpRequest".to_string(),
            }],
            return_type: Some("HttpResponse".to_string()),
        };
        merge_into_metadata(&mut metadata, &data_flow);

        let obj = metadata.as_object().expect("should be an object");
        assert!(obj.contains_key("param_types"));
        assert!(obj.contains_key("return_type"));
        assert_eq!(obj["loc"], 42);
        assert_eq!(obj["return_type"], "HttpResponse");

        let params = obj["param_types"].as_array().expect("should be array");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0]["name"], "req");
        assert_eq!(params[0]["type"], "HttpRequest");
    }

    #[test]
    fn merge_into_metadata_skips_empty_params() {
        let mut metadata = serde_json::json!({"loc": 10});
        let data_flow = DataFlowMetadata {
            param_types: vec![],
            return_type: Some("MyType".to_string()),
        };
        merge_into_metadata(&mut metadata, &data_flow);

        let obj = metadata.as_object().expect("should be an object");
        assert!(
            !obj.contains_key("param_types"),
            "empty param_types should not be inserted"
        );
        assert_eq!(obj["return_type"], "MyType");
    }

    #[test]
    fn merge_into_metadata_skips_none_return() {
        let mut metadata = serde_json::json!({"loc": 5});
        let data_flow = DataFlowMetadata {
            param_types: vec![ParamType {
                name: "x".to_string(),
                type_name: "Foo".to_string(),
            }],
            return_type: None,
        };
        merge_into_metadata(&mut metadata, &data_flow);

        let obj = metadata.as_object().expect("should be an object");
        assert!(obj.contains_key("param_types"));
        assert!(
            !obj.contains_key("return_type"),
            "None return_type should not be inserted"
        );
    }

    #[test]
    fn merge_into_metadata_noop_on_non_object() {
        let mut metadata = serde_json::json!("not an object");
        let data_flow = DataFlowMetadata {
            param_types: vec![],
            return_type: Some("T".to_string()),
        };
        merge_into_metadata(&mut metadata, &data_flow);
        // Should not panic, value remains unchanged.
        assert_eq!(metadata, serde_json::json!("not an object"));
    }

    // ---- DataFlowMetadata serialization tests ----

    #[test]
    fn data_flow_metadata_serialization_round_trip() {
        let meta = DataFlowMetadata {
            param_types: vec![ParamType {
                name: "msg".to_string(),
                type_name: "my_crate::Message".to_string(),
            }],
            return_type: Some("my_crate::Response".to_string()),
        };

        let json = serde_json::to_string(&meta).expect("should serialize");
        let deserialized: DataFlowMetadata =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(meta, deserialized);
    }

    #[test]
    fn data_flow_metadata_skips_empty_fields_in_json() {
        let meta = DataFlowMetadata {
            param_types: vec![],
            return_type: None,
        };

        let json = serde_json::to_string(&meta).expect("should serialize");
        assert!(!json.contains("param_types"));
        assert!(!json.contains("return_type"));
    }
}
