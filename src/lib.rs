//! Version-aware native Podman inspection and deployment planning.
//!
//! The first public contracts are introduced only after their versioned Libpod evidence and
//! compatibility tests exist. See the repository architecture for the planned boundaries.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds_without_a_premature_public_contract() {}
}
