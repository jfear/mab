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

    /// Interval is invalid: `start` must be strictly less than `end`.
    ///
    /// Shared by `Interval` and all types that compose it (e.g.
    /// `AnnotationInterval`).
    #[error("invalid interval: start {start} must be less than end {end}")]
    InvalidInterval {
        /// The rejected interval start.
        start: usize,
        /// The rejected interval end.
        end: usize,
    },

    /// An annotation's kind must not be empty.
    #[error("annotation kind is empty")]
    EmptyAnnotationKind,

    /// An annotation must have at least one interval.
    #[error("annotation has no intervals")]
    EmptyAnnotationIntervals,

    /// Two annotation intervals overlap (the colliding boundary).
    #[error("overlapping annotation intervals at {start}..{end}")]
    OverlappingIntervals {
        /// Start of the colliding interval boundary.
        start: usize,
        /// End of the colliding interval boundary.
        end: usize,
    },

    /// A qualifier key must not be empty (values may be).
    #[error("annotation qualifier key is empty")]
    EmptyQualifierKey,

    /// An annotation interval exceeds the sequence bounds.
    #[error("annotation interval end {end} exceeds sequence length {sequence_len}")]
    AnnotationOutOfBounds {
        /// The rejected interval end.
        end: usize,
        /// The length of the sequence the interval was validated against.
        sequence_len: usize,
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

    #[test]
    fn invalid_interval_display_includes_bounds() {
        let msg = Error::InvalidInterval { start: 5, end: 5 }.to_string();
        assert!(msg.contains("start 5"), "{msg}");
        assert!(msg.contains("end 5"), "{msg}");
    }

    #[test]
    fn empty_annotation_kind_display_mentions_kind() {
        assert_eq!(
            Error::EmptyAnnotationKind.to_string(),
            "annotation kind is empty"
        );
    }

    #[test]
    fn empty_annotation_intervals_display_mentions_intervals() {
        assert_eq!(
            Error::EmptyAnnotationIntervals.to_string(),
            "annotation has no intervals"
        );
    }

    #[test]
    fn overlapping_intervals_display_includes_boundary() {
        let msg = Error::OverlappingIntervals { start: 3, end: 7 }.to_string();
        assert!(msg.contains("3..7"), "{msg}");
    }

    #[test]
    fn empty_qualifier_key_display_mentions_key() {
        assert_eq!(
            Error::EmptyQualifierKey.to_string(),
            "annotation qualifier key is empty"
        );
    }

    #[test]
    fn annotation_out_of_bounds_display_includes_end_and_length() {
        let msg = Error::AnnotationOutOfBounds {
            end: 12,
            sequence_len: 10,
        }
        .to_string();
        assert!(msg.contains("end 12"), "{msg}");
        assert!(msg.contains("length 10"), "{msg}");
    }
}
