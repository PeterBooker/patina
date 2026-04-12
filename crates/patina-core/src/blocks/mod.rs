//! Gutenberg block parser — `parse_blocks()` equivalent.
//!
//! Port of WordPress's `WP_Block_Parser` (`wp-includes/class-wp-block-parser.php`).
//! The output is byte-for-byte compatible with the PHP implementation: each
//! [`ParsedBlock`] corresponds to the array produced by casting a
//! `WP_Block_Parser_Block` to an associative array in PHP.

pub mod grammar;
pub mod types;

use grammar::next_token;
use serde_json::Value as JsonValue;
use types::{ParsedBlock, TokenKind};

/// Parse a document into a list of top-level blocks.
///
/// Matches the output of WordPress's `parse_blocks()`.
pub fn parse_blocks(document: &str) -> Vec<ParsedBlock> {
    let mut parser = BlockParser::new(document.as_bytes());
    while parser.proceed() {}
    parser.output
}

/// Internal state for the block parser.
struct BlockParser<'a> {
    document: &'a [u8],
    offset: usize,
    output: Vec<ParsedBlock>,
    stack: Vec<StackFrame>,
}

/// A partially-parsed block on the parser's stack, waiting for its closer.
struct StackFrame {
    block: ParsedBlock,
    token_start: usize,
    token_length: usize,
    prev_offset: usize,
    leading_html_start: Option<usize>,
}

impl<'a> BlockParser<'a> {
    fn new(document: &'a [u8]) -> Self {
        Self {
            document,
            offset: 0,
            output: Vec::new(),
            stack: Vec::new(),
        }
    }

    /// Consume the next token and advance the parse state by one step.
    /// Returns `false` when the parser has finished producing tokens.
    ///
    /// Direct translation of `WP_Block_Parser::proceed()`.
    fn proceed(&mut self) -> bool {
        let token = next_token(self.document, self.offset);
        let stack_depth = self.stack.len();

        let leading_html_start = if token.start_offset > self.offset {
            Some(self.offset)
        } else {
            None
        };

        match token.kind {
            TokenKind::NoMoreTokens => {
                if stack_depth == 0 {
                    self.add_freeform(None);
                    return false;
                }

                // One unclosed block — assume an implicit closer.
                if stack_depth == 1 {
                    self.add_block_from_stack(None);
                    return false;
                }

                // Multiple unclosed blocks — collapse the whole stack.
                while !self.stack.is_empty() {
                    self.add_block_from_stack(None);
                }
                false
            }

            TokenKind::VoidBlock => {
                let block = self.make_block(
                    token.block_name.unwrap_or_default(),
                    token.attrs,
                    Vec::new(),
                    String::new(),
                    Vec::new(),
                );

                if stack_depth == 0 {
                    if let Some(lhs) = leading_html_start {
                        let html = bytes_to_string(&self.document[lhs..token.start_offset]);
                        self.output.push(ParsedBlock::freeform(html));
                    }
                    self.output.push(block);
                    self.offset = token.start_offset + token.token_length;
                    return true;
                }

                self.add_inner_block(block, token.start_offset, token.token_length, None);
                self.offset = token.start_offset + token.token_length;
                true
            }

            TokenKind::BlockOpener => {
                let block = self.make_block(
                    token.block_name.unwrap_or_default(),
                    token.attrs,
                    Vec::new(),
                    String::new(),
                    Vec::new(),
                );
                self.stack.push(StackFrame {
                    block,
                    token_start: token.start_offset,
                    token_length: token.token_length,
                    prev_offset: token.start_offset + token.token_length,
                    leading_html_start,
                });
                self.offset = token.start_offset + token.token_length;
                true
            }

            TokenKind::BlockCloser => {
                if stack_depth == 0 {
                    // Unmatched closer — treat the whole document as freeform.
                    self.add_freeform(None);
                    return false;
                }

                if stack_depth == 1 {
                    self.add_block_from_stack(Some(token.start_offset));
                    self.offset = token.start_offset + token.token_length;
                    return true;
                }

                // Nested case: pop the current block and attach it as an
                // inner block of the new top-of-stack parent.
                let mut stack_top = self.stack.pop().expect("stack non-empty");
                let html =
                    bytes_to_string(&self.document[stack_top.prev_offset..token.start_offset]);
                stack_top.block.inner_html.push_str(&html);
                stack_top.block.inner_content.push(Some(html));
                stack_top.prev_offset = token.start_offset + token.token_length;

                let last_offset = Some(token.start_offset + token.token_length);
                self.add_inner_block(
                    stack_top.block,
                    stack_top.token_start,
                    stack_top.token_length,
                    last_offset,
                );
                self.offset = token.start_offset + token.token_length;
                true
            }
        }
    }

