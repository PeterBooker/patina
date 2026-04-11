//! HTML sanitization — the `wp_kses` family.
//!
//! Strips disallowed HTML tags and attributes based on an allowlist.
//! Placeholder — implementation deferred to Phase 9.

pub mod allowed_html;
pub mod normalize;
pub mod protocols;
pub mod tag_parser;
