//! `sanitize_title_with_dashes()` — WordPress slug normalizer.
//!
//! Byte-for-byte port of `wp-includes/formatting.php::sanitize_title_with_dashes()`.
//!
//! Pipeline:
//!   1. `strip_tags()` — remove HTML tags.
//!   2. Preserve escaped octets `%XX`, drop stray `%` signs.
//!   3. If the input is valid UTF-8 (always for `&str`), Unicode-lowercase
//!      then `utf8_uri_encode()` with a 200-byte cap. The latter emits every
//!      non-ASCII byte as `%xx` (lowercase) so the rest of the pipeline is
//!      pure ASCII.
//!   4. ASCII `strtolower` (no-op after step 3 but matches WP byte-parity).
//!   5. For `context == "save"`, replace a fixed list of `%xx`-encoded
//!      punctuation / whitespace / entity sequences with `-`, `''`, or `x`.
//!   6. Strip HTML-entity-like runs matching `/&.+?;/`.
//!   7. `.` → `-`.
//!   8. Drop any byte not in `[%a-z0-9 _-]`.
//!   9. Collapse runs of ASCII whitespace to a single `-`.
//!  10. Collapse runs of `-` to a single `-`.
//!  11. Trim leading/trailing `-`.

use std::borrow::Cow;

/// WordPress `sanitize_title_with_dashes($title, $raw_title, $context)`.
///
/// The second argument is unused by WP but kept for signature parity.
pub fn sanitize_title_with_dashes<'a>(
    title: &'a str,
    _raw_title: &str,
    context: &str,
) -> Cow<'a, str> {
    let s = strip_tags(title);
    let s = keep_valid_percent_octets(&s);

    // `&str` is always valid UTF-8, so wp_is_valid_utf8() is always true
    // here. Unicode-lowercase + utf8_uri_encode matches WP's behavior when
    // mbstring is loaded (universal in modern PHP).
    let lowered: String = s.chars().flat_map(char::to_lowercase).collect();
    let s = utf8_uri_encode(&lowered, 200);

    // ASCII strtolower — no-op because utf8_uri_encode output is ASCII-only,
    // but keep the call for byte-parity with WP.
    let s = s.to_ascii_lowercase();

    let s = if context == "save" {
        apply_save_context_replacements(&s)
    } else {
        s
    };

    let s = remove_html_entities(&s);
    let s = s.replace('.', "-");
    let s = filter_allowed_chars(&s);
    let s = collapse_whitespace_to_dash(&s);
    let s = collapse_dashes(&s);
    let trimmed = s.trim_matches('-').to_string();
    Cow::Owned(trimmed)
}

