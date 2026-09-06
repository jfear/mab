//! Document containers for sequences and alignments.
//!
//! This module provides the content-derived document identifier
//! ([`MAB_NAMESPACE`] and [`derive_uid`]) along with thin, generic
//! containers that propagate a sequence's alphabet type parameter through
//! documents. Detailed field design (metadata, annotations, coordinate
//! systems, and gap handling) is defined in ADR-0007
//! (`docs/decisions/ADR-0007-sequence-document.md`).

use crate::sequence::{Alphabet, Sequence};
use uuid::Uuid;

pub mod annotation;
pub mod metadata;
pub mod topology;

pub use annotation::{AnnotationInterval, SequenceAnnotation};
pub use metadata::{SequenceMetadata, SequenceMetadataBuilder};
pub use topology::Topology;

/// Mab's UUID namespace for content-derived document identifiers.
///
/// Fixed value — do not regenerate.
pub const MAB_NAMESPACE: Uuid = uuid::uuid!("e2c4c74a-2df8-46ee-887d-8a517a92824a");

/// Derive the content-based document uid: `v5(MAB_NAMESPACE, sequence bytes)`.
///
/// Consumed by the document constructors in a later batch of ADR-0007.
#[allow(dead_code)] // TODO(ADR-0007): remove once SequenceDocument::with_metadata lands
fn derive_uid(sequence_bytes: &[u8]) -> Uuid {
    Uuid::new_v5(&MAB_NAMESPACE, sequence_bytes)
}

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
    fn derive_uid_is_deterministic() {
        let first = derive_uid(b"ACGT");
        let second = derive_uid(b"ACGT");
        assert_eq!(first, second);
    }

    #[test]
    fn derive_uid_matches_pinned_vector() {
        assert_eq!(
            derive_uid(b"ACGT"),
            uuid::uuid!("6118f9aa-f48f-5d14-aef8-26ecf7587974")
        );
    }

    #[test]
    fn derive_uid_depends_only_on_sequence_bytes() {
        // Same sequence contents under different names must yield the same
        // uid: the identifier is content-derived, not metadata-derived.
        let alpha = SequenceDocument {
            name: "alpha".to_owned(),
            sequence: Sequence::<IupacDna>::try_new("ACGT").unwrap(),
        };
        let beta = SequenceDocument {
            name: "beta".to_owned(),
            sequence: Sequence::<IupacDna>::try_new("ACGT").unwrap(),
        };
        assert_eq!(
            derive_uid(alpha.sequence.as_bytes()),
            derive_uid(beta.sequence.as_bytes())
        );
    }

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
