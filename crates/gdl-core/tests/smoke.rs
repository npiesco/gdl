//! Feature 1 smoke test — proves the workspace builds, the integration
//! harness runs, and `gdl_core::version()` is a real symbol returning a
//! non-empty `&'static str` matching the crate's package version.

#[test]
fn version_returns_package_version() {
    let v = gdl_core::version();
    assert!(!v.is_empty(), "gdl_core::version() must not be empty");
    assert_eq!(v, env!("CARGO_PKG_VERSION"));
}
