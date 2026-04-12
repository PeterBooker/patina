//! Block data types returned by the parser.
//!
//! Mirrors WordPress's `WP_Block_Parser_Block` structure. A document parses
//! into `Vec<ParsedBlock>`; each block may contain nested inner blocks.

use serde_json::Value as JsonValue;

/// A single parsed block, matching the shape of WordPress's
/// `(array) new WP_Block_Parser_Block(...)`.
///
/// Field names match PHP's property names (camelCase) so that serialization
/// to a PHP associative array is a 1:1 mapping.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedBlock {
    /// `blockName`: full name like `"core/paragraph"`, or `None` for freeform
    /// HTML blocks (pre-block text and content between blocks).
    pub block_name: Option<String>,

    /// `attrs`: parsed JSON attributes from block comment delimiters.
    ///
    /// WordPress stores this as a PHP array. When there are no attributes,
    /// or an empty JSON object `{}`, PHP uses an empty `array()`, which
    /// serializes to `[]`. When there are attributes, it's an associative
    /// PHP array, serialized to a JSON object `{"key":"value"}`.
    ///
    /// We use `serde_json::Value` directly so the serialized fixture output
    /// matches WordPress byte-for-byte.
    pub attrs: JsonValue,

    /// `innerBlocks`: child blocks nested inside this one.
    pub inner_blocks: Vec<ParsedBlock>,

    /// `innerHTML`: concatenation of the HTML fragments between inner blocks
    /// (or the whole inner HTML if there are no inner blocks).
    pub inner_html: String,

    /// `innerContent`: ordered list of HTML fragments interleaved with `null`
    /// markers for each inner block.
    ///
    /// Example: if the block has `"Before"`, an inner block, `"Inner"`, then
    /// another inner block, `"After"`, this becomes
    /// `[Some("Before"), None, Some("Inner"), None, Some("After")]`.
    pub inner_content: Vec<Option<String>>,
}

impl ParsedBlock {
    /// Construct a freeform (raw HTML) block — `blockName` is `None`, `attrs`
    /// is `[]`, no inner blocks, `innerHTML` is the HTML, and `innerContent`
    /// holds a single entry with the same HTML.
    ///
    /// This matches WordPress's `WP_Block_Parser::freeform()`.
    pub fn freeform(html: String) -> Self {
        Self {
            block_name: None,
            attrs: JsonValue::Array(Vec::new()),
            inner_blocks: Vec::new(),
            inner_html: html.clone(),
            inner_content: vec![Some(html)],
        }
    }

    /// Serialize to a `serde_json::Value` matching WordPress's
    /// `json_encode((array) $block)` output. Keys are in the same order
    /// as PHP produces them: `blockName`, `attrs`, `innerBlocks`,
    /// `innerHTML`, `innerContent`.
    pub fn to_json_value(&self) -> JsonValue {
        let mut map = serde_json::Map::with_capacity(5);
        map.insert(
            "blockName".to_string(),
            match &self.block_name {
                Some(s) => JsonValue::String(s.clone()),
                None => JsonValue::Null,
            },
        );
        map.insert("attrs".to_string(), self.attrs.clone());
        map.insert(
            "innerBlocks".to_string(),
            JsonValue::Array(self.inner_blocks.iter().map(Self::to_json_value).collect()),
        );
        map.insert(
            "innerHTML".to_string(),
            JsonValue::String(self.inner_html.clone()),
        );
        map.insert(
            "innerContent".to_string(),
            JsonValue::Array(
                self.inner_content
                    .iter()
                    .map(|item| match item {
                        Some(s) => JsonValue::String(s.clone()),
                        None => JsonValue::Null,
                    })
                    .collect(),
            ),
        );
        JsonValue::Object(map)
    }
}

/// Classification of a scanned token in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// End of document reached — no more tokens.
    NoMoreTokens,
    /// `<!-- wp:name /-->` — a self-closing block.
    VoidBlock,
    /// `<!-- wp:name -->` — opens a block that needs a matching closer.
    BlockOpener,
    /// `<!-- /wp:name -->` — closes a previously-opened block.
    BlockCloser,
}

/// A scanned token from the document, matching the tuple shape returned by
/// WordPress's `WP_Block_Parser::next_token()`.
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub block_name: Option<String>,
    pub attrs: Option<JsonValue>,
    pub start_offset: usize,
    pub token_length: usize,
}

impl Token {
    pub fn no_more_tokens() -> Self {
        Self {
            kind: TokenKind::NoMoreTokens,
            block_name: None,
            attrs: None,
            start_offset: 0,
            token_length: 0,
        }
    }
}
