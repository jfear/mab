//! Core domain types and algorithms for Mab (My Alignment Browser).
//!
//! This crate is intentionally small right now. It will grow as we research
//! and decide how to model alignments, sequences, and genomic coordinates.
//!
//! # Error handling
//!
//! Fallible operations return [`Result`], whose error type is the crate-local
//! [`Error`] defined in [`error`]. See ADR-0005.

pub mod error;
pub mod strand;

pub use error::Error;
pub use strand::Strand;

/// The result type used throughout this crate.
pub type Result<T> = std::result::Result<T, Error>;

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