/// PHP `strip_tags($input)` with no allowed tags.
///
/// Removes byte ranges from `<` to the next `>` (tag), and `<!--` to `-->`
/// (HTML comment). An unclosed tag or comment causes the rest of the string
/// to be dropped, matching PHP's behavior for unterminated markup.
fn strip_tags(input: &str) -> Cow<'_, str> {
    let bytes = input.as_bytes();
    if memchr::memchr(b'<', bytes).is_none() {
        return Cow::Borrowed(input);
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // HTML comment: <!-- ... -->
            if bytes[i + 1..].starts_with(b"!--") {
                match memchr::memmem::find(&bytes[i + 4..], b"-->") {
                    Some(end) => i = i + 4 + end + 3,
                    None => break,
                }
                continue;
            }
            // Generic tag: skip through the next `>`.
            match memchr::memchr(b'>', &bytes[i + 1..]) {
                Some(end) => i = i + 1 + end + 1,
                None => break,
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    // Only complete `<...>` / `<!--...-->` ranges were dropped, and both
    // are ASCII-only (`<`, `>`, `!`, `-` all < 0x80), so the remaining
    // bytes form valid UTF-8 whenever the input did.
    Cow::Owned(unsafe { String::from_utf8_unchecked(out) })
}

/// Three-pass preserve-strip-restore for `%xx` octets, matching WP exactly.
///
/// WP runs three regex/string passes:
///   1. `%XX` (hex) → `---XX---` (tag the octets so pass 2 can't touch them)
///   2. `%`        → `` (drop every remaining `%`)
///   3. `---XX---` (hex) → `%XX` (restore the tagged octets)
///
/// The three-pass form has a quirk: a literal `---XX---` in the input
/// becomes `%XX` even though the input had no `%`. Faithful ports must
/// reproduce it, so we do the passes one-by-one rather than try to fuse
/// them into a single scan.
fn keep_valid_percent_octets(input: &str) -> String {
    let step1 = replace_percent_hex_with_marker(input);
    let step2 = step1.replace('%', "");
    replace_marker_with_percent_hex(&step2)
}

fn is_ascii_hex(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn replace_percent_hex_with_marker(input: &str) -> String {
    let bytes = input.as_bytes();
    if memchr::memchr(b'%', bytes).is_none() {
        return input.to_string();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && is_ascii_hex(bytes[i + 1])
            && is_ascii_hex(bytes[i + 2])
        {
            out.extend_from_slice(b"---");
            out.push(bytes[i + 1]);
            out.push(bytes[i + 2]);
            out.extend_from_slice(b"---");
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    unsafe { String::from_utf8_unchecked(out) }
}

fn replace_marker_with_percent_hex(input: &str) -> String {
    let bytes = input.as_bytes();
    // Fast reject — need at least "---XX---" (8 bytes) and a '---' somewhere.
    if bytes.len() < 8 || memchr::memmem::find(bytes, b"---").is_none() {
        return input.to_string();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 8 <= bytes.len()
            && &bytes[i..i + 3] == b"---"
            && is_ascii_hex(bytes[i + 3])
            && is_ascii_hex(bytes[i + 4])
            && &bytes[i + 5..i + 8] == b"---"
        {
            out.push(b'%');
            out.push(bytes[i + 3]);
            out.push(bytes[i + 4]);
            i += 8;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    unsafe { String::from_utf8_unchecked(out) }
}

/// Port of WP `utf8_uri_encode($string, 200, false)`.
///
/// Passes ASCII through as-is; encodes every byte of every multi-byte UTF-8
/// sequence as lowercase `%xx`. Stops before emitting anything that would
/// exceed `max_len` bytes of output. `max_len == 0` disables the cap.
fn utf8_uri_encode(input: &str, max_len: usize) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len());
    let mut out_len = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        let v = bytes[i];
        if v < 128 {
            if max_len > 0 && out_len + 1 > max_len {
                break;
            }
            out.push(v as char);
            out_len += 1;
            i += 1;
        } else {
            let num_octets = if v < 0xC0 {
                // Orphan continuation byte — matches PHP's permissive read.
                1
            } else if v < 0xE0 {
                2
            } else if v < 0xF0 {
                3
            } else {
                4
            };
            if max_len > 0 && out_len + num_octets * 3 > max_len {
                break;
            }
            for j in 0..num_octets {
                if i + j >= bytes.len() {
                    break;
                }
                // PHP uses `dechex($byte)` which emits lowercase hex without
                // a leading zero. For any valid UTF-8 byte >= 0x80 that's
                // always two digits, but clamp to match PHP if we ever see
                // an orphan continuation byte < 0x10 (impossible in valid
                // UTF-8, but preserves parity).
                if bytes[i + j] < 0x10 {
                    out.push('%');
                    out.push(char::from_digit(bytes[i + j] as u32, 16).unwrap_or('0'));
                } else {
                    out.push_str(&format!("%{:x}", bytes[i + j]));
                }
            }
            out_len += num_octets * 3;
            i += num_octets;
        }
    }
    out
}

/// Strip HTML-entity-like runs: `/&.+?;/` (non-greedy).
fn remove_html_entities(input: &str) -> String {
    let bytes = input.as_bytes();
    if memchr::memchr(b'&', bytes).is_none() {
        return input.to_string();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // Need at least one byte between `&` and `;`.
            if let Some(pos) = memchr::memchr(b';', &bytes[i + 1..]) {
                if pos >= 1 {
                    i += 1 + pos + 1;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    unsafe { String::from_utf8_unchecked(out) }
}

/// Strip every byte not in `[%a-z0-9 _-]`.
fn filter_allowed_chars(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if matches!(b, b'%' | b'a'..=b'z' | b'0'..=b'9' | b' ' | b'_' | b'-') {
            out.push(b);
        }
    }
    unsafe { String::from_utf8_unchecked(out) }
}

/// Collapse runs of ASCII whitespace (`\s` without `/u`) to a single `-`.
fn collapse_whitespace_to_dash(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut in_ws = false;
    for &b in bytes {
        let is_ws = matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C);
        if is_ws {
            if !in_ws {
                out.push(b'-');
                in_ws = true;
            }
        } else {
            out.push(b);
            in_ws = false;
        }
    }
    unsafe { String::from_utf8_unchecked(out) }
}

/// Collapse runs of `-` to a single `-`.
fn collapse_dashes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut prev_dash = false;
    for &b in bytes {
        if b == b'-' {
            if !prev_dash {
                out.push(b'-');
                prev_dash = true;
            }
        } else {
            out.push(b);
            prev_dash = false;
        }
    }
    unsafe { String::from_utf8_unchecked(out) }
}

/// `context == "save"` replacement chain: four `str_replace` passes.
fn apply_save_context_replacements(input: &str) -> String {
    // Pass 1: several encoded whitespace / dash punctuation sequences → `-`.
    let mut s = input.to_string();
    for needle in ["%c2%a0", "%e2%80%91", "%e2%80%93", "%e2%80%94"] {
        if s.contains(needle) {
            s = s.replace(needle, "-");
        }
    }
    // Pass 2: HTML entities for the same characters → `-`.
    for needle in [
        "&nbsp;", "&#8209;", "&#160;", "&ndash;", "&#8211;", "&mdash;", "&#8212;",
    ] {
        if s.contains(needle) {
            s = s.replace(needle, "-");
        }
    }
    // Pass 3: `/` → `-`.
    if s.contains('/') {
        s = s.replace('/', "-");
    }
    // Pass 4: long list of ornamental / zero-width sequences → strip.
    for needle in SAVE_STRIP {
        if s.contains(needle) {
            s = s.replace(needle, "");
        }
    }
    // Pass 5: non-visible width-bearing spaces → `-`.
    for needle in SAVE_REPLACE_DASH_WIDE {
        if s.contains(needle) {
            s = s.replace(needle, "-");
        }
    }
    // Pass 6: `&times` (U+00D7) → `x`.
    if s.contains("%c3%97") {
        s = s.replace("%c3%97", "x");
    }
    s
}

const SAVE_STRIP: &[&str] = &[
    // Soft hyphen.
    "%c2%ad",
    // &iexcl, &iquest.
    "%c2%a1",
    "%c2%bf",
    // Angle quotes.
    "%c2%ab",
    "%c2%bb",
    "%e2%80%b9",
    "%e2%80%ba",
    // Curly quotes.
    "%e2%80%98",
    "%e2%80%99",
    "%e2%80%9c",
    "%e2%80%9d",
    "%e2%80%9a",
    "%e2%80%9b",
    "%e2%80%9e",
    "%e2%80%9f",
    // Bullet.
    "%e2%80%a2",
    // &copy, &reg, &deg, &hellip, &trade.
    "%c2%a9",
    "%c2%ae",
    "%c2%b0",
    "%e2%80%a6",
    "%e2%84%a2",
    // Acute accents.
    "%c2%b4",
    "%cb%8a",
    "%cc%81",
    "%cd%81",
    // Grave accent, macron, caron.
    "%cc%80",
    "%cc%84",
    "%cc%8c",
    // Non-visible zero-width.
    "%e2%80%8b",
    "%e2%80%8c",
    "%e2%80%8d",
    "%e2%80%8e",
    "%e2%80%8f",
    "%e2%80%aa",
    "%e2%80%ab",
    "%e2%80%ac",
    "%e2%80%ad",
    "%e2%80%ae",
    "%ef%bb%bf",
    "%ef%bf%bc",
];

const SAVE_REPLACE_DASH_WIDE: &[&str] = &[
    "%e2%80%80", // En quad.
    "%e2%80%81", // Em quad.
    "%e2%80%82", // En space.
    "%e2%80%83", // Em space.
    "%e2%80%84", // Three-per-em space.
    "%e2%80%85", // Four-per-em space.
    "%e2%80%86", // Six-per-em space.
    "%e2%80%87", // Figure space.
    "%e2%80%88", // Punctuation space.
    "%e2%80%89", // Thin space.
    "%e2%80%8a", // Hair space.
    "%e2%80%a8", // Line separator.
    "%e2%80%a9", // Paragraph separator.
    "%e2%80%af", // Narrow no-break space.
];

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn stwd(s: &str) -> String {
        sanitize_title_with_dashes(s, "", "display").into_owned()
    }

    #[test]
    fn empty() {
        assert_eq!(stwd(""), "");
    }

    #[test]
    fn plain_ascii() {
        assert_eq!(stwd("Hello World"), "hello-world");
    }

    #[test]
    fn multiple_spaces_collapse() {
        assert_eq!(stwd("hello   world"), "hello-world");
    }

    #[test]
    fn strips_html_tags() {
        assert_eq!(stwd("<em>Title</em>"), "title");
    }

    #[test]
    fn strips_html_comment() {
        assert_eq!(stwd("foo<!-- hidden -->bar"), "foobar");
    }

    #[test]
    fn strips_percent_not_in_octet() {
        assert_eq!(stwd("50% off"), "50-off");
    }

    #[test]
    fn keeps_encoded_octets() {
        // `%20` is a valid octet — preserved through the pipeline.
        assert_eq!(stwd("a%20b"), "a%20b");
    }

    #[test]
    fn collapses_dashes_and_trims() {
        assert_eq!(stwd("--hello---world--"), "hello-world");
    }

    #[test]
    fn removes_entity_runs() {
        assert_eq!(stwd("foo&amp;bar"), "foobar");
    }

    #[test]
    fn dot_becomes_dash() {
        assert_eq!(stwd("file.name.ext"), "file-name-ext");
    }

    #[test]
    fn unicode_lowercase_then_encode() {
        // Uppercase with acute → lowercased → percent-encoded lowercase hex.
        assert_eq!(stwd("Café"), "caf%c3%a9");
    }

    #[test]
    fn japanese_gets_encoded() {
        // All non-ASCII → `%xx` lowercase.
        let out = stwd("日本語");
        assert!(out.starts_with("%e6%97%a5"), "unexpected: {out}");
        assert!(out.chars().all(|c| c.is_ascii()));
    }

    #[test]
    fn strtolower_of_acute_lowercase_only() {
        // This runs `mb_strtolower` so `É` (U+00C9) becomes `é` (U+00E9).
        // Stock PHP `strtolower` would leave `%c3%89` here; WP's use of
        // mb_strtolower before encoding produces `%c3%a9`.
        assert_eq!(stwd("É"), "%c3%a9");
    }

    #[test]
    fn save_context_slash_becomes_dash() {
        let out = sanitize_title_with_dashes("foo/bar", "", "save").into_owned();
        assert_eq!(out, "foo-bar");
    }

    #[test]
    fn save_context_en_dash_becomes_dash() {
        // U+2013 EN DASH encodes to %e2%80%93 → replaced by `-`.
        let out = sanitize_title_with_dashes("hello\u{2013}world", "", "save").into_owned();
        assert_eq!(out, "hello-world");
    }

    #[test]
    fn save_context_times_becomes_x() {
        // U+00D7 MULTIPLICATION SIGN → `%c3%97` → `x`.
        let out = sanitize_title_with_dashes("10\u{00D7}10", "", "save").into_owned();
        assert_eq!(out, "10x10");
    }

    #[test]
    fn display_context_keeps_en_dash_encoding() {
        // display context doesn't run the save-only rewrite, so the %e2%80%93
        // survives until the final filter pass strips nothing (it has % and
        // hex — all allowed).
        let out = sanitize_title_with_dashes("hello\u{2013}world", "", "display").into_owned();
        assert_eq!(out, "hello%e2%80%93world");
    }

    #[test]
    fn truncates_at_200_bytes() {
        // 250 copies of `a` — all ASCII, each takes 1 output byte, so the
        // output is capped at 200 chars (which then lose nothing since the
        // trim steps can't shorten a block of `a`).
        let input = "a".repeat(250);
        let out = stwd(&input);
        assert_eq!(out.len(), 200);
    }

    #[test]
    fn matches_wordpress_fixtures() {
        let fixtures = crate::test_support::load_fixtures("sanitize_title_with_dashes");
        assert!(!fixtures.is_empty(), "no fixtures loaded");
        for (i, f) in fixtures.iter().enumerate() {
            let input = f.input[0].as_str().unwrap();
            let raw = f.input.get(1).and_then(|v| v.as_str()).unwrap_or("");
            let ctx = f.input.get(2).and_then(|v| v.as_str()).unwrap_or("display");
            let expected = f.output.as_str().unwrap();
            let got = sanitize_title_with_dashes(input, raw, ctx).into_owned();
            assert_eq!(
                got,
                expected,
                "fixture {i} mismatch\n  input   = {:?}\n  context = {ctx}\n  expected= {:?}\n  got     = {:?}",
                truncate(input, 120),
                truncate(expected, 120),
                truncate(&got, 120),
            );
        }
    }

    fn truncate(s: &str, n: usize) -> String {
        if s.len() <= n {
            s.to_string()
        } else {
            format!(
                "{}…",
                &s[..s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(n)]
            )
        }
    }
}
