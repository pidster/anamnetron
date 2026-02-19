//! Svelte script block extraction.
//!
//! Extracts `<script>` block content from `.svelte` files so it can be
//! parsed as TypeScript by tree-sitter-typescript.

/// A script block extracted from a Svelte file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptBlock {
    /// The TypeScript/JavaScript content of the script block.
    pub content: String,
    /// The line offset of the `<script>` tag in the original `.svelte` file.
    /// Used to adjust source_ref line numbers.
    pub line_offset: usize,
    /// Whether this is a `<script context="module">` block.
    pub is_module: bool,
}

/// Extract `<script>` blocks from a Svelte source file.
///
/// Finds `<script>` or `<script lang="ts">` tags, extracts the content
/// between the opening and closing tags, and records the line offset for
/// correct source_ref generation.
pub fn extract_script_blocks(source: &str) -> Vec<ScriptBlock> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while let Some(open_start) = source[search_from..].find("<script") {
        let abs_open_start = search_from + open_start;

        // Find the end of the opening tag
        let Some(open_end) = source[abs_open_start..].find('>') else {
            break;
        };
        let abs_open_end = abs_open_start + open_end + 1;

        // Extract the opening tag text to check attributes
        let tag_text = &source[abs_open_start..abs_open_end];

        // Check for context="module"
        let is_module =
            tag_text.contains("context=\"module\"") || tag_text.contains("context='module'");

        // Find the closing </script> tag
        let Some(close_start) = source[abs_open_end..].find("</script>") else {
            break; // Unclosed tag -- skip
        };
        let abs_close_start = abs_open_end + close_start;

        // Extract content between tags
        let content = source[abs_open_end..abs_close_start].to_string();

        // Calculate line offset (count newlines before the opening tag)
        let line_offset = source[..abs_open_start].matches('\n').count();

        blocks.push(ScriptBlock {
            content,
            line_offset,
            is_module,
        });

        search_from = abs_close_start + "</script>".len();
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_simple_script_block() {
        let source = r#"<script lang="ts">
export let name: string = "world";
</script>

<h1>Hello {name}!</h1>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].content.trim(),
            r#"export let name: string = "world";"#
        );
        assert_eq!(blocks[0].line_offset, 0);
        assert!(!blocks[0].is_module);
    }

    #[test]
    fn extracts_module_script_block() {
        let source = r#"<script context="module" lang="ts">
export function helper() { return 42; }
</script>

<script lang="ts">
let count = 0;
</script>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 2);
        assert!(
            blocks.iter().any(|b| b.is_module),
            "should find module script"
        );
        assert!(
            blocks.iter().any(|b| !b.is_module),
            "should find instance script"
        );
    }

    #[test]
    fn extracts_script_without_lang_attribute() {
        let source = r#"<script>
let x = 1;
</script>"#;
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].content.trim(), "let x = 1;");
    }

    #[test]
    fn line_offset_is_correct() {
        let source = "<p>hello</p>\n<p>world</p>\n<script lang=\"ts\">\nlet x = 1;\n</script>";
        let blocks = extract_script_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].line_offset, 2,
            "script tag is on line 2 (0-indexed)"
        );
    }

    #[test]
    fn no_script_block_returns_empty() {
        let source = "<h1>Hello world</h1>\n<p>No script here</p>";
        let blocks = extract_script_blocks(source);
        assert!(blocks.is_empty());
    }

    #[test]
    fn malformed_unclosed_script_returns_empty() {
        let source = "<script lang=\"ts\">\nlet x = 1;";
        let blocks = extract_script_blocks(source);
        assert!(blocks.is_empty(), "unclosed script tag should be skipped");
    }
}
