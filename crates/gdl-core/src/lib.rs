//! gdl-core: read-only git status + diff model.
//!
//! Feature 1 ships only the crate skeleton; `version()` lands in the
//! Green phase to turn `tests/smoke.rs` green.

/// Returns the `gdl-core` package version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
