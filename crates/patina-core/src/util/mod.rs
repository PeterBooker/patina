//! Shared building blocks used across multiple function modules.
//!
//! These are implementation details, not WordPress functions themselves.
//! Entity detection, null byte stripping, character classification tables.

pub mod byte_class;
pub mod entities;
pub mod null_bytes;
