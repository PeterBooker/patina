//! HTML tag tokenizer and attribute parser for kses.
//!
//! Parses tag names and attributes from HTML tag strings like
//! `<div class="foo" id="bar">`.

/// Parsed attribute from an HTML tag.
pub struct ParsedAttr {
    pub name: String,
    /// The reconstructed attribute string (e.g., `class="foo"`)
    pub whole: String,
}

/// Parse attributes from an HTML tag's attribute string.
///
/// Matches WordPress's `wp_kses_hair()` — a state machine that parses
/// attribute names and values (double-quoted, single-quoted, or unquoted).
pub fn parse_attributes(
    attr: &str,
    is_uri_attr: &dyn Fn(&str) -> bool,
    check_protocol: &dyn Fn(&str) -> String,
) -> Vec<ParsedAttr> {
    let mut attrs = Vec::new();
    // Linear scan for duplicate detection — faster than HashSet for <10 attrs
    let mut seen: Vec<String> = Vec::new();
    let mut remaining = attr;

    loop {
        remaining = remaining.trim_start();
        if remaining.is_empty() {
            break;
        }

        // State 0: Find attribute name ([_a-zA-Z][-_a-zA-Z0-9:.])
        let name_end = remaining
            .find(|c: char| {
                !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != ':' && c != '.'
            })
            .unwrap_or(remaining.len());

        if name_end == 0 {
            // Skip non-attribute character (e.g., `/` at end of self-closing tag)
            remaining = &remaining[1..];
            continue;
        }

        let attr_name = &remaining[..name_end];
        let attr_name_lower = attr_name.to_lowercase();
        remaining = &remaining[name_end..];

        // State 1: Look for = sign
        let trimmed = remaining.trim_start();
        if !trimmed.starts_with('=') {
            // Valueless attribute (e.g., `hidden`, `disabled`)
            if !seen.iter().any(|s| s == &attr_name_lower) {
                seen.push(attr_name_lower);
                attrs.push(ParsedAttr {
                    name: attr_name.to_string(),
                    whole: attr_name.to_string(),
                });
            }
            remaining = trimmed;
            continue;
        }

        // Skip = and surrounding whitespace
        remaining = trimmed[1..].trim_start();

        // State 2: Parse value (double-quoted, single-quoted, or unquoted)
        let (value, quote_char, advance) = parse_value(remaining);
        remaining = &remaining[advance..];

        // Protocol check for URI attributes
        let final_value = if is_uri_attr(attr_name) {
            check_protocol(value)
        } else {
            value.to_string()
        };

        if !seen.iter().any(|s| s == &attr_name_lower) {
            seen.push(attr_name_lower);
            let whole = match quote_char {
                Some('\'') => format!("{attr_name}='{final_value}'"),
                _ => format!("{attr_name}=\"{final_value}\""),
            };
            attrs.push(ParsedAttr {
                name: attr_name.to_string(),
                whole,
            });
        }
    }

    attrs
}

/// Parse an attribute value. Returns (value, quote_char, bytes_consumed).
fn parse_value(input: &str) -> (&str, Option<char>, usize) {
    if input.starts_with('"') {
        match input[1..].find('"') {
            Some(p) => (&input[1..1 + p], Some('"'), 1 + p + 1),
            None => (&input[1..], Some('"'), input.len()),
        }
    } else if input.starts_with('\'') {
        match input[1..].find('\'') {
            Some(p) => (&input[1..1 + p], Some('\''), 1 + p + 1),
            None => (&input[1..], Some('\''), input.len()),
        }
    } else {
        // Unquoted value — ends at whitespace, /, or >
        let end = input
            .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
            .unwrap_or(input.len());
        (&input[..end], None, end)
    }
}
