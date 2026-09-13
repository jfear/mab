//! Document containers for sequences and alignments.
//!
//! This module provides the content-derived document identifier
//! ([`MAB_NAMESPACE`] and the `derive_uid` helper) along with the immutable
//! [`SequenceDocument`] (name, validated sequence, topology, annotations,
//! and source-derived metadata) and the alphabet-homogeneous
//! [`AlignmentDocument`]. See ADR-0007
//! (`docs/decisions/ADR-0007-sequence-document.md`).

use crate::sequence::{Alphabet, IupacDna, IupacRna, Sequence};
use crate::{Error, Result};
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
/// Consumed by the [`SequenceDocument`] constructors.
fn derive_uid(sequence_bytes: &[u8]) -> Uuid {
    Uuid::new_v5(&MAB_NAMESPACE, sequence_bytes)
}

/// A named sequence document with metadata, annotations, and a
/// content-derived identity.
///
/// The `uid` is a v5 UUID computed from the sequence bytes at construction.
/// Same sequence ⇒ same uid regardless of import source (dedup by design).
///
/// All fields are private; construction goes through the provided
/// constructors. Documents are immutable — no mutation methods.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SequenceDocument<A: Alphabet> {
    /// Content-derived identity computed from the validated, normalized sequence bytes.
    uid: Uuid,
    /// Human-readable document name; may be empty.
    name: String,
    /// Immutable residues validated against alphabet `A`.
    sequence: Sequence<A>,
    /// Whether the sequence is linear or circular.
    topology: Topology,
    /// Embedded annotations whose intervals lie within the sequence bounds.
    annotations: Vec<SequenceAnnotation>,
    /// Optional source-derived metadata and import extras.
    metadata: SequenceMetadata,
}

impl<A: Alphabet> SequenceDocument<A> {
    /// Create a document with default metadata and no annotations.
    /// `uid = v5(MAB_NAMESPACE, sequence bytes)`. Infallible.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::sequence::{IupacDna, Sequence};
    /// use mab_core::SequenceDocument;
    ///
    /// let doc = SequenceDocument::new("pBR322", Sequence::<IupacDna>::try_new("ACGT").unwrap());
    /// assert_eq!(doc.name(), "pBR322");
    /// assert_eq!(doc.sequence().as_str(), "ACGT");
    /// assert_eq!(doc.len(), 4);
    /// assert!(doc.annotations().is_empty());
    /// assert_eq!(doc.topology(), mab_core::Topology::Linear);
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, sequence: Sequence<A>) -> Self {
        Self::with_metadata(name, sequence, SequenceMetadata::default())
    }

    /// Create a document with metadata and no annotations; topology
    /// defaults to `Topology::Linear`. Infallible.
    #[must_use]
    pub fn with_metadata(
        name: impl Into<String>,
        sequence: Sequence<A>,
        metadata: SequenceMetadata,
    ) -> Self {
        let uid = derive_uid(sequence.as_bytes());
        Self {
            uid,
            name: name.into(),
            sequence,
            topology: Topology::Linear,
            annotations: Vec::new(),
            metadata,
        }
    }

    /// Create a document with all core fields set. This is the constructor
    /// the I/O layer will use for GenBank/EMBL import.
    ///
    /// # Examples
    ///
    /// ```
    /// use mab_core::sequence::{IupacDna, Sequence};
    /// use mab_core::{AnnotationInterval, SequenceAnnotation, SequenceDocument, SequenceMetadata, Strand, Topology};
    ///
    /// let annotation = SequenceAnnotation::new(
    ///     "lacZ",
    ///     "CDS",
    ///     Strand::Forward,
    ///     vec![AnnotationInterval::new(0, 4, false, false).unwrap()],
    ///     vec![],
    /// )
    /// .unwrap();
    /// let doc = SequenceDocument::with_annotations(
    ///     "pBR322",
    ///     Sequence::<IupacDna>::try_new("ACGT").unwrap(),
    ///     Topology::Circular,
    ///     SequenceMetadata::default(),
    ///     vec![annotation],
    /// )
    /// .unwrap();
    /// assert_eq!(doc.topology(), Topology::Circular);
    /// assert_eq!(doc.annotations().len(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::AnnotationOutOfBounds`] if any annotation
    /// interval has `end > sequence.len()`.
    pub fn with_annotations(
        name: impl Into<String>,
        sequence: Sequence<A>,
        topology: Topology,
        metadata: SequenceMetadata,
        annotations: Vec<SequenceAnnotation>,
    ) -> Result<Self> {
        let seq_len = sequence.len();
        for ann in &annotations {
            for iv in ann.intervals() {
                if iv.end() > seq_len {
                    return Err(Error::AnnotationOutOfBounds {
                        end: iv.end(),
                        sequence_len: seq_len,
                    });
                }
            }
        }
        let uid = derive_uid(sequence.as_bytes());
        Ok(Self {
            uid,
            name: name.into(),
            sequence,
            topology,
            annotations,
            metadata,
        })
    }

    /// The UID hashes `Sequence::as_bytes()` without an alphabet tag.
    /// Because `Sequence` normalizes residues to uppercase, differently cased
    /// source input with the same residues produces the same UID. Identical
    /// residue bytes across alphabet marker types also share a UID.
    #[must_use]
    pub const fn uid(&self) -> Uuid {
        self.uid
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The validated residues.
    #[must_use]
    pub const fn sequence(&self) -> &Sequence<A> {
        &self.sequence
    }

    /// Sequence topology.
    #[must_use]
    pub const fn topology(&self) -> Topology {
        self.topology
    }

    /// Embedded annotations.
    #[must_use]
    pub fn annotations(&self) -> &[SequenceAnnotation] {
        &self.annotations
    }

    /// Source-derived metadata and extras.
    #[must_use]
    pub const fn metadata(&self) -> &SequenceMetadata {
        &self.metadata
    }

    /// Sequence length in residues (derived; never stored).
    #[must_use]
    pub fn len(&self) -> usize {
        self.sequence.len()
    }

    /// Returns `true` if the sequence contains no residues.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sequence.is_empty()
    }
}

