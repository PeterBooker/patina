//! Output escaping functions: `esc_html`, `esc_attr`, `esc_url`.
//!
//! These make unsafe strings safe for a specific output context (HTML body,
//! HTML attribute, URL). They do NOT strip tags — that's kses.

pub mod attr;
pub mod html;
pub mod specialchars;
pub mod url;

pub use attr::esc_attr;
pub use html::esc_html;
pub use url::esc_url;
