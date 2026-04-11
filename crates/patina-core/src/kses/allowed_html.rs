//! `AllowedHtmlSpec` — compiled tag/attribute allowlist for wp_kses.
//!
//! Loaded once from the WordPress `$allowedposttags` spec at activation time,
//! or built-in for the common "post" preset.

use std::collections::{HashMap, HashSet};

/// Compiled allowed HTML specification.
/// Tags are lowercase. Attribute names are lowercase.
#[derive(Clone)]
pub struct AllowedHtmlSpec {
    /// Map of tag name → allowed attributes.
    /// If the set is empty, the tag is allowed with no attributes.
    pub tags: HashMap<String, TagSpec>,
}

/// Specification for a single allowed tag.
#[derive(Clone, Default)]
pub struct TagSpec {
    /// Allowed attribute names. `data-*` is handled as a wildcard.
    pub attrs: HashSet<String>,
    /// Whether `data-*` wildcard attributes are allowed.
    pub allow_data_attrs: bool,
    /// Whether `aria-*` wildcard attributes are allowed.
    pub allow_aria_attrs: bool,
}

impl AllowedHtmlSpec {
    /// Check if a tag is allowed.
    pub fn is_tag_allowed(&self, tag: &str) -> bool {
        self.tags.contains_key(tag)
    }

    /// Get the spec for a tag, if allowed.
    pub fn get_tag(&self, tag: &str) -> Option<&TagSpec> {
        self.tags.get(tag)
    }

    /// Check if a tag has no allowed attributes (should render bare).
    pub fn tag_has_no_attrs(&self, tag: &str) -> bool {
        self.tags.get(tag).map_or(false, |spec| {
            spec.attrs.is_empty() && !spec.allow_data_attrs
        })
    }
}

impl TagSpec {
    /// Check if an attribute name is allowed for this tag.
    pub fn is_attr_allowed(&self, attr: &str) -> bool {
        if self.attrs.contains(attr) {
            return true;
        }
        if self.allow_data_attrs && attr.starts_with("data-") {
            return true;
        }
        if self.allow_aria_attrs && attr.starts_with("aria-") {
            return true;
        }
        false
    }
}

/// Build the "post" preset from the JSON spec exported from WordPress.
/// This is called once and cached.
pub fn build_post_spec() -> AllowedHtmlSpec {
    let json = include_str!("../../../../fixtures/wp_kses_allowed_post_tags.json");
    parse_allowed_html_json(json)
}

/// Parse an allowed HTML spec from JSON (WordPress's $allowedposttags format).
pub fn parse_allowed_html_json(json: &str) -> AllowedHtmlSpec {
    let raw: HashMap<String, serde_json::Value> = serde_json::from_str(json).unwrap_or_default();

    let mut tags = HashMap::new();

    for (tag_name, attrs_val) in raw {
        let mut spec = TagSpec::default();

        if let Some(attrs_obj) = attrs_val.as_object() {
            for (attr_name, _attr_val) in attrs_obj {
                let attr_lower = attr_name.to_lowercase();
                if attr_lower == "data-*" {
                    spec.allow_data_attrs = true;
                } else if attr_lower == "aria-*" {
                    // Some tags have aria-* as a wildcard pattern
                    spec.allow_aria_attrs = true;
                } else if attr_lower.starts_with("aria-") {
                    // Individual aria attributes also imply aria support
                    spec.allow_aria_attrs = true;
                    spec.attrs.insert(attr_lower);
                } else {
                    spec.attrs.insert(attr_lower);
                }
            }
        }

        tags.insert(tag_name.to_lowercase(), spec);
    }

    AllowedHtmlSpec { tags }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_spec_loads() {
        let spec = build_post_spec();
        assert!(spec.tags.len() > 100, "expected 100+ tags");
        assert!(spec.is_tag_allowed("a"));
        assert!(spec.is_tag_allowed("div"));
        assert!(spec.is_tag_allowed("p"));
        assert!(!spec.is_tag_allowed("script"));
        assert!(!spec.is_tag_allowed("iframe"));
    }

    #[test]
    fn a_tag_attributes() {
        let spec = build_post_spec();
        let a = spec.get_tag("a").unwrap();
        assert!(a.is_attr_allowed("href"));
        assert!(a.is_attr_allowed("title"));
        assert!(a.is_attr_allowed("class"));
        assert!(a.is_attr_allowed("data-custom"));
        assert!(!a.is_attr_allowed("onclick"));
    }

    #[test]
    fn script_not_allowed() {
        let spec = build_post_spec();
        assert!(!spec.is_tag_allowed("script"));
        assert!(!spec.is_tag_allowed("style"));
        assert!(!spec.is_tag_allowed("iframe"));
    }
}