impl SequenceDocument<IupacDna> {
    /// GC fraction of the sequence. Forwards to [`Sequence::gc_fraction`].
    ///
    /// The calculated f64 is finite and in [0.0, 1.0].
    /// None means the sequence is empty.
    /// Percentage conversion, formatting, and rounding are presentation concerns.
    #[must_use]
    pub fn gc_fraction(&self) -> Option<f64> {
        self.sequence.gc_fraction()
    }
}

impl SequenceDocument<IupacRna> {
    /// GC fraction of the sequence. Forwards to [`Sequence::gc_fraction`].
    ///
    /// The calculated f64 is finite and in [0.0, 1.0].
    /// None means the sequence is empty.
    /// Percentage conversion, formatting, and rounding are presentation concerns.
    #[must_use]
    pub fn gc_fraction(&self) -> Option<f64> {
        self.sequence.gc_fraction()
    }
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
/// let dna = SequenceDocument::new(
///     String::new(),
///     Sequence::<IupacDna>::try_new("ACGT").unwrap(),
/// );
/// let protein = SequenceDocument::new(
///     String::new(),
///     Sequence::<IupacAminoAcid>::try_new("ACDE").unwrap(),
/// );
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
    use crate::Strand;
    use crate::sequence::{IupacAminoAcid, IupacDna, IupacRna, Sequence};

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
        let alpha = SequenceDocument::new("alpha", Sequence::<IupacDna>::try_new("ACGT").unwrap());
        let beta = SequenceDocument::new("beta", Sequence::<IupacDna>::try_new("ACGT").unwrap());
        assert_eq!(alpha.uid(), beta.uid());
    }

    #[test]
    fn uid_is_shared_across_alphabets_with_identical_residues() {
        // The uid hashes raw bytes without an alphabet tag, so DNA and
        // amino-acid documents with identical residues share a uid.
        let dna = SequenceDocument::new("dna", Sequence::<IupacDna>::try_new("ACGT").unwrap());
        let protein = SequenceDocument::new(
            "protein",
            Sequence::<IupacAminoAcid>::try_new("ACGT").unwrap(),
        );
        assert_eq!(dna.uid(), protein.uid());
    }

    #[test]
    fn uid_matches_derive_uid_for_same_bytes() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let doc = SequenceDocument::new("example", seq.clone());
        assert_eq!(doc.uid(), derive_uid(seq.as_bytes()));
    }

    #[test]
    fn new_equals_with_metadata_default() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let via_new = SequenceDocument::new("example", seq.clone());
        let via_with_metadata =
            SequenceDocument::with_metadata("example", seq, SequenceMetadata::default());
        assert_eq!(via_new, via_with_metadata);
        assert_eq!(via_new.uid(), via_with_metadata.uid());
    }

    #[test]
    fn len_and_is_empty_delegate_to_sequence() {
        let empty = SequenceDocument::new("empty", Sequence::<IupacDna>::try_new("").unwrap());
        assert_eq!(empty.len(), 0);
        assert!(empty.is_empty());

        let full = SequenceDocument::new("full", Sequence::<IupacDna>::try_new("ACGT").unwrap());
        assert_eq!(full.len(), 4);
        assert!(!full.is_empty());
    }

    #[test]
    fn equality_compares_all_fields() {
        let make = |name: &str| {
            SequenceDocument::new(name, Sequence::<IupacDna>::try_new("ACGT").unwrap())
        };
        assert_eq!(make("x"), make("x"));

        // Same content-derived uid, but different names ⇒ not equal.
        assert_ne!(make("alpha"), make("beta"));

        // Different metadata ⇒ not equal.
        let plain = SequenceDocument::new("x", Sequence::<IupacDna>::try_new("ACGT").unwrap());
        let described = SequenceDocument::with_metadata(
            "x",
            Sequence::<IupacDna>::try_new("ACGT").unwrap(),
            SequenceMetadata::builder()
                .description("cloning vector")
                .build(),
        );
        assert_ne!(plain, described);
    }

    #[test]
    fn with_annotations_accepts_interval_ending_at_len() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let annotation = SequenceAnnotation::new(
            "lacZ",
            "CDS",
            Strand::Forward,
            vec![AnnotationInterval::new(0, 4, false, false).unwrap()],
            vec![],
        )
        .unwrap();
        let doc = SequenceDocument::with_annotations(
            "example",
            seq,
            Topology::Circular,
            SequenceMetadata::default(),
            vec![annotation],
        )
        .unwrap();
        assert_eq!(doc.topology(), Topology::Circular);
        assert_eq!(doc.annotations().len(), 1);
    }

    #[test]
    fn with_annotations_rejects_interval_beyond_len() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let annotation = SequenceAnnotation::new(
            "lacZ",
            "CDS",
            Strand::Forward,
            vec![AnnotationInterval::new(0, 5, false, false).unwrap()],
            vec![],
        )
        .unwrap();
        let err = SequenceDocument::with_annotations(
            "example",
            seq,
            Topology::Linear,
            SequenceMetadata::default(),
            vec![annotation],
        )
        .unwrap_err();
        assert_eq!(
            err,
            Error::AnnotationOutOfBounds {
                end: 5,
                sequence_len: 4
            }
        );
    }

    #[test]
    fn gc_fraction_forwards_to_sequence() {
        let dna = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let dna_doc = SequenceDocument::new("dna", dna.clone());
        assert_eq!(dna_doc.gc_fraction(), dna.gc_fraction());

        let rna = Sequence::<IupacRna>::try_new("ACGU").unwrap();
        let rna_doc = SequenceDocument::new("rna", rna.clone());
        assert_eq!(rna_doc.gc_fraction(), rna.gc_fraction());
    }

    #[test]
    fn sequence_document_accessors_report_construction_values() {
        let seq = Sequence::<IupacDna>::try_new("ACGT").unwrap();
        let metadata = SequenceMetadata::builder().accession("AF000017.1").build();
        let doc = SequenceDocument::with_metadata("example", seq, metadata);
        assert_eq!(doc.name(), "example");
        assert_eq!(doc.sequence().as_str(), "ACGT");
        assert_eq!(doc.metadata().accession(), Some("AF000017.1"));
        assert!(doc.annotations().is_empty());
    }

    #[test]
    fn alignment_document_homogeneous_alphabet() {
        let alignment = AlignmentDocument {
            name: "test alignment".to_owned(),
            sequences: vec![
                SequenceDocument::new("seq1", Sequence::<IupacDna>::try_new("ACGT").unwrap()),
                SequenceDocument::new("seq2", Sequence::<IupacDna>::try_new("TGCA").unwrap()),
            ],
        };
        assert_eq!(alignment.sequences.len(), 2);
        assert_eq!(alignment.sequences[1].name(), "seq2");
    }
}