    fn make_block(
        &self,
        name: String,
        attrs: Option<JsonValue>,
        inner_blocks: Vec<ParsedBlock>,
        inner_html: String,
        inner_content: Vec<Option<String>>,
    ) -> ParsedBlock {
        ParsedBlock {
            block_name: Some(name),
            // No attrs → empty PHP array → serialized as `[]`.
            attrs: attrs.unwrap_or_else(|| JsonValue::Array(Vec::new())),
            inner_blocks,
            inner_html,
            inner_content,
        }
    }

    /// Push a length of text from the current offset as a freeform block.
    /// Direct translation of `WP_Block_Parser::add_freeform()`.
    fn add_freeform(&mut self, length: Option<usize>) {
        let length = length.unwrap_or_else(|| self.document.len() - self.offset);
        if length == 0 {
            return;
        }
        let html = bytes_to_string(&self.document[self.offset..self.offset + length]);
        self.output.push(ParsedBlock::freeform(html));
    }

    /// Attach a block as an inner block of the current top-of-stack parent.
    /// Direct translation of `WP_Block_Parser::add_inner_block()`.
    fn add_inner_block(
        &mut self,
        block: ParsedBlock,
        token_start: usize,
        token_length: usize,
        last_offset: Option<usize>,
    ) {
        let parent = self
            .stack
            .last_mut()
            .expect("add_inner_block called with empty stack");

        let html = bytes_to_string(&self.document[parent.prev_offset..token_start]);
        if !html.is_empty() {
            parent.block.inner_html.push_str(&html);
            parent.block.inner_content.push(Some(html));
        }

        parent.block.inner_blocks.push(block);
        parent.block.inner_content.push(None);
        parent.prev_offset = last_offset.unwrap_or(token_start + token_length);
    }

    /// Pop the stack top and push it onto `output`.
    /// Direct translation of `WP_Block_Parser::add_block_from_stack()`.
    fn add_block_from_stack(&mut self, end_offset: Option<usize>) {
        let mut stack_top = self.stack.pop().expect("stack non-empty");
        let prev_offset = stack_top.prev_offset;

        let html = match end_offset {
            Some(end) => bytes_to_string(&self.document[prev_offset..end]),
            None => bytes_to_string(&self.document[prev_offset..]),
        };

        if !html.is_empty() {
            stack_top.block.inner_html.push_str(&html);
            stack_top.block.inner_content.push(Some(html));
        }

        if let Some(lhs) = stack_top.leading_html_start {
            let leading = bytes_to_string(&self.document[lhs..stack_top.token_start]);
            self.output.push(ParsedBlock::freeform(leading));
        }

        self.output.push(stack_top.block);
    }
}

