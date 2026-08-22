//! Core domain types and algorithms for Mab (My Alignment Browser).
//!
//! This crate is intentionally small right now. It will grow as we research
//! and decide how to model alignments, sequences, and genomic coordinates.

/// The current version of the library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_present() {
        assert!(!VERSION.is_empty());
    }
}
