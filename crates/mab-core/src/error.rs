//! Error types for `mab-core`.
//!
//! Every fallible operation in this crate returns [`crate::Result`], whose
//! error type is the crate-local [`Error`] defined here. External errors are
//! converted at the boundary via [`From`] (`#[from]`) or wrapped with
//! context-bearing variants (`#[source]`). See ADR-0005.

/// The error type for all fallible operations in `mab-core`.
#[derive(Debug, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Invalid strand string (expected `+`, `-`, or `.`).
    #[error("invalid strand: {0:?} (expected '+', '-', or '.')")]
    StrandParse(String),

    /// A residue byte is not valid for the sequence's alphabet.
    #[error("invalid residue {residue:#04x} ('{}') at position {position} for {alphabet}", *residue as char)]
    InvalidResidue {
        /// The byte that failed validation.
        residue: u8,
        /// Zero-based position of the invalid byte in the input.
        position: usize,
        /// Human-readable name of the alphabet.
        alphabet: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
    }

    #[test]
    fn invalid_residue_display_includes_position_and_alphabet() {
        let err = Error::InvalidResidue {
            residue: b'!',
            position: 4,
            alphabet: "IUPAC DNA",
        };
        let msg = err.to_string();
        assert!(msg.contains("0x21"), "{msg}");
        assert!(msg.contains("'!'"), "{msg}");
        assert!(msg.contains("at position 4"), "{msg}");
        assert!(msg.contains("for IUPAC DNA"), "{msg}");
    }
}
