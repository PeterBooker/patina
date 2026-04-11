use std::path::{Path, PathBuf};

use serde::Deserialize;

/// A single test fixture: input arguments and expected output.
#[derive(Deserialize)]
pub struct Fixture {
    pub input: Vec<serde_json::Value>,
    pub output: serde_json::Value,
}

/// Returns the path to the repo-root `fixtures/` directory.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
}

/// Load fixtures for a given function name (e.g., "esc_html").
pub fn load_fixtures(function: &str) -> Vec<Fixture> {
    let path = fixtures_dir().join(format!("{function}.json"));
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture not found: {}: {e}", path.display()));
    serde_json::from_str(&data)
        .unwrap_or_else(|e| panic!("fixture parse error: {}: {e}", path.display()))
}