/// Convert a byte slice to a String, replacing invalid UTF-8 sequences with
/// U+FFFD. WordPress's PHP parser uses `substr()` which is byte-level and
/// doesn't care about encoding; for Rust output we use lossy conversion
/// so we never panic on non-UTF-8 bytes (which are rare but possible in
/// WordPress content).
fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document_produces_empty_output() {
        assert_eq!(parse_blocks(""), vec![]);
    }

    #[test]
    fn plain_text_becomes_single_freeform_block() {
        let blocks = parse_blocks("hello world");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name, None);
        assert_eq!(blocks[0].inner_html, "hello world");
    }

    #[test]
    fn single_void_block() {
        let blocks = parse_blocks("<!-- wp:separator /-->");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name.as_deref(), Some("core/separator"));
        assert_eq!(blocks[0].inner_blocks.len(), 0);
        assert_eq!(blocks[0].inner_html, "");
    }

    #[test]
    fn single_wrapped_block() {
        let blocks = parse_blocks("<!-- wp:paragraph -->Hello<!-- /wp:paragraph -->");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name.as_deref(), Some("core/paragraph"));
        assert_eq!(blocks[0].inner_html, "Hello");
        assert_eq!(blocks[0].inner_content, vec![Some("Hello".to_string())]);
    }

    #[test]
    fn leading_freeform_before_block() {
        let blocks = parse_blocks("prefix text<!-- wp:separator /-->");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_name, None);
        assert_eq!(blocks[0].inner_html, "prefix text");
        assert_eq!(blocks[1].block_name.as_deref(), Some("core/separator"));
    }

    #[test]
    fn trailing_freeform_after_block() {
        let blocks = parse_blocks("<!-- wp:separator /-->trailing text");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_name.as_deref(), Some("core/separator"));
        assert_eq!(blocks[1].block_name, None);
        assert_eq!(blocks[1].inner_html, "trailing text");
    }

    #[test]
    fn two_adjacent_wrapped_blocks() {
        let blocks = parse_blocks(
            "<!-- wp:paragraph -->One<!-- /wp:paragraph --><!-- wp:paragraph -->Two<!-- /wp:paragraph -->",
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].inner_html, "One");
        assert_eq!(blocks[1].inner_html, "Two");
    }

    #[test]
    fn nested_block() {
        let input = "\
<!-- wp:group -->\
<div class=\"wp-block-group\">\
<!-- wp:paragraph -->Hello<!-- /wp:paragraph -->\
</div>\
<!-- /wp:group -->";
        let blocks = parse_blocks(input);
        assert_eq!(blocks.len(), 1);
        let group = &blocks[0];
        assert_eq!(group.block_name.as_deref(), Some("core/group"));
        assert_eq!(group.inner_blocks.len(), 1);
        let para = &group.inner_blocks[0];
        assert_eq!(para.block_name.as_deref(), Some("core/paragraph"));
        assert_eq!(para.inner_html, "Hello");
    }

    #[test]
    fn nested_inner_content_layout() {
        let input = "\
<!-- wp:group --><div class=\"wp-block-group\">\
<!-- wp:paragraph -->A<!-- /wp:paragraph -->\
</div><!-- /wp:group -->";
        let blocks = parse_blocks(input);
        let group = &blocks[0];
        // innerContent should be: Some(before), None (inner block), Some(after)
        assert_eq!(group.inner_content.len(), 3);
        assert_eq!(
            group.inner_content[0].as_deref(),
            Some("<div class=\"wp-block-group\">")
        );
        assert_eq!(group.inner_content[1], None);
        assert_eq!(group.inner_content[2].as_deref(), Some("</div>"));
    }

    #[test]
    fn block_with_attrs_parsed_as_json() {
        let blocks = parse_blocks(r#"<!-- wp:heading {"level":3} -->H<!-- /wp:heading -->"#);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].attrs.get("level").and_then(|v| v.as_i64()),
            Some(3)
        );
    }

    #[test]
    fn block_without_attrs_uses_empty_array() {
        let blocks = parse_blocks("<!-- wp:paragraph -->x<!-- /wp:paragraph -->");
        // No attrs → matches PHP's `array()` → serialized as `[]`
        assert_eq!(blocks[0].attrs, serde_json::json!([]));
    }

    #[test]
    fn unclosed_block_collapses_to_freeform_plus_block() {
        // WP assumes implicit closer for stack depth 1.
        let blocks = parse_blocks("<!-- wp:paragraph -->dangling");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_name.as_deref(), Some("core/paragraph"));
    }

    #[test]
    fn freeform_block_has_empty_array_attrs() {
        let blocks = parse_blocks("plain");
        assert_eq!(blocks[0].attrs, serde_json::json!([]));
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("parse_blocks");
        assert!(!fixtures.is_empty(), "no fixtures loaded");

        let mut mismatches = Vec::new();
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().expect("input[0] should be a string");
            let expected = &f.output;

            let parsed = parse_blocks(input);
            let actual = serde_json::Value::Array(
                parsed
                    .iter()
                    .map(types::ParsedBlock::to_json_value)
                    .collect(),
            );

            if &actual != expected {
                mismatches.push((i, input.to_string(), expected.clone(), actual));
            }
        }

        if !mismatches.is_empty() {
            for (idx, input, expected, actual) in &mismatches {
                eprintln!("\n=== fixture {idx} MISMATCH ===");
                eprintln!("  Input:    {input:?}");
                eprintln!(
                    "  Expected: {}",
                    serde_json::to_string_pretty(expected).unwrap_or_default()
                );
                eprintln!(
                    "  Got:      {}",
                    serde_json::to_string_pretty(actual).unwrap_or_default()
                );
            }
            panic!(
                "{} fixture mismatch(es) out of {}",
                mismatches.len(),
                fixtures.len()
            );
        }
    }
}
