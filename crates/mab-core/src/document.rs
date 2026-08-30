//! Minimal document containers for sequences and alignments.
//!
//! This module provides thin, generic placeholders that propagate a
//! sequence's alphabet type parameter through documents. Detailed field
//! design (metadata, annotations, coordinate systems, and gap handling) is
//! intentionally deferred to a future ADR.

use crate::sequence::{Alphabet, Sequence};

/// A named sequence document.
///
/// `SequenceDocument<A>` pairs a [`Sequence<A>`] with a human-readable name.
/// The type parameter ensures that a collection of DNA sequence documents
/// cannot accidentally contain RNA or amino acid sequences.
///
/// # Design note
///
/// Additional fields such as description, source file path, annotations, and
/// quality values are deferred to a future ADR.
pub struct SequenceDocument<A: Alphabet> {
    /// Human-readable name of the sequence.
    pub name: String,
    /// The validated sequence contents.
    pub sequence: Sequence<A>,
}

/// A named alignment document.
///
/// `AlignmentDocument<A>` holds a collection of [`SequenceDocument<A>`]
/// rows. The type parameter ensures alphabet homogeneity across the
/// alignment.
///
/// # Design note
///
/// Gap handling, row metadata, reference coordinates, and consensus
/// representation are deferred to a future ADR.
///
/// Alphabet homogeneity is enforced at compile time: mixing rows of
/// different alphabets does not compile.
///
/// ```compile_fail
/// use mab_core::sequence::{IupacDna, IupacAminoAcid, Sequence};
/// use mab_core::SequenceDocument;
///
/// let dna = SequenceDocument {
///     name: String::new(),
///     sequence: Sequence::<IupacDna>::try_new("ACGT").unwrap(),
/// };
/// let protein = SequenceDocument {
///     name: String::new(),
///     sequence: Sequence::<IupacAminoAcid>::try_new("ACDE").unwrap(),
/// };
/// let alignment = vec![dna, protein]; // ERROR: mismatched types
/// ```
pub struct AlignmentDocument<A: Alphabet> {
    /// Human-readable name of the alignment.
    pub name: String,
    /// Rows of the alignment.
    pub sequences: Vec<SequenceDocument<A>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::{IupacDna, Sequence};

    #[test]
    fn sequence_document_homogeneous_alphabet() {
        let doc = SequenceDocument {
            name: "example".to_owned(),
            sequence: Sequence::<IupacDna>::try_new("ACGT").unwrap(),
        };
        assert_eq!(doc.name, "example");
        assert_eq!(doc.sequence.as_str(), "ACGT");
    }

    #[test]
    fn alignment_document_homogeneous_alphabet() {
        let alignment = AlignmentDocument {
            name: "test alignment".to_owned(),
            sequences: vec![
                SequenceDocument {
                    name: "seq1".to_owned(),
                    sequence: Sequence::<IupacDna>::try_new("ACGT").unwrap(),
                },
                SequenceDocument {
                    name: "seq2".to_owned(),
                    sequence: Sequence::<IupacDna>::try_new("TGCA").unwrap(),
                },
            ],
        };
        assert_eq!(alignment.sequences.len(), 2);
    }
}
