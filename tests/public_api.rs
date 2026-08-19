//! Compile-time public API contract.

use podman_lens as _;

#[test]
fn initial_release_intentionally_exposes_no_public_items() {
    let library_source = include_str!("../src/lib.rs");
    let public_item = library_source.lines().find(|line| {
        let line = line.trim_start();
        line.starts_with("pub ") || line.starts_with("pub(") || line.starts_with("pub[") || line.starts_with("pub<")
    });

    assert!(
        public_item.is_none(),
        "M0 must not publish a premature API: {public_item:?}"
    );
}

#[test]
fn crate_can_be_linked_by_an_external_consumer() {
    let crate_name = env!("CARGO_PKG_NAME");
    assert_eq!(crate_name, "podman-lens");
}
