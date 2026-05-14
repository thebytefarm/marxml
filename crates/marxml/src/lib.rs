//! # marxml
//!
//! Fast markdown + XML query and mutation.
//!
//! This is a placeholder release. The working API lands in `0.1.0`.
//! See <https://github.com/thebytefarm/marxml>.

#![doc(html_root_url = "https://docs.rs/marxml/0.0.0")]

/// Crate version exposed for downstream diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::VERSION;

    #[test]
    fn version_matches_cargo_pkg_version() {
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }
}
