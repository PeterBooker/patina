//! Block grammar tokenizer.
//!
//! Scans a document for Gutenberg block comment delimiters and classifies
//! each one as a void block, opener, or closer. Hand-rolled byte scanner
//! that matches the semantics of WordPress's `WP_Block_Parser::next_token()`
//! regex byte-for-byte — including the quirk that a `}` followed by ` -->`
//! or ` /-->` is always treated as the terminating attrs brace, even if
//! it appears inside a JSON string literal. This matches WordPress.

use super::types::{Token, TokenKind};
use serde_json::Value as JsonValue;

/// Find the next block delimiter at or after `offset` in `document`.
///
/// Returns a [`Token`] of kind [`TokenKind::NoMoreTokens`] if no valid
/// delimiter is found. Invalid `<!--` sequences (e.g. `<!-- not-a-block -->`)
/// are skipped silently.
pub fn next_token(document: &[u8], offset: usize) -> Token {
    let finder = memchr::memmem::Finder::new(b"<!--");
    let mut search_from = offset;

    while search_from <= document.len() {
        let Some(rel) = finder.find(&document[search_from..]) else {
            return Token::no_more_tokens();
        };
        let start = search_from + rel;

        if let Some(token) = try_parse_delimiter(document, start) {
            return token;
        }
        search_from = start + 1;
    }

    Token::no_more_tokens()
}

/// Attempt to parse a block delimiter starting at `start` (which must point
/// at `<`). Returns `None` if the sequence isn't a valid block delimiter.
fn try_parse_delimiter(doc: &[u8], start: usize) -> Option<Token> {
    // Intro: literal `<!--`
    if doc.get(start..start + 4)? != b"<!--" {
        return None;
    }
    let mut p = start + 4;

    // Required `\s+`
    let ws_start = p;
    while p < doc.len() && is_whitespace(doc[p]) {
        p += 1;
    }
    if p == ws_start {
        return None;
    }

    // Optional closer `/`
    let is_closer = if doc.get(p).copied() == Some(b'/') {
        p += 1;
        true
    } else {
        false
    };

    // Literal `wp:`
    if doc.get(p..p + 3)? != b"wp:" {
        return None;
    }
    p += 3;

    // Optional namespace `[a-z][a-z0-9_-]*/`
    let namespace_end = scan_ident(doc, p);
    let namespace_str = if namespace_end > p && doc.get(namespace_end).copied() == Some(b'/') {
        let ns = &doc[p..namespace_end];
        p = namespace_end + 1;
        Some(ns)
    } else {
        None
    };

    // Required name `[a-z][a-z0-9_-]*`
    let name_end = scan_ident(doc, p);
    if name_end == p {
        return None;
    }
    let name_bytes = &doc[p..name_end];
    p = name_end;

    // Build full name: always prefixed with namespace + "/", defaulting to
    // "core/" when no explicit namespace was given — matches WP.
    let full_name = match namespace_str {
        Some(ns) => {
            let mut s = String::with_capacity(ns.len() + 1 + name_bytes.len());
            s.push_str(std::str::from_utf8(ns).ok()?);
            s.push('/');
            s.push_str(std::str::from_utf8(name_bytes).ok()?);
            s
        }
        None => {
            let mut s = String::with_capacity(5 + name_bytes.len());
            s.push_str("core/");
            s.push_str(std::str::from_utf8(name_bytes).ok()?);
            s
        }
    };

    // Required `\s+` between name and rest
    let ws_start = p;
    while p < doc.len() && is_whitespace(doc[p]) {
        p += 1;
    }
    if p == ws_start {
        return None;
    }

    // Optional attrs group: `{...}\s+`
    let attrs_value = if doc.get(p).copied() == Some(b'{') {
        let (attrs_end, raw_attrs) = scan_json_object(doc, p)?;
        p = attrs_end;

        // Required whitespace after the JSON object
        let ws_start = p;
        while p < doc.len() && is_whitespace(doc[p]) {
            p += 1;
        }
        if p == ws_start {
            return None;
        }

        Some(parse_attrs_json(raw_attrs))
    } else {
        None
    };

    // Optional void marker `/`
    let is_void = if doc.get(p).copied() == Some(b'/') {
        p += 1;
        true
    } else {
        false
    };

    // Literal `-->`
    if doc.get(p..p + 3)? != b"-->" {
        return None;
    }
    p += 3;

    let length = p - start;
    let kind = match (is_closer, is_void) {
        (true, _) => TokenKind::BlockCloser,
        (false, true) => TokenKind::VoidBlock,
        (false, false) => TokenKind::BlockOpener,
    };

    // WP notes that a closer with `void` or attrs is an error but ignores
    // the violation — we do the same: closer discards attrs/void and keeps
    // the closer semantics.
    let attrs = if matches!(kind, TokenKind::BlockCloser) {
        None
    } else {
        attrs_value
    };

    Some(Token {
        kind,
        block_name: Some(full_name),
        attrs,
        start_offset: start,
        token_length: length,
    })
}

/// Scan an identifier matching `[a-z][a-z0-9_-]*` starting at `p`.
/// Returns the byte offset one past the last identifier character, or `p`
/// unchanged if no valid identifier starts there.
fn scan_ident(doc: &[u8], p: usize) -> usize {
    if p >= doc.len() || !doc[p].is_ascii_lowercase() {
        return p;
    }
    let mut end = p + 1;
    while end < doc.len() && is_ident_rest(doc[end]) {
        end += 1;
    }
    end
}

#[inline]
fn is_ident_rest(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-'
}

#[inline]
fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

/// Scan a JSON object starting at `start` (which must point at `{`).
///
/// Returns `(end_offset, raw_bytes)` where `end_offset` is one past the
/// closing `}` and `raw_bytes` is the full `{...}` span including the braces.
///
/// This matches WordPress's regex semantics: the terminating brace is
/// whichever `}` is followed by optional whitespace and then `-->` or
/// `/-->`. That means `}` characters appearing elsewhere (even inside
/// JSON strings, if they happen to be followed by `-->`) are treated as
/// the terminator. WordPress has this same quirk; matching it exactly is
/// important for fixture compatibility.
fn scan_json_object(doc: &[u8], start: usize) -> Option<(usize, &[u8])> {
    if doc.get(start).copied() != Some(b'{') {
        return None;
    }

    let mut i = start + 1;
    while i < doc.len() {
        if doc[i] == b'}' && is_terminating_brace(doc, i) {
            let end = i + 1;
            return Some((end, &doc[start..end]));
        }
        i += 1;
    }
    None
}

/// Check whether the `}` at position `i` is the terminating attrs brace —
/// i.e. the one followed by `\s+ -->` or `\s+ /-->`.
fn is_terminating_brace(doc: &[u8], i: usize) -> bool {
    debug_assert_eq!(doc[i], b'}');

    let mut j = i + 1;
    let ws_start = j;
    while j < doc.len() && is_whitespace(doc[j]) {
        j += 1;
    }
    // WP's regex requires `\s+` (at least one) between `}` and `-->` or `/-->`.
    if j == ws_start {
        return false;
    }
    // Either `-->` or `/-->`
    if doc.get(j..j + 3) == Some(b"-->") {
        return true;
    }
    if doc.get(j).copied() == Some(b'/') && doc.get(j + 1..j + 4) == Some(b"-->") {
        return true;
    }
    false
}

/// Decode the raw JSON object bytes into a `serde_json::Value`.
/// Falls back to `Value::Null` on decode failure — matches PHP's
/// `json_decode` returning `null` for invalid JSON.
fn parse_attrs_json(raw: &[u8]) -> JsonValue {
    serde_json::from_slice(raw).unwrap_or(JsonValue::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_kind(doc: &str, offset: usize) -> TokenKind {
        next_token(doc.as_bytes(), offset).kind
    }

    #[test]
    fn no_tokens_in_empty_document() {
        assert_eq!(token_kind("", 0), TokenKind::NoMoreTokens);
    }

    #[test]
    fn no_tokens_in_plain_text() {
        assert_eq!(
            token_kind("hello world, no blocks here", 0),
            TokenKind::NoMoreTokens
        );
    }

    #[test]
    fn void_block_without_attrs() {
        let doc = "<!-- wp:separator /-->";
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::VoidBlock);
        assert_eq!(t.block_name.as_deref(), Some("core/separator"));
        assert_eq!(t.start_offset, 0);
        assert_eq!(t.token_length, doc.len());
    }

    #[test]
    fn void_block_with_attrs() {
        let doc = r#"<!-- wp:image {"id":42} /-->"#;
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::VoidBlock);
        assert_eq!(t.block_name.as_deref(), Some("core/image"));
        assert!(matches!(
            t.attrs.as_ref().and_then(|a| a.get("id")),
            Some(serde_json::Value::Number(_))
        ));
    }

    #[test]
    fn block_opener() {
        let doc = "<!-- wp:paragraph -->Hello<!-- /wp:paragraph -->";
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::BlockOpener);
        assert_eq!(t.block_name.as_deref(), Some("core/paragraph"));
    }

    #[test]
    fn block_closer() {
        let doc = "<!-- wp:paragraph -->Hello<!-- /wp:paragraph -->";
        let opener = next_token(doc.as_bytes(), 0);
        let after_opener = opener.start_offset + opener.token_length;
        let next = next_token(doc.as_bytes(), after_opener + "Hello".len());
        assert_eq!(next.kind, TokenKind::BlockCloser);
        assert_eq!(next.block_name.as_deref(), Some("core/paragraph"));
    }

    #[test]
    fn namespaced_block() {
        let doc = "<!-- wp:my-plugin/custom-block /-->";
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::VoidBlock);
        assert_eq!(t.block_name.as_deref(), Some("my-plugin/custom-block"));
    }

    #[test]
    fn nested_json_attrs() {
        let doc = r#"<!-- wp:group {"style":{"spacing":{"padding":"10px"}}} -->"#;
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::BlockOpener);
        let attrs = t.attrs.expect("should have parsed attrs");
        assert!(attrs.get("style").and_then(|s| s.get("spacing")).is_some());
    }

    #[test]
    fn skips_invalid_delimiters() {
        let doc = "<!-- just a comment --><!-- wp:paragraph /-->";
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::VoidBlock);
        assert_eq!(t.block_name.as_deref(), Some("core/paragraph"));
    }

    #[test]
    fn attrs_none_when_no_attrs() {
        let doc = "<!-- wp:paragraph /-->";
        let t = next_token(doc.as_bytes(), 0);
        assert!(t.attrs.is_none());
    }

    #[test]
    fn closer_strips_attrs_and_void() {
        // WP's regex allows these but the parser ignores them on closers.
        let doc = r#"<!-- /wp:paragraph -->"#;
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::BlockCloser);
        assert!(t.attrs.is_none());
    }

    #[test]
    fn json_string_with_close_marker_terminates_early() {
        // Documenting WP's quirk: a `}` followed by ` -->` inside a JSON
        // string still terminates the attrs scan. Our parser must match.
        let doc = r#"<!-- wp:test {"text":"ok} -->oops"} -->"#;
        let t = next_token(doc.as_bytes(), 0);
        assert_eq!(t.kind, TokenKind::BlockOpener);
        // Attrs decode will fail on the truncated JSON — that's fine,
        // we match WP's behavior (which also fails to decode).
    }
}
